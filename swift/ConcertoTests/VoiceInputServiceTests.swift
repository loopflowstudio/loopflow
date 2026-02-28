import Foundation
import Testing
@testable import LoopflowCore

@Suite("Voice Input Service")
struct VoiceInputServiceTests {
    @MainActor
    @Test("start and stop recording updates state and returns final transcript")
    func startAndStopRecording() async {
        let engine = MockVoiceInputEngine(partials: ["hello world"], finalTranscript: "hello world")
        let service = VoiceInputService(
            permissionClient: MockVoicePermissionClient(status: .granted, requestAccessResult: true),
            engineFactory: { engine }
        )

        do {
            try await service.startRecording()
        } catch {
            Issue.record("Expected startRecording to succeed: \(error)")
            return
        }

        #expect(service.state == .recording)
        await waitUntil { service.partialTranscript == "hello world" }

        let transcript = await service.stopRecording()

        #expect(transcript == "hello world")
        #expect(service.state == .idle)
        #expect(service.partialTranscript.isEmpty)
        #expect(service.permissionStatus == .granted)
    }

    @MainActor
    @Test("startListening enters VAD mode and emits utterance transcripts")
    func startListeningEmitsUtterances() async {
        let engine = MockVoiceInputEngine(partials: [], finalTranscript: "")
        let service = VoiceInputService(
            permissionClient: MockVoicePermissionClient(status: .granted, requestAccessResult: true),
            engineFactory: { engine }
        )

        var transcripts: [String] = []
        service.setTranscriptHandler { transcript in
            transcripts.append(transcript)
        }

        do {
            try await service.startListening()
        } catch {
            Issue.record("Expected startListening to succeed: \(error)")
            return
        }

        #expect(service.inputMode == .voiceActivityDetection)
        #expect(service.state == .listening)

        await engine.emitVAD(.speechStarted)
        #expect(service.state == .recording)

        await engine.emitVAD(.partial("hello"))
        #expect(service.partialTranscript == "hello")

        await engine.emitVAD(.utteranceComplete("hello world"))
        await waitUntil {
            transcripts == ["hello world"]
        }

        #expect(service.state == .listening)
        #expect(service.partialTranscript.isEmpty)
    }

    @MainActor
    @Test("denied microphone permission fails start")
    func deniedMicrophonePermission() async {
        let service = VoiceInputService(
            permissionClient: MockVoicePermissionClient(
                status: .denied,
                requestAccessResult: false
            ),
            engineFactory: { MockVoiceInputEngine(partials: [], finalTranscript: "") }
        )

        do {
            try await service.startRecording()
            Issue.record("Expected startRecording to throw")
        } catch let error as VoiceInputServiceError {
            #expect(error == .microphonePermissionDenied)
        } catch {
            Issue.record("Unexpected error: \(error)")
        }

        #expect(service.state == .idle)
        #expect(service.permissionStatus == .denied)
    }

    @MainActor
    @Test("not determined microphone permission requests access and starts recording")
    func undeterminedMicrophonePermissionGrantsAndStarts() async {
        let service = VoiceInputService(
            permissionClient: MockVoicePermissionClient(
                status: .notDetermined,
                requestAccessResult: true
            ),
            engineFactory: { MockVoiceInputEngine(partials: ["hello"], finalTranscript: "hello") }
        )

        do {
            try await service.startRecording()
        } catch {
            Issue.record("Expected startRecording to succeed: \(error)")
            return
        }

        #expect(service.state == .recording)
        #expect(service.permissionStatus == .granted)
    }

    @MainActor
    @Test("cancel resets UI state and stops engine")
    func cancelStopsEngine() async {
        let engine = MockVoiceInputEngine(partials: ["partial"], finalTranscript: "ignored")
        let service = VoiceInputService(
            permissionClient: MockVoicePermissionClient(status: .granted, requestAccessResult: true),
            engineFactory: { engine }
        )

        do {
            try await service.startRecording()
        } catch {
            Issue.record("Expected startRecording to succeed: \(error)")
            return
        }

        service.cancel()
        await waitUntil {
            engine.cancelCallCount() == 1
        }

        #expect(service.state == .idle)
        #expect(service.partialTranscript.isEmpty)
    }

    @MainActor
    @Test("pause and resume listening ignore VAD events while paused")
    func pauseAndResumeListening() async {
        let engine = MockVoiceInputEngine(partials: [], finalTranscript: "")
        let service = VoiceInputService(
            permissionClient: MockVoicePermissionClient(status: .granted, requestAccessResult: true),
            engineFactory: { engine }
        )

        var transcripts: [String] = []
        service.setTranscriptHandler { transcript in
            transcripts.append(transcript)
        }

        do {
            try await service.startListening()
        } catch {
            Issue.record("Expected startListening to succeed: \(error)")
            return
        }

        service.pauseListening()
        await engine.emitVAD(.speechStarted)
        await engine.emitVAD(.partial("ignored"))
        await engine.emitVAD(.utteranceComplete("ignored"))

        #expect(service.state == .listening)
        #expect(service.partialTranscript.isEmpty)
        #expect(transcripts.isEmpty)

        do {
            try await service.resumeListening()
        } catch {
            Issue.record("Expected resumeListening to succeed: \(error)")
            return
        }

        await engine.emitVAD(.speechStarted)
        await engine.emitVAD(.partial("accepted"))
        await engine.emitVAD(.utteranceComplete("accepted"))
        await waitUntil {
            transcripts == ["accepted"]
        }
    }

    @MainActor
    @Test("setVADSensitivity updates active VAD session")
    func setVADSensitivityRestartsSession() async {
        let engine = MockVoiceInputEngine(partials: [], finalTranscript: "")
        let service = VoiceInputService(
            permissionClient: MockVoicePermissionClient(status: .granted, requestAccessResult: true),
            engineFactory: { engine }
        )

        do {
            try await service.startListening()
        } catch {
            Issue.record("Expected startListening to succeed: \(error)")
            return
        }

        service.setVADSensitivity(.noisy)
        await waitUntil {
            engine.lastVADSensitivity() == .noisy
        }

        #expect(service.vadSensitivity == .noisy)
    }

    @MainActor
    @Test("stopListening exits VAD mode")
    func stopListeningExitsVADMode() async {
        let engine = MockVoiceInputEngine(partials: [], finalTranscript: "")
        let service = VoiceInputService(
            permissionClient: MockVoicePermissionClient(status: .granted, requestAccessResult: true),
            engineFactory: { engine }
        )

        do {
            try await service.startListening()
        } catch {
            Issue.record("Expected startListening to succeed: \(error)")
            return
        }

        service.stopListening()
        await waitUntil {
            engine.stopVADCallCount() == 1
        }

        #expect(service.inputMode == .pushToTalk)
        #expect(service.state == .idle)
    }

    @MainActor
    @Test("stop falls back to partial transcript when final transcript is empty")
    func stopFallsBackToPartialTranscript() async {
        let engine = MockVoiceInputEngine(partials: ["hello world"], finalTranscript: "")
        let service = VoiceInputService(
            permissionClient: MockVoicePermissionClient(status: .granted, requestAccessResult: true),
            engineFactory: { engine }
        )

        do {
            try await service.startRecording()
        } catch {
            Issue.record("Expected startRecording to succeed: \(error)")
            return
        }

        await waitUntil { service.partialTranscript == "hello world" }
        let transcript = await service.stopRecording()

        #expect(transcript == "hello world")
    }

    @MainActor
    @Test("stop removes Whisper control tokens and blank-audio markers")
    func stopRemovesWhisperControlTokens() async {
        let noisy = "[BLANK_AUDIO] [MUSIC] (crickets chirping) <|startoftranscript|><|en|><|transcribe|><|notimestamps|> I am Jack, my name is Jack, I have a phone.<|endoftext|>"
        let engine = MockVoiceInputEngine(partials: ["partial"], finalTranscript: noisy)
        let service = VoiceInputService(
            permissionClient: MockVoicePermissionClient(status: .granted, requestAccessResult: true),
            engineFactory: { engine }
        )

        do {
            try await service.startRecording()
        } catch {
            Issue.record("Expected startRecording to succeed: \(error)")
            return
        }

        let transcript = await service.stopRecording()
        #expect(transcript == "I am Jack, my name is Jack, I have a phone.")
    }

    @MainActor
    @Test("partial fallback removes Whisper control tokens and blank-audio markers")
    func partialFallbackRemovesWhisperControlTokens() async {
        let noisyPartial = "[BLANK_AUDIO] [MUSIC] (crickets chirping) <|startoftranscript|> hello there <|endoftext|>"
        let engine = MockVoiceInputEngine(partials: [noisyPartial], finalTranscript: "")
        let service = VoiceInputService(
            permissionClient: MockVoicePermissionClient(status: .granted, requestAccessResult: true),
            engineFactory: { engine }
        )

        do {
            try await service.startRecording()
        } catch {
            Issue.record("Expected startRecording to succeed: \(error)")
            return
        }

        let transcript = await service.stopRecording()
        #expect(transcript == "hello there")
    }

    @MainActor
    @Test("cancelCurrentUtterance drops current VAD transcript and keeps listening")
    func cancelCurrentUtteranceDropsTranscript() async {
        let engine = MockVoiceInputEngine(partials: [], finalTranscript: "")
        let service = VoiceInputService(
            permissionClient: MockVoicePermissionClient(status: .granted, requestAccessResult: true),
            engineFactory: { engine }
        )

        var transcripts: [String] = []
        service.setTranscriptHandler { transcript in
            transcripts.append(transcript)
        }

        do {
            try await service.startListening()
        } catch {
            Issue.record("Expected startListening to succeed: \(error)")
            return
        }

        await engine.emitVAD(.speechStarted)
        await engine.emitVAD(.partial("draft"))
        service.cancelCurrentUtterance()
        await engine.emitVAD(.utteranceComplete("draft should drop"))

        #expect(transcripts.isEmpty)
        #expect(service.state == .listening)
    }

    @MainActor
    @Test("start waits for in-flight cancellation before re-recording")
    func startAfterCancelStartsStreamingAgain() async {
        let engine = SlowCancelVoiceInputEngine()
        let service = VoiceInputService(
            permissionClient: MockVoicePermissionClient(status: .granted, requestAccessResult: true),
            engineFactory: { engine }
        )

        do {
            try await service.startRecording()
        } catch {
            Issue.record("Expected first startRecording to succeed: \(error)")
            return
        }

        #expect(engine.startCallCount() == 1)

        service.cancel()

        do {
            try await service.startRecording()
        } catch {
            Issue.record("Expected second startRecording to succeed: \(error)")
            return
        }

        #expect(service.state == .recording)
        #expect(engine.startCallCount() == 2)
    }

    @MainActor
    @Test("background warmup prepares model without requesting microphone access")
    func backgroundWarmupPreparesModel() async {
        let permissionClient = CountingVoicePermissionClient(
            status: .notDetermined,
            requestAccessResult: true
        )
        let engine = MockVoiceInputEngine(
            partials: ["hello"],
            finalTranscript: "hello",
            prepareDelayNanos: 140_000_000
        )
        let service = VoiceInputService(
            permissionClient: permissionClient,
            engineFactory: { engine }
        )

        service.prewarmModelInBackground()
        await waitUntil {
            engine.prepareCallCount() == 1
        }

        #expect(permissionClient.requestAccessCallCount == 0)

        do {
            try await service.startRecording()
        } catch {
            Issue.record("Expected startRecording to succeed after warmup: \(error)")
            return
        }

        #expect(engine.prepareCallCount() == 1)
        #expect(service.state == .recording)
    }
}

private struct MockVoicePermissionClient: VoiceInputPermissionClient {
    let status: VoiceInputService.PermissionStatus
    let requestAccessResult: Bool

    func authorizationStatus() async -> VoiceInputService.PermissionStatus {
        status
    }

    func requestAccess() async -> Bool {
        requestAccessResult
    }
}

@MainActor
private final class CountingVoicePermissionClient: VoiceInputPermissionClient {
    let status: VoiceInputService.PermissionStatus
    let requestAccessResult: Bool
    private(set) var requestAccessCallCount = 0

    init(status: VoiceInputService.PermissionStatus, requestAccessResult: Bool) {
        self.status = status
        self.requestAccessResult = requestAccessResult
    }

    func authorizationStatus() async -> VoiceInputService.PermissionStatus {
        status
    }

    func requestAccess() async -> Bool {
        requestAccessCallCount += 1
        return requestAccessResult
    }
}

private final class MockVoiceInputEngine: VoiceInputEngine, @unchecked Sendable {
    private let partials: [String]
    private let finalTranscript: String
    private let prepareDelayNanos: UInt64
    private var prepareCount = 0
    private var cancelCount = 0
    private var stopVADCount = 0
    private var vadHandler: (@Sendable (VADEvent) -> Void)?
    private var latestVADSensitivity: VADSensitivity?

    init(
        partials: [String],
        finalTranscript: String,
        prepareDelayNanos: UInt64 = 0
    ) {
        self.partials = partials
        self.finalTranscript = finalTranscript
        self.prepareDelayNanos = prepareDelayNanos
    }

    func prepareModel(onProgress: @escaping @Sendable (Double?) -> Void) async throws {
        prepareCount += 1
        onProgress(0.4)
        if prepareDelayNanos > 0 {
            try? await Task.sleep(nanoseconds: prepareDelayNanos)
        }
        onProgress(1)
    }

    func startStreaming(onPartial: @escaping @Sendable (String) -> Void) async throws {
        for partial in partials {
            onPartial(partial)
        }
    }

    func stopStreamingAndFinalizeTranscript() async throws -> String {
        finalTranscript
    }

    func cancelStreaming() async {
        cancelCount += 1
    }

    func startVADSession(
        sensitivity: VADSensitivity,
        onEvent: @escaping @Sendable (VADEvent) -> Void
    ) async throws {
        latestVADSensitivity = sensitivity
        vadHandler = onEvent
    }

    func stopVADSession() async {
        stopVADCount += 1
        vadHandler = nil
    }

    func cancelCallCount() -> Int {
        cancelCount
    }

    func prepareCallCount() -> Int {
        prepareCount
    }

    func emitVAD(_ event: VADEvent) async {
        vadHandler?(event)
    }

    func stopVADCallCount() -> Int {
        stopVADCount
    }

    func lastVADSensitivity() -> VADSensitivity? {
        latestVADSensitivity
    }
}

private final class SlowCancelVoiceInputEngine: VoiceInputEngine, @unchecked Sendable {
    private var isStreaming = false
    private var startCount = 0

    func prepareModel(onProgress: @escaping @Sendable (Double?) -> Void) async throws {}

    func startStreaming(onPartial: @escaping @Sendable (String) -> Void) async throws {
        guard !isStreaming else { return }
        isStreaming = true
        startCount += 1
        onPartial("partial \(startCount)")
    }

    func stopStreamingAndFinalizeTranscript() async throws -> String {
        isStreaming = false
        return ""
    }

    func cancelStreaming() async {
        try? await Task.sleep(nanoseconds: 120_000_000)
        isStreaming = false
    }

    func startCallCount() -> Int {
        startCount
    }
}

@MainActor
private func waitUntil(
    timeoutNanos: UInt64 = 1_000_000_000,
    condition: @escaping @MainActor () -> Bool
) async {
    let start = DispatchTime.now().uptimeNanoseconds
    while !condition() {
        if DispatchTime.now().uptimeNanoseconds - start > timeoutNanos {
            break
        }
        try? await Task.sleep(nanoseconds: 20_000_000)
    }
}
