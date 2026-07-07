import AVFAudio
import AVFoundation
import Foundation
import Speech
#if canImport(SpeechAnalysis)
import SpeechAnalysis
#endif
@preconcurrency import WhisperKit

public enum VoiceInputServiceError: LocalizedError, Equatable {
    case microphonePermissionDenied
    case transcriptionUnavailable
    case modelPreparationTimedOut
    case transcriptionTimedOut
    case cancelled

    public var errorDescription: String? {
        switch self {
        case .microphonePermissionDenied:
            return "Microphone access is required for voice input."
        case .transcriptionUnavailable:
            return "Voice transcription is unavailable right now."
        case .modelPreparationTimedOut:
            return "Speech model preparation timed out. Check your network and try again."
        case .transcriptionTimedOut:
            return "Transcription timed out. Using partial transcript."
        case .cancelled:
            return "Voice input was cancelled."
        }
    }
}

public enum InputMode: Equatable, Sendable {
    case pushToTalk
    case voiceActivityDetection
}

public enum VADSensitivity: String, CaseIterable, Sendable {
    case quiet
    case normal
    case noisy
}

public enum VADEvent: Sendable {
    case speechStarted
    case partial(String)
    case utteranceComplete(String)
}

private struct VADConfiguration {
    let energyThreshold: Float
    let silenceDuration: TimeInterval
}

private extension VADSensitivity {
    var configuration: VADConfiguration {
        switch self {
        case .quiet:
            return VADConfiguration(energyThreshold: 0.08, silenceDuration: 2.0)
        case .normal:
            return VADConfiguration(energyThreshold: 0.15, silenceDuration: 1.5)
        case .noisy:
            return VADConfiguration(energyThreshold: 0.30, silenceDuration: 1.0)
        }
    }
}

private actor VADActivityTracker {
    private var hasSpeech = false
    private var lastSpeechAt: Date?

    func markSpeechDetected() -> Bool {
        let isFirstSpeech = !hasSpeech
        hasSpeech = true
        lastSpeechAt = Date()
        return isFirstSpeech
    }

    func shouldFinalize(after silenceDuration: TimeInterval) -> Bool {
        guard hasSpeech, let lastSpeechAt else { return false }
        return Date().timeIntervalSince(lastSpeechAt) >= silenceDuration
    }

    func reset() {
        hasSpeech = false
        lastSpeechAt = nil
    }
}

private func makeVADMonitorTask(
    tracker: VADActivityTracker,
    silenceDuration: TimeInterval,
    onEvent: @escaping @Sendable (VADEvent) -> Void,
    finalize: @escaping @Sendable () async throws -> String,
    restart: @escaping @Sendable () async throws -> Void
) -> Task<Void, Never> {
    Task {
        while !Task.isCancelled {
            try? await Task.sleep(for: .milliseconds(120))
            guard await tracker.shouldFinalize(after: silenceDuration) else { continue }

            let transcript = (try? await finalize())?
                .cleanedVoiceTranscript() ?? ""
            await tracker.reset()

            guard !Task.isCancelled else { return }
            if !transcript.isEmpty {
                onEvent(.utteranceComplete(transcript))
            }

            do {
                try await restart()
            } catch {
                return
            }
        }
    }
}

@MainActor
protocol VoiceInputPermissionClient {
    func authorizationStatus() async -> VoiceInputService.PermissionStatus
    func requestAccess() async -> Bool
}

private struct SystemVoiceInputPermissionClient: VoiceInputPermissionClient {
    func authorizationStatus() async -> VoiceInputService.PermissionStatus {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized:
            return .granted
        case .notDetermined:
            return .notDetermined
        case .denied, .restricted:
            return .denied
        @unknown default:
            return .denied
        }
    }

    func requestAccess() async -> Bool {
        await withCheckedContinuation { continuation in
            AVCaptureDevice.requestAccess(for: .audio) { granted in
                continuation.resume(returning: granted)
            }
        }
    }
}

protocol VoiceInputEngine: Sendable {
    func prepareModel(onProgress: @escaping @Sendable (Double?) -> Void) async throws
    func startStreaming(onPartial: @escaping @Sendable (String) -> Void) async throws
    func stopStreamingAndFinalizeTranscript() async throws -> String
    func cancelStreaming() async
    func startVADSession(
        sensitivity: VADSensitivity,
        onEvent: @escaping @Sendable (VADEvent) -> Void
    ) async throws
    func stopVADSession() async
}

extension VoiceInputEngine {
    func startVADSession(
        sensitivity _: VADSensitivity,
        onEvent _: @escaping @Sendable (VADEvent) -> Void
    ) async throws {
        throw VoiceInputServiceError.transcriptionUnavailable
    }

    func stopVADSession() async {}
}

// SAFETY: all engine access is serialized through @MainActor VoiceInputService.
// WhisperKit callback types are non-Sendable, so we provide external
// synchronization rather than requiring strict Sendable checking here.
final class WhisperKitVoiceInputEngine: VoiceInputEngine, @unchecked Sendable {
    private let modelVariant = "tiny"
    private let modelDownloadBase: URL

    private var whisperKit: WhisperKit?
    private var streamTranscriber: AudioStreamTranscriber?
    private var streamTask: Task<Void, Error>?
    private var vadMonitorTask: Task<Void, Never>?
    private var cachedModelFolder: URL?

    init(modelDownloadBase: URL = WhisperKitVoiceInputEngine.defaultModelDownloadBase()) {
        self.modelDownloadBase = modelDownloadBase
    }

    func prepareModel(onProgress: @escaping @Sendable (Double?) -> Void) async throws {
        guard whisperKit == nil else { return }

        let modelFolder = try await resolveModelFolder(onProgress: onProgress)
        let config = WhisperKitConfig(
            model: modelVariant,
            modelFolder: modelFolder.path,
            verbose: false,
            logLevel: .none,
            load: true,
            download: false
        )
        whisperKit = try await WhisperKit(config)
    }

    func startStreaming(onPartial: @escaping @Sendable (String) -> Void) async throws {
        await stopVADSession()

        try await beginStreaming(useVAD: false, silenceThreshold: 0.3) { _, newState in
            onPartial(Self.partialText(from: newState))
        }
    }

    func stopStreamingAndFinalizeTranscript() async throws -> String {
        await stopStreaming(cancelTask: false)
        return try await finalizeTranscript()
    }

    func cancelStreaming() async {
        await stopVADSession()
        whisperKit?.audioProcessor.stopRecording()
    }

    func startVADSession(
        sensitivity: VADSensitivity,
        onEvent: @escaping @Sendable (VADEvent) -> Void
    ) async throws {
        guard vadMonitorTask == nil else { return }

        let configuration = sensitivity.configuration
        let tracker = VADActivityTracker()
        let stateHandler: @Sendable (AudioStreamTranscriber.State, AudioStreamTranscriber.State) -> Void = { _, newState in
            let partial = Self.partialText(from: newState)
            let latestEnergy = newState.bufferEnergy.last ?? 0

            if latestEnergy >= configuration.energyThreshold {
                Task {
                    let shouldEmitSpeechStart = await tracker.markSpeechDetected()
                    if shouldEmitSpeechStart {
                        onEvent(.speechStarted)
                    }
                }
            }

            guard !partial.isEmpty else { return }
            Task {
                let shouldEmitSpeechStart = await tracker.markSpeechDetected()
                if shouldEmitSpeechStart {
                    onEvent(.speechStarted)
                }
                onEvent(.partial(partial))
            }
        }

        try await beginStreaming(
            useVAD: true,
            silenceThreshold: configuration.energyThreshold,
            onStateChange: stateHandler
        )

        vadMonitorTask = makeVADMonitorTask(
            tracker: tracker,
            silenceDuration: configuration.silenceDuration,
            onEvent: onEvent,
            finalize: { [weak self] in try await self?.stopStreamingAndFinalizeTranscript() ?? "" },
            restart: { [weak self] in
                try await self?.beginStreaming(
                    useVAD: true,
                    silenceThreshold: configuration.energyThreshold,
                    onStateChange: stateHandler
                )
            }
        )
    }

    func stopVADSession() async {
        vadMonitorTask?.cancel()
        vadMonitorTask = nil
        await stopStreaming(cancelTask: true)
    }

    private func beginStreaming(
        useVAD: Bool,
        silenceThreshold: Float,
        onStateChange: @escaping @Sendable (AudioStreamTranscriber.State, AudioStreamTranscriber.State) -> Void
    ) async throws {
        guard let whisperKit else {
            throw VoiceInputServiceError.transcriptionUnavailable
        }
        guard let tokenizer = whisperKit.tokenizer else {
            throw VoiceInputServiceError.transcriptionUnavailable
        }

        await stopStreaming(cancelTask: true)

        let transcriber = AudioStreamTranscriber(
            audioEncoder: whisperKit.audioEncoder,
            featureExtractor: whisperKit.featureExtractor,
            segmentSeeker: whisperKit.segmentSeeker,
            textDecoder: whisperKit.textDecoder,
            tokenizer: tokenizer,
            audioProcessor: whisperKit.audioProcessor,
            decodingOptions: Self.decodingOptions,
            silenceThreshold: silenceThreshold,
            useVAD: useVAD,
            stateChangeCallback: onStateChange
        )

        streamTranscriber = transcriber
        streamTask = Task {
            try await transcriber.startStreamTranscription()
        }
    }

    private func resolveModelFolder(onProgress: @escaping @Sendable (Double?) -> Void) async throws -> URL {
        if let cachedModelFolder {
            return cachedModelFolder
        }

        if let existingModelFolder = findExistingModelFolder() {
            cachedModelFolder = existingModelFolder
            return existingModelFolder
        }

        onProgress(0)
        let progressCallback: @Sendable (Progress) -> Void = { progress in
            onProgress(progress.fractionCompleted)
        }
        let downloadedModelFolder = try await WhisperKit.download(
            variant: modelVariant,
            downloadBase: modelDownloadBase,
            progressCallback: progressCallback
        )
        onProgress(1)

        cachedModelFolder = downloadedModelFolder
        return downloadedModelFolder
    }

    private func findExistingModelFolder() -> URL? {
        guard let enumerator = FileManager.default.enumerator(
            at: modelDownloadBase,
            includingPropertiesForKeys: nil,
            options: [.skipsHiddenFiles]
        ) else {
            return nil
        }

        for case let url as URL in enumerator {
            guard url.lastPathComponent.localizedCaseInsensitiveContains(modelVariant) else { continue }
            guard Self.looksLikeModelFolder(url) else { continue }
            return url
        }

        return nil
    }

    private func finalizeTranscript() async throws -> String {
        guard let whisperKit else {
            throw VoiceInputServiceError.transcriptionUnavailable
        }

        let audioSamples = Array(whisperKit.audioProcessor.audioSamples)
        guard !audioSamples.isEmpty else {
            return ""
        }

        let results = try await whisperKit.transcribe(
            audioArray: audioSamples,
            decodeOptions: Self.decodingOptions
        )

        return results
            .map(\.text)
            .joined(separator: " ")
            .cleanedVoiceTranscript()
    }

    private func stopStreaming(cancelTask: Bool) async {
        if let streamTranscriber {
            await streamTranscriber.stopStreamTranscription()
        }

        if let streamTask {
            if cancelTask {
                streamTask.cancel()
            }
            _ = try? await streamTask.value
        }

        streamTask = nil
        streamTranscriber = nil
    }

    private static func partialText(from state: AudioStreamTranscriber.State) -> String {
        let segmentText = (state.confirmedSegments + state.unconfirmedSegments)
            .map(\.text)
            .joined(separator: " ")
            .cleanedVoiceTranscript()

        if !segmentText.isEmpty {
            return segmentText
        }

        let currentText = state.currentText.cleanedVoiceTranscript()
        if currentText == "Waiting for speech..." {
            return ""
        }
        return currentText
    }

    private static func looksLikeModelFolder(_ url: URL) -> Bool {
        let requiredPaths = [
            "AudioEncoder.mlmodelc",
            "MelSpectrogram.mlmodelc",
            "TextDecoder.mlmodelc",
        ]

        return requiredPaths.allSatisfy { path in
            FileManager.default.fileExists(atPath: url.appendingPathComponent(path).path)
        }
    }

    private static var decodingOptions: DecodingOptions {
        DecodingOptions(
            verbose: false,
            task: .transcribe,
            withoutTimestamps: true
        )
    }

    private static func defaultModelDownloadBase() -> URL {
        let applicationSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
        let modelsDirectory = applicationSupport
            .appendingPathComponent("Concerto", isDirectory: true)
            .appendingPathComponent("WhisperKitModels", isDirectory: true)

        try? FileManager.default.createDirectory(at: modelsDirectory, withIntermediateDirectories: true)
        return modelsDirectory
    }
}

#if canImport(SpeechAnalysis)
@available(macOS 26.0, iOS 26.0, *)
private actor AppleTranscriptBuffer {
    private var latestText = ""

    func clear() {
        latestText = ""
    }

    func replace(with text: String) -> Bool {
        guard text != latestText else { return false }
        latestText = text
        return true
    }

    func snapshot() -> String {
        latestText
    }
}

// SAFETY: all engine access is serialized through @MainActor VoiceInputService.
// SpeechAnalyzer/DictationTranscriber are non-Sendable framework types and are
// only touched through this serialized access path.
@available(macOS 26.0, iOS 26.0, *)
final class AppleDictationVoiceInputEngine: VoiceInputEngine, @unchecked Sendable {
    private let locale: Locale
    private let transcriptBuffer = AppleTranscriptBuffer()

    private var analyzer: SpeechAnalyzer?
    private var transcriber: DictationTranscriber?
    private var audioEngine: AVAudioEngine?
    private var inputContinuation: AsyncThrowingStream<AnalyzerInput, Error>.Continuation?
    private var analyzerTask: Task<Void, Error>?
    private var resultsTask: Task<Void, Error>?
    private var vadMonitorTask: Task<Void, Never>?
    private var isModelPrepared = false

    init(locale: Locale = .autoupdatingCurrent) {
        self.locale = locale
    }

    func prepareModel(onProgress: @escaping @Sendable (Double?) -> Void) async throws {
        guard !isModelPrepared else { return }

        var preset = DictationTranscriber.Preset.progressiveShortDictation
        preset.transcriptionOptions.formUnion([.punctuation, .emoji, .etiquetteReplacements])
        let transcriber = DictationTranscriber(locale: locale, preset: preset)
        let modules: [any SpeechModule] = [transcriber]
        let status = await AssetInventory.status(forModules: modules)
        guard status != .unsupported else {
            throw VoiceInputServiceError.transcriptionUnavailable
        }

        try await installAssetsIfNeeded(for: modules, onProgress: onProgress)

        let analyzer = SpeechAnalyzer(
            modules: modules,
            options: SpeechAnalyzer.Options(
                priority: .userInitiated,
                modelRetention: .processLifetime
            )
        )

        try await analyzer.prepareToAnalyze(in: nil) { progress in
            onProgress(progress.fractionCompleted)
        }
        onProgress(1)

        self.transcriber = transcriber
        self.analyzer = analyzer
        isModelPrepared = true
    }

    func startStreaming(onPartial: @escaping @Sendable (String) -> Void) async throws {
        await stopVADSession()
        try await beginStreaming(onPartial: onPartial)
    }

    func startVADSession(
        sensitivity: VADSensitivity,
        onEvent: @escaping @Sendable (VADEvent) -> Void
    ) async throws {
        guard vadMonitorTask == nil else { return }

        let configuration = sensitivity.configuration
        let tracker = VADActivityTracker()
        let partialHandler: @Sendable (String) -> Void = { partial in
            guard !partial.isEmpty else { return }
            Task {
                let shouldEmitSpeechStart = await tracker.markSpeechDetected()
                if shouldEmitSpeechStart {
                    onEvent(.speechStarted)
                }
                onEvent(.partial(partial))
            }
        }

        try await beginStreaming(onPartial: partialHandler)

        vadMonitorTask = makeVADMonitorTask(
            tracker: tracker,
            silenceDuration: configuration.silenceDuration,
            onEvent: onEvent,
            finalize: { [weak self] in try await self?.stopStreamingAndFinalizeTranscript() ?? "" },
            restart: { [weak self] in try await self?.beginStreaming(onPartial: partialHandler) }
        )
    }

    func stopVADSession() async {
        vadMonitorTask?.cancel()
        vadMonitorTask = nil
        await cancelStreaming()
    }

    private func beginStreaming(onPartial: @escaping @Sendable (String) -> Void) async throws {
        guard analyzerTask == nil, resultsTask == nil else { return }
        guard let analyzer, let transcriber else {
            throw VoiceInputServiceError.transcriptionUnavailable
        }

        await transcriptBuffer.clear()

        let stream = AsyncThrowingStream<AnalyzerInput, Error> { continuation in
            self.inputContinuation = continuation
        }

        do {
            try await startAudioCapture(for: transcriber)
        } catch {
            inputContinuation?.finish(throwing: error)
            inputContinuation = nil
            throw error
        }

        analyzerTask = Task {
            try await analyzer.start(inputSequence: stream)
        }

        resultsTask = Task {
            for try await result in transcriber.results {
                let latest = String(result.text.characters)
                    .cleanedVoiceTranscript()
                guard !latest.isEmpty else { continue }

                let didChange = await self.transcriptBuffer.replace(with: latest)
                guard didChange else { continue }
                onPartial(latest)
            }
        }
    }

    func stopStreamingAndFinalizeTranscript() async throws -> String {
        guard analyzerTask != nil || resultsTask != nil else {
            return await transcriptBuffer.snapshot()
        }

        await stopAudioCapture()

        if let analyzer {
            try await analyzer.finalizeAndFinishThroughEndOfInput()
        }

        try await drainTasks()
        return await transcriptBuffer.snapshot()
    }

    func cancelStreaming() async {
        vadMonitorTask?.cancel()
        vadMonitorTask = nil
        await stopAudioCapture()
        await transcriptBuffer.clear()

        if let analyzer {
            await analyzer.cancelAndFinishNow()
        }

        analyzerTask?.cancel()
        resultsTask?.cancel()
        _ = try? await analyzerTask?.value
        _ = try? await resultsTask?.value
        analyzerTask = nil
        resultsTask = nil
    }

    private func installAssetsIfNeeded(
        for modules: [any SpeechModule],
        onProgress: @escaping @Sendable (Double?) -> Void
    ) async throws {
        let status = await AssetInventory.status(forModules: modules)
        switch status {
        case .installed:
            onProgress(1)
            return
        case .unsupported:
            throw VoiceInputServiceError.transcriptionUnavailable
        case .supported, .downloading:
            break
        @unknown default:
            break
        }

        guard let installationRequest = try await AssetInventory.assetInstallationRequest(supporting: modules) else {
            // Assets may already be available by the time we request installation.
            return
        }

        onProgress(0)
        let progressTask = Task {
            while !Task.isCancelled {
                onProgress(installationRequest.progress.fractionCompleted)
                try? await Task.sleep(for: .milliseconds(150))
            }
        }

        do {
            try await installationRequest.downloadAndInstall()
        } catch {
            progressTask.cancel()
            throw error
        }
        progressTask.cancel()
        onProgress(1)
    }

    private func startAudioCapture(for transcriber: DictationTranscriber) async throws {
        let audioEngine = AVAudioEngine()
        let inputNode = audioEngine.inputNode
        let inputFormat = inputNode.outputFormat(forBus: 0)
        let preferredFormat = await SpeechAnalyzer.bestAvailableAudioFormat(
            compatibleWith: [transcriber],
            considering: inputFormat
        ) ?? inputFormat

        inputNode.installTap(onBus: 0, bufferSize: 1_024, format: preferredFormat) { [weak self] buffer, _ in
            guard let self else { return }
            self.inputContinuation?.yield(.init(buffer: buffer))
        }

        do {
            audioEngine.prepare()
            try audioEngine.start()
            self.audioEngine = audioEngine
        } catch {
            inputNode.removeTap(onBus: 0)
            audioEngine.stop()
            throw error
        }
    }

    private func stopAudioCapture() async {
        if let audioEngine {
            audioEngine.inputNode.removeTap(onBus: 0)
            audioEngine.stop()
        }
        self.audioEngine = nil

        inputContinuation?.finish()
        inputContinuation = nil
    }

    private func drainTasks() async throws {
        defer {
            analyzerTask = nil
            resultsTask = nil
        }

        do {
            if let analyzerTask {
                try await analyzerTask.value
            }
            if let resultsTask {
                try await resultsTask.value
            }
        } catch is CancellationError {
            // treat cancellation as expected during stop/cancel boundaries
        }
    }
}
#endif

@Observable
@MainActor
public final class VoiceInputService {
    private static let modelPreparationTimeout: Duration = .seconds(90)
    private static let transcriptionTimeout: Duration = .seconds(20)

    public enum State: Equatable {
        case idle
        case listening
        case recording
        case transcribing
    }

    public enum PermissionStatus: Equatable {
        case notDetermined
        case granted
        case denied
    }

    public private(set) var state: State = .idle
    public private(set) var inputMode: InputMode = .pushToTalk
    public private(set) var vadSensitivity: VADSensitivity = .normal
    public private(set) var partialTranscript: String = ""
    public private(set) var permissionStatus: PermissionStatus = .notDetermined
    public private(set) var modelDownloadProgress: Double?
    public private(set) var modelStatusText: String?
    public private(set) var errorMessage: String?

    private let permissionClient: any VoiceInputPermissionClient
    private let engineFactory: () -> any VoiceInputEngine
    private var engine: (any VoiceInputEngine)?
    private var modelPreparationTask: Task<Void, Error>?
    private var isModelPrepared = false
    private var lastModelProgressBucket: Int?
    private var operationID = UUID()
    private var isListeningPaused = false
    private var dropNextVADUtterance = false
    @ObservationIgnored private var transcriptHandler: ((String) -> Void)?

    public init() {
        permissionClient = SystemVoiceInputPermissionClient()
        engineFactory = Self.defaultEngineFactory
    }

    init(
        permissionClient: any VoiceInputPermissionClient,
        engineFactory: @escaping () -> any VoiceInputEngine
    ) {
        self.permissionClient = permissionClient
        self.engineFactory = engineFactory
    }

    private static func defaultEngineFactory() -> any VoiceInputEngine {
        #if canImport(SpeechAnalysis)
        if #available(macOS 26.0, iOS 26.0, *) {
            return AppleDictationVoiceInputEngine()
        }
        #endif
        return WhisperKitVoiceInputEngine()
    }

    public func prewarmModelInBackground() {
        guard !isModelPrepared else { return }
        guard modelPreparationTask == nil else { return }
        logVoice("prewarm started")

        Task { @MainActor [weak self] in
            guard let self else { return }
            do {
                try await self.ensureModelPrepared(for: nil)
                self.logVoice("prewarm completed")
            } catch {
                self.logVoice("prewarm failed: \(error.localizedDescription)")
                // Ignore background warmup failures; on-demand start retries.
            }
        }
    }

    public func setTranscriptHandler(_ handler: ((String) -> Void)?) {
        transcriptHandler = handler
    }

    public func startListening() async throws {
        if inputMode == .voiceActivityDetection, state == .listening, !isListeningPaused {
            return
        }

        errorMessage = nil
        lastModelProgressBucket = nil
        inputMode = .voiceActivityDetection
        isListeningPaused = false
        dropNextVADUtterance = false
        logVoice("VAD start requested")

        try await ensurePermission()

        let currentOperationID = UUID()
        operationID = currentOperationID
        partialTranscript = ""
        state = .transcribing
        modelStatusText = "Preparing speech model…"
        modelDownloadProgress = nil

        do {
            let engine = resolveEngine()
            try await ensureModelPrepared(for: currentOperationID)

            guard operationID == currentOperationID else {
                throw VoiceInputServiceError.cancelled
            }

            modelStatusText = nil
            modelDownloadProgress = nil

            try await engine.startVADSession(
                sensitivity: vadSensitivity,
                onEvent: makeVADEventHandler(operationID: currentOperationID)
            )

            guard operationID == currentOperationID else {
                throw VoiceInputServiceError.cancelled
            }

            state = .listening
            logVoice("VAD listening started")
        } catch {
            if case VoiceInputServiceError.cancelled = error {
                logVoice("VAD start cancelled")
                return
            }
            inputMode = .pushToTalk
            resetPresentationState()
            errorMessage = error.localizedDescription
            logVoice("VAD start failed: \(error.localizedDescription)")
            throw error
        }
    }

    public func stopListening() {
        guard inputMode == .voiceActivityDetection else { return }

        operationID = UUID()
        inputMode = .pushToTalk
        isListeningPaused = false
        dropNextVADUtterance = false
        resetPresentationState()
        logVoice("VAD stop requested")

        guard let engine else { return }
        Task {
            await engine.stopVADSession()
        }
    }

    public func setVADSensitivity(_ sensitivity: VADSensitivity) {
        guard vadSensitivity != sensitivity else { return }
        vadSensitivity = sensitivity
        logVoice("VAD sensitivity set: \(sensitivity.rawValue)")

        guard inputMode == .voiceActivityDetection, !isListeningPaused else { return }
        guard let engine else { return }
        let currentOperationID = operationID

        Task { @MainActor [weak self] in
            guard let self else { return }
            guard self.inputMode == .voiceActivityDetection else { return }
            guard self.operationID == currentOperationID else { return }

            await engine.stopVADSession()

            do {
                try await engine.startVADSession(
                    sensitivity: sensitivity,
                    onEvent: self.makeVADEventHandler(operationID: currentOperationID)
                )
            } catch {
                self.errorMessage = error.localizedDescription
            }
        }
    }

    public func pauseListening() {
        guard inputMode == .voiceActivityDetection else { return }
        isListeningPaused = true

        if state == .recording || state == .transcribing {
            dropNextVADUtterance = true
        }

        partialTranscript = ""
        state = .listening
        logVoice("VAD paused")
    }

    public func resumeListening() async throws {
        guard inputMode == .voiceActivityDetection else { return }
        guard isListeningPaused else { return }
        isListeningPaused = false
        state = .listening
        logVoice("VAD resumed")
    }

    public func cancelCurrentUtterance() {
        guard inputMode == .voiceActivityDetection else { return }
        guard state == .recording || state == .transcribing else { return }

        dropNextVADUtterance = true
        partialTranscript = ""
        state = .listening
        logVoice("VAD utterance cancelled")
    }

    public func startRecording() async throws {
        guard state == .idle else { return }
        guard inputMode == .pushToTalk else { return }
        errorMessage = nil
        lastModelProgressBucket = nil
        logVoice("start requested")
        dropNextVADUtterance = false

        if let engine {
            await engine.cancelStreaming()
        }

        try await ensurePermission()
        let currentOperationID = UUID()
        operationID = currentOperationID

        partialTranscript = ""
        state = .transcribing
        modelStatusText = "Preparing speech model…"
        modelDownloadProgress = nil
        logVoice("permission granted; preparing model")

        do {
            let engine = resolveEngine()
            try await ensureModelPrepared(for: currentOperationID)

            guard operationID == currentOperationID else {
                throw VoiceInputServiceError.cancelled
            }

            modelStatusText = nil
            modelDownloadProgress = nil

            try await engine.startStreaming { [weak self] partial in
                Task { @MainActor [weak self] in
                    guard let self else { return }
                    guard self.operationID == currentOperationID else { return }
                    guard self.state == .recording || self.state == .transcribing else { return }
                    self.partialTranscript = partial.cleanedVoiceTranscript()
                }
            }

            guard operationID == currentOperationID else {
                throw VoiceInputServiceError.cancelled
            }

            state = .recording
            logVoice("recording started")
        } catch {
            if case VoiceInputServiceError.cancelled = error {
                logVoice("start cancelled")
                return
            }
            resetPresentationState()
            errorMessage = error.localizedDescription
            logVoice("start failed: \(error.localizedDescription)")
            throw error
        }
    }

    public func stopRecording() async -> String {
        guard inputMode == .pushToTalk else { return "" }
        guard state == .recording || state == .transcribing else { return "" }
        logVoice("stop requested")

        let currentOperationID = operationID
        state = .transcribing
        modelStatusText = "Transcribing…"
        modelDownloadProgress = nil

        guard let engine else {
            let fallback = partialTranscript
            if operationID == currentOperationID {
                resetPresentationState()
            }
            return fallback
        }

        let fallbackTranscript = partialTranscript
        var preservedErrorMessage: String?
        let transcript: String?
        do {
            transcript = try await withTimeout(VoiceInputService.transcriptionTimeout) {
                try await engine.stopStreamingAndFinalizeTranscript()
            } timeoutError: {
                VoiceInputServiceError.transcriptionTimedOut
            }.cleanedVoiceTranscript()
        } catch let error as VoiceInputServiceError where error == .transcriptionTimedOut {
            preservedErrorMessage = error.localizedDescription
            logVoice("stop timed out; falling back to partial transcript")
            transcript = nil
        } catch {
            preservedErrorMessage = error.localizedDescription
            logVoice("stop failed: \(error.localizedDescription); falling back to partial transcript")
            transcript = nil
        }

        if operationID == currentOperationID {
            resetPresentationState()
            if let preservedErrorMessage {
                errorMessage = preservedErrorMessage
            }
        }

        if let transcript, !transcript.isEmpty {
            logVoice("stop complete with final transcript (\(transcript.count) chars)")
            return transcript
        }

        logVoice("stop complete with partial transcript fallback (\(fallbackTranscript.count) chars)")
        return fallbackTranscript
    }

    public func cancel() {
        operationID = UUID()
        inputMode = .pushToTalk
        isListeningPaused = false
        dropNextVADUtterance = false
        resetPresentationState()
        logVoice("cancel requested")

        guard let engine else { return }
        Task {
            await engine.stopVADSession()
            await engine.cancelStreaming()
        }
    }

    private func makeVADEventHandler(
        operationID expectedID: UUID
    ) -> @Sendable (VADEvent) -> Void {
        { [weak self] event in
            Task { @MainActor [weak self] in
                guard let self else { return }
                guard self.operationID == expectedID else { return }
                guard self.inputMode == .voiceActivityDetection else { return }
                if self.isListeningPaused {
                    self.handlePausedVADEvent(event)
                    return
                }
                self.handleVADEvent(event)
            }
        }
    }

    private func handleVADEvent(_ event: VADEvent) {
        switch event {
        case .speechStarted:
            state = .recording
            partialTranscript = ""

        case .partial(let partial):
            guard !dropNextVADUtterance else { return }
            let cleaned = partial.cleanedVoiceTranscript()
            guard !cleaned.isEmpty else { return }
            if state == .listening {
                state = .recording
            }
            partialTranscript = cleaned

        case .utteranceComplete(let transcript):
            state = .transcribing
            if !dropNextVADUtterance {
                let cleaned = transcript.cleanedVoiceTranscript()
                if !cleaned.isEmpty {
                    transcriptHandler?(cleaned)
                }
            }
            dropNextVADUtterance = false
            partialTranscript = ""
            state = .listening
        }
    }

    private func handlePausedVADEvent(_ event: VADEvent) {
        guard dropNextVADUtterance else { return }
        if case .utteranceComplete = event {
            dropNextVADUtterance = false
            partialTranscript = ""
            state = .listening
        }
    }

    private func ensurePermission() async throws {
        let status = await permissionClient.authorizationStatus()
        permissionStatus = status
        logVoice("permission status: \(String(describing: status))")

        switch status {
        case .granted:
            return
        case .denied:
            throw VoiceInputServiceError.microphonePermissionDenied
        case .notDetermined:
            let granted = await permissionClient.requestAccess()
            permissionStatus = granted ? .granted : .denied
            logVoice("permission prompt result: \(granted ? "granted" : "denied")")
            guard granted else {
                throw VoiceInputServiceError.microphonePermissionDenied
            }
        }
    }

    private func ensureModelPrepared(for operationID: UUID?) async throws {
        if isModelPrepared {
            return
        }

        if let modelPreparationTask {
            do {
                try await withTimeout(VoiceInputService.modelPreparationTimeout) {
                    try await modelPreparationTask.value
                } timeoutError: {
                    VoiceInputServiceError.modelPreparationTimedOut
                }
            } catch {
                modelPreparationTask.cancel()
                self.modelPreparationTask = nil
                if Task.isCancelled {
                    throw VoiceInputServiceError.cancelled
                }
                logVoice("model preparation wait failed: \(error.localizedDescription)")
                throw error
            }
            return
        }

        let engine = resolveEngine()
        logVoice("model preparation task started")
        let task = Task<Void, Error> {
            try await engine.prepareModel { [weak self] progress in
                Task { @MainActor [weak self] in
                    guard let self else { return }
                    self.logModelProgress(progress)
                    guard let operationID else { return }
                    guard self.operationID == operationID else { return }
                    self.modelStatusText = "Downloading speech model…"
                    self.modelDownloadProgress = progress.map { max(0, min(1, $0)) }
                }
            }
        }

        modelPreparationTask = task
        defer { modelPreparationTask = nil }

        do {
            try await withTimeout(VoiceInputService.modelPreparationTimeout) {
                try await task.value
            } timeoutError: {
                VoiceInputServiceError.modelPreparationTimedOut
            }
        } catch {
            task.cancel()
            if Task.isCancelled {
                throw VoiceInputServiceError.cancelled
            }
            logVoice("model preparation failed: \(error.localizedDescription)")
            throw error
        }
        isModelPrepared = true
        logVoice("model preparation complete")
    }

    private func withTimeout<T: Sendable>(
        _ timeout: Duration,
        operation: @escaping @Sendable () async throws -> T,
        timeoutError: @escaping @Sendable () -> Error
    ) async throws -> T {
        try await withThrowingTaskGroup(of: T.self) { group in
            group.addTask {
                try await operation()
            }
            group.addTask {
                try await Task.sleep(for: timeout)
                throw timeoutError()
            }

            guard let result = try await group.next() else {
                throw VoiceInputServiceError.transcriptionUnavailable
            }
            group.cancelAll()
            return result
        }
    }

    private func logModelProgress(_ progress: Double?) {
        guard let progress else { return }
        let bounded = max(0, min(1, progress))
        let bucket = Int((bounded * 100).rounded(.down) / 10) * 10
        guard bucket != lastModelProgressBucket else { return }
        lastModelProgressBucket = bucket
        logVoice("model download progress: \(bucket)%")
    }

    private func logVoice(_ message: String) {
        LoggingService.ui("voice: \(message)")
    }

    private func resolveEngine() -> any VoiceInputEngine {
        if let engine {
            return engine
        }
        let newEngine = engineFactory()
        logVoice("engine selected: \(type(of: newEngine))")
        engine = newEngine
        return newEngine
    }

    private func resetPresentationState() {
        state = .idle
        partialTranscript = ""
        modelDownloadProgress = nil
        modelStatusText = nil
        errorMessage = nil
    }
}

private extension String {
    func cleanedVoiceTranscript() -> String {
        replacingOccurrences(
            of: #"<\|[^|>]+?\|>"#,
            with: " ",
            options: .regularExpression
        )
        .replacingOccurrences(
            of: #"\[BLANK_AUDIO\]"#,
            with: " ",
            options: [.regularExpression, .caseInsensitive]
        )
        .replacingOccurrences(
            of: #"(?:\[[^\]]*(?:music|applause|laughter|noise|silence|inaudible|chirping|crickets|cheering|clapping|humming|singing)[^\]]*\]|\([^)]*(?:music|applause|laughter|noise|silence|inaudible|chirping|crickets|cheering|clapping|humming|singing)[^)]*\))"#,
            with: " ",
            options: [.regularExpression, .caseInsensitive]
        )
        .normalizedWhitespace()
    }

    func normalizedWhitespace() -> String {
        split(whereSeparator: \.isWhitespace)
            .joined(separator: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
