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
            permissionClient: MockVoicePermissionClient(status: .granted),
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
            permissionClient: MockVoicePermissionClient(status: .denied),
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
    @Test("cancel resets UI state and stops engine")
    func cancelStopsEngine() async {
        let engine = MockVoiceInputEngine(partials: ["partial"], finalTranscript: "ignored")
        let service = VoiceInputService(
            permissionClient: MockVoicePermissionClient(status: .granted),
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
            permissionClient: MockVoicePermissionClient(status: .granted),
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
    @Test("start waits for in-flight cancellation before re-recording")
    func startAfterCancelStartsStreamingAgain() async {
        let engine = SlowCancelVoiceInputEngine()
        let service = VoiceInputService(
            permissionClient: MockVoicePermissionClient(status: .granted),
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
}

private struct MockVoicePermissionClient: VoiceInputPermissionClient {
    let status: VoiceInputService.PermissionStatus

    func authorizationStatus() async -> VoiceInputService.PermissionStatus {
        status
    }

    func requestAccess() async -> Bool {
        status == .granted
    }
}

private final class MockVoiceInputEngine: VoiceInputEngine, @unchecked Sendable {
    private let partials: [String]
    private let finalTranscript: String
    private var cancelCount = 0

    init(partials: [String], finalTranscript: String) {
        self.partials = partials
        self.finalTranscript = finalTranscript
    }

    func prepareModel(onProgress: @escaping @Sendable (Double?) -> Void) async throws {
        onProgress(0.4)
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
