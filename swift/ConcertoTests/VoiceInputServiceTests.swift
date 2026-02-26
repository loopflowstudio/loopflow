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

    func cancelCallCount() -> Int {
        cancelCount
    }

    func prepareCallCount() -> Int {
        prepareCount
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
