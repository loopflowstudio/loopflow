import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

@Suite("ChatState")
struct ChatStateTests {
    @MainActor
    @Test("send consumes session text deltas and marks completion")
    func sendConsumesSessionEvents() async {
        let service = MockChatService(
            eventBatches: [[
                .success(AgentSessionEventEnvelope(seq: 0, event: .turnStarted(turnId: "turn_1"))),
                .success(AgentSessionEventEnvelope(seq: 1, event: .textDelta(turnId: "turn_1", content: "Hello"))),
                .success(AgentSessionEventEnvelope(seq: 2, event: .textDelta(turnId: "turn_1", content: " world"))),
                .success(AgentSessionEventEnvelope(seq: 3, event: .turnCompleted(turnId: "turn_1", status: "completed"))),
            ]]
        )

        let state = ChatState(waveId: "wave-test", waveService: service)
        await state.send("Hi")

        #expect(state.turnState == .completed)
        #expect(state.messages.count == 2)
        #expect(state.messages[0].role == .user)
        #expect(state.messages[0].content == "Hi")
        #expect(state.messages[1].role == .assistant)
        #expect(state.messages[1].content == "Hello world")
    }

    @MainActor
    @Test("send surfaces session errors and failed turns")
    func sendFailedTurn() async {
        let service = MockChatService(
            eventBatches: [[
                .success(AgentSessionEventEnvelope(seq: 0, event: .turnStarted(turnId: "turn_1"))),
                .success(AgentSessionEventEnvelope(seq: 1, event: .error(code: "oops", message: "bad thing"))),
                .success(AgentSessionEventEnvelope(seq: 2, event: .turnCompleted(turnId: "turn_1", status: "failed"))),
            ]]
        )

        let state = ChatState(waveId: "wave-test", waveService: service)
        await state.send("Hi")

        #expect(state.turnState == .failed)
        #expect(state.messages.count == 2)
        #expect(state.messages[1].role == .error)
        #expect(state.messages[1].content == "bad thing")
    }

    @MainActor
    @Test("session is created once and reused across sends")
    func sessionReusedAcrossTurns() async {
        let service = MockChatService(
            eventBatches: [
                [
                    .success(AgentSessionEventEnvelope(seq: 0, event: .turnStarted(turnId: "turn_1"))),
                    .success(AgentSessionEventEnvelope(seq: 1, event: .turnCompleted(turnId: "turn_1", status: "completed"))),
                ],
                [
                    .success(AgentSessionEventEnvelope(seq: 2, event: .turnStarted(turnId: "turn_2"))),
                    .success(AgentSessionEventEnvelope(seq: 3, event: .turnCompleted(turnId: "turn_2", status: "completed"))),
                ],
            ]
        )

        let state = ChatState(waveId: "wave-test", waveService: service)
        await state.send("one")
        await state.send("two")

        #expect(service.createSessionCallCount == 1)
        #expect(service.sendInputCallCount == 2)
    }
}

private final class MockChatService: ChatService, @unchecked Sendable {
    private let queue = DispatchQueue(label: "MockChatService")
    private var eventBatches: [[Result<AgentSessionEventEnvelope, WaveServiceError>]]

    private(set) var createSessionCallCount = 0
    private(set) var sendInputCallCount = 0

    init(eventBatches: [[Result<AgentSessionEventEnvelope, WaveServiceError>]] = []) {
        self.eventBatches = eventBatches
    }

    func createSession(
        provider: String,
        waveRunId: String?,
        config: AgentSessionConfig
    ) async throws -> AgentSession {
        queue.sync { createSessionCallCount += 1 }
        return AgentSession(
            id: "session_1",
            provider: provider,
            status: "active",
            waveRunId: waveRunId,
            providerSessionId: nil,
            config: config,
            createdAt: nil,
            endedAt: nil
        )
    }

    func sendSessionInput(sessionId: String, content: String) async throws -> AgentSession {
        queue.sync { sendInputCallCount += 1 }
        return AgentSession(
            id: sessionId,
            provider: "claude",
            status: "active",
            waveRunId: nil,
            providerSessionId: nil,
            config: AgentSessionConfig(),
            createdAt: nil,
            endedAt: nil
        )
    }

    func streamSessionEvents(
        sessionId: String,
        afterSeq: Int?
    ) -> AsyncThrowingStream<AgentSessionEventEnvelope, Error> {
        let batch = queue.sync {
            eventBatches.isEmpty ? [] : eventBatches.removeFirst()
        }

        return AsyncThrowingStream { continuation in
            for entry in batch {
                switch entry {
                case .success(let event):
                    continuation.yield(event)
                case .failure(let error):
                    continuation.finish(throwing: error)
                    return
                }
            }
            continuation.finish()
        }
    }
}
