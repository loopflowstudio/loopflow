import AVFoundation
import Foundation
@preconcurrency import WhisperKit

public enum VoiceInputServiceError: LocalizedError, Equatable {
    case microphonePermissionDenied
    case transcriptionUnavailable
    case cancelled

    public var errorDescription: String? {
        switch self {
        case .microphonePermissionDenied:
            return "Microphone access is required for voice input."
        case .transcriptionUnavailable:
            return "Voice transcription is unavailable right now."
        case .cancelled:
            return "Voice input was cancelled."
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
        case .restricted, .denied:
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
}

final class WhisperKitVoiceInputEngine: VoiceInputEngine, @unchecked Sendable {
    private let modelVariant = "tiny"
    private let modelDownloadBase: URL

    private var whisperKit: WhisperKit?
    private var streamTranscriber: AudioStreamTranscriber?
    private var streamTask: Task<Void, Error>?
    private var latestPartialTranscript = ""
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
        guard streamTask == nil else { return }
        guard let whisperKit else {
            throw VoiceInputServiceError.transcriptionUnavailable
        }
        guard let tokenizer = whisperKit.tokenizer else {
            throw VoiceInputServiceError.transcriptionUnavailable
        }

        let decodingOptions = DecodingOptions(
            verbose: false,
            task: .transcribe,
            withoutTimestamps: true
        )

        let stateCallback: @Sendable (AudioStreamTranscriber.State, AudioStreamTranscriber.State) -> Void = { _, newState in
            let partial = WhisperKitVoiceInputEngine.partialText(from: newState)
            onPartial(partial)
        }

        let transcriber = AudioStreamTranscriber(
            audioEncoder: whisperKit.audioEncoder,
            featureExtractor: whisperKit.featureExtractor,
            segmentSeeker: whisperKit.segmentSeeker,
            textDecoder: whisperKit.textDecoder,
            tokenizer: tokenizer,
            audioProcessor: whisperKit.audioProcessor,
            decodingOptions: decodingOptions,
            useVAD: false
        ) { oldState, newState in
            stateCallback(oldState, newState)
        }

        streamTranscriber = transcriber
        streamTask = Task {
            try await transcriber.startStreamTranscription()
        }
    }

    func stopStreamingAndFinalizeTranscript() async throws -> String {
        if let streamTranscriber {
            await streamTranscriber.stopStreamTranscription()
        }

        if let streamTask {
            _ = try? await streamTask.value
        }

        streamTask = nil
        streamTranscriber = nil

        return try await finalizeTranscript()
    }

    func cancelStreaming() async {
        if let streamTranscriber {
            await streamTranscriber.stopStreamTranscription()
        }

        if let streamTask {
            streamTask.cancel()
            _ = try? await streamTask.value
        }

        streamTask = nil
        streamTranscriber = nil
        whisperKit?.audioProcessor.stopRecording()
        latestPartialTranscript = ""
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
            return latestPartialTranscript.normalizedWhitespace()
        }

        let results = try await whisperKit.transcribe(
            audioArray: audioSamples,
            decodeOptions: DecodingOptions(
                verbose: false,
                task: .transcribe,
                withoutTimestamps: true
            )
        )

        let transcript = results
            .map(\.text)
            .joined(separator: " ")
            .normalizedWhitespace()

        if !transcript.isEmpty {
            latestPartialTranscript = transcript
        }

        return latestPartialTranscript.normalizedWhitespace()
    }

    private static func partialText(from state: AudioStreamTranscriber.State) -> String {
        let segmentText = (state.confirmedSegments + state.unconfirmedSegments)
            .map(\.text)
            .joined(separator: " ")
            .normalizedWhitespace()

        if !segmentText.isEmpty {
            return segmentText
        }

        let currentText = state.currentText.normalizedWhitespace()
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

@Observable
@MainActor
public final class VoiceInputService {
    public enum State: Equatable {
        case idle
        case recording
        case transcribing
    }

    public enum PermissionStatus: Equatable {
        case notDetermined
        case granted
        case denied
    }

    public private(set) var state: State = .idle
    public private(set) var partialTranscript: String = ""
    public private(set) var permissionStatus: PermissionStatus = .notDetermined
    public private(set) var modelDownloadProgress: Double?
    public private(set) var modelStatusText: String?

    private let permissionClient: any VoiceInputPermissionClient
    private let engineFactory: () -> any VoiceInputEngine
    private var engine: (any VoiceInputEngine)?
    private var operationID = UUID()

    public init() {
        permissionClient = SystemVoiceInputPermissionClient()
        engineFactory = { WhisperKitVoiceInputEngine() }
    }

    init(
        permissionClient: any VoiceInputPermissionClient,
        engineFactory: @escaping () -> any VoiceInputEngine
    ) {
        self.permissionClient = permissionClient
        self.engineFactory = engineFactory
    }

    public func startRecording() async throws {
        guard state == .idle else { return }

        try await ensurePermission()
        let currentOperationID = UUID()
        operationID = currentOperationID

        partialTranscript = ""
        state = .transcribing
        modelStatusText = "Preparing speech model…"
        modelDownloadProgress = nil

        do {
            let engine = resolveEngine()
            try await engine.prepareModel { [weak self] progress in
                Task { @MainActor [weak self] in
                    guard let self else { return }
                    guard self.operationID == currentOperationID else { return }
                    self.modelStatusText = "Downloading speech model…"
                    self.modelDownloadProgress = progress.map { max(0, min(1, $0)) }
                }
            }

            guard operationID == currentOperationID else {
                throw VoiceInputServiceError.cancelled
            }

            modelStatusText = nil
            modelDownloadProgress = nil
            partialTranscript = ""

            try await engine.startStreaming { [weak self] partial in
                Task { @MainActor [weak self] in
                    guard let self else { return }
                    guard self.operationID == currentOperationID else { return }
                    guard self.state == .recording || self.state == .transcribing else { return }
                    self.partialTranscript = partial
                }
            }

            guard operationID == currentOperationID else {
                throw VoiceInputServiceError.cancelled
            }

            state = .recording
        } catch {
            if case VoiceInputServiceError.cancelled = error {
                return
            }
            resetPresentationState()
            throw error
        }
    }

    public func stopRecording() async -> String {
        guard state == .recording || state == .transcribing else { return "" }

        let currentOperationID = operationID
        state = .transcribing
        modelStatusText = "Transcribing…"
        modelDownloadProgress = nil

        defer {
            if operationID == currentOperationID {
                resetPresentationState()
            }
        }

        guard let engine else {
            return partialTranscript.normalizedWhitespace()
        }

        let transcript = (try? await engine.stopStreamingAndFinalizeTranscript())
            ?? partialTranscript

        return transcript.normalizedWhitespace()
    }

    public func cancel() {
        operationID = UUID()
        resetPresentationState()

        guard let engine else { return }
        Task {
            await engine.cancelStreaming()
        }
    }

    private func ensurePermission() async throws {
        let status = await permissionClient.authorizationStatus()
        permissionStatus = status

        switch status {
        case .granted:
            return
        case .denied:
            throw VoiceInputServiceError.microphonePermissionDenied
        case .notDetermined:
            let granted = await permissionClient.requestAccess()
            permissionStatus = granted ? .granted : .denied
            guard granted else {
                throw VoiceInputServiceError.microphonePermissionDenied
            }
        }
    }

    private func resolveEngine() -> any VoiceInputEngine {
        if let engine {
            return engine
        }
        let newEngine = engineFactory()
        engine = newEngine
        return newEngine
    }

    private func resetPresentationState() {
        state = .idle
        partialTranscript = ""
        modelDownloadProgress = nil
        modelStatusText = nil
    }
}

private extension String {
    func normalizedWhitespace() -> String {
        split(whereSeparator: \.isWhitespace)
            .joined(separator: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
