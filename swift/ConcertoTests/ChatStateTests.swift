import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

@Suite("ChatState")
struct ChatStateTests {
    @MainActor
    @Test("send consumes text deltas once when replay includes duplicate seq")
    func sendDedupesDuplicateSeq() async {
        let service = MockChatService(
            streamPlans: [
                .init(events: [
                    .success(AgentSessionEventEnvelope(seq: 0, event: .turnStarted(turnId: "turn_1"))),
                    .success(AgentSessionEventEnvelope(seq: 1, event: .textDelta(turnId: "turn_1", content: "Hello"))),
                    .success(AgentSessionEventEnvelope(seq: 1, event: .textDelta(turnId: "turn_1", content: "Hello"))),
                    .success(AgentSessionEventEnvelope(seq: 2, event: .turnCompleted(turnId: "turn_1", status: "completed"))),
                ])
            ]
        )

        let state = ChatState(
            waveId: "wave-test",
            sessionConfig: AgentSessionConfig(step: "design", repoRoot: "/tmp/repo"),
            waveService: service,
            userDefaults: makeUserDefaults("dedupe")
        )
        await state.send("Hi")
        await waitUntil { state.turnState == .completed }

        let assistantMessages = messages(in: state, role: .assistant)
        #expect(assistantMessages.count == 1)
        #expect(assistantMessages[0].content == "Hello")
    }

    @MainActor
    @Test("send surfaces session errors and failed turns")
    func sendFailedTurn() async {
        let service = MockChatService(
            streamPlans: [
                .init(events: [
                    .success(AgentSessionEventEnvelope(seq: 0, event: .turnStarted(turnId: "turn_1"))),
                    .success(AgentSessionEventEnvelope(seq: 1, event: .error(code: "oops", message: "bad thing"))),
                    .success(AgentSessionEventEnvelope(seq: 2, event: .turnCompleted(turnId: "turn_1", status: "failed"))),
                ])
            ]
        )

        let state = ChatState(
            waveId: "wave-test",
            sessionConfig: AgentSessionConfig(step: "design", repoRoot: "/tmp/repo"),
            waveService: service,
            userDefaults: makeUserDefaults("failed-turn")
        )
        await state.send("Hi")
        await waitUntil { state.turnState == .failed }

        let errorMessages = messages(in: state, role: .error)
        #expect(errorMessages.count == 1)
        #expect(errorMessages[0].content == "bad thing")
    }

    @MainActor
    @Test("session is created once and reused across sends")
    func sessionReusedAcrossTurns() async {
        let service = MockChatService(
            streamPlans: [
                .init(events: [
                    .success(AgentSessionEventEnvelope(seq: 0, event: .turnStarted(turnId: "turn_1"))),
                    .success(AgentSessionEventEnvelope(seq: 1, event: .textDelta(turnId: "turn_1", content: "reply one"))),
                    .success(AgentSessionEventEnvelope(seq: 2, event: .turnCompleted(turnId: "turn_1", status: "completed"))),
                ]),
                .init(events: [
                    .success(AgentSessionEventEnvelope(seq: 3, event: .turnStarted(turnId: "turn_2"))),
                    .success(AgentSessionEventEnvelope(seq: 4, event: .textDelta(turnId: "turn_2", content: "reply two"))),
                    .success(AgentSessionEventEnvelope(seq: 5, event: .turnCompleted(turnId: "turn_2", status: "completed"))),
                ]),
            ]
        )

        let state = ChatState(
            waveId: "wave-test",
            sessionConfig: AgentSessionConfig(step: "design", repoRoot: "/tmp/repo"),
            waveService: service,
            userDefaults: makeUserDefaults("reuse-session")
        )
        await state.send("one")
        await waitUntil { state.turnState == .completed }
        await state.send("two")
        await waitUntil { state.turnState == .completed }

        #expect(service.createSessionCallCount == 1)
    }

    @MainActor
    @Test("send waits for session to become active before posting input")
    func sendWaitsForActiveSession() async {
        let service = MockChatService(
            createSessionResults: [.success(startingSession(id: "session_1"))],
            getSessionResults: [
                .success(startingSession(id: "session_1")),
                .success(activeSession(id: "session_1")),
            ],
            streamPlans: [
                .init(events: [
                    .success(AgentSessionEventEnvelope(seq: 0, event: .turnStarted(turnId: "turn_1"))),
                    .success(AgentSessionEventEnvelope(seq: 1, event: .turnCompleted(turnId: "turn_1", status: "completed"))),
                ])
            ]
        )

        let state = ChatState(waveId: "wave-test", sessionConfig: AgentSessionConfig(step: "design", repoRoot: "/tmp/repo"), waveService: service, userDefaults: makeUserDefaults("wait-active"))
        await state.send("hello")
        await waitUntil { state.turnState == .completed }

        #expect(service.sendInputCallCount == 1)
        #expect(service.getSessionCallCount >= 2)
        #expect(messages(in: state, role: .error).isEmpty)
    }

    @MainActor
    @Test("item update before start is ignored")
    func ignoresOutOfOrderItemUpdate() async {
        let service = MockChatService(
            streamPlans: [
                .init(events: [
                    .success(AgentSessionEventEnvelope(seq: 0, event: .turnStarted(turnId: "turn_1"))),
                    .success(AgentSessionEventEnvelope(seq: 1, event: .itemUpdated(
                        turnId: "turn_1",
                        itemId: "cmd_1",
                        delta: .output(content: "ignored")
                    ))),
                    .success(AgentSessionEventEnvelope(seq: 2, event: .itemStarted(
                        turnId: "turn_1",
                        item: .command(CommandItem(id: "cmd_1", command: ["git", "status"], status: .inProgress))
                    ))),
                    .success(AgentSessionEventEnvelope(seq: 3, event: .turnCompleted(turnId: "turn_1", status: "completed"))),
                ])
            ]
        )

        let state = ChatState(waveId: "wave-test", sessionConfig: AgentSessionConfig(step: "design", repoRoot: "/tmp/repo"), waveService: service, userDefaults: makeUserDefaults("out-of-order"))
        await state.send("check")
        await waitUntil { state.turnState == .completed }

        let items = itemEntries(in: state)
        #expect(items.count == 1)
        #expect(items[0].itemId == "cmd_1")
    }

    @MainActor
    @Test("multiple updates for one item keep one transcript row")
    func keepsStableIdentityForItemUpdates() async {
        let service = MockChatService(
            streamPlans: [
                .init(events: [
                    .success(AgentSessionEventEnvelope(seq: 0, event: .turnStarted(turnId: "turn_1"))),
                    .success(AgentSessionEventEnvelope(seq: 1, event: .itemStarted(
                        turnId: "turn_1",
                        item: .command(CommandItem(id: "cmd_1", command: ["git", "status"], status: .inProgress))
                    ))),
                    .success(AgentSessionEventEnvelope(seq: 2, event: .itemUpdated(
                        turnId: "turn_1",
                        itemId: "cmd_1",
                        delta: .output(content: "M src/main.rs\n")
                    ))),
                    .success(AgentSessionEventEnvelope(seq: 3, event: .itemUpdated(
                        turnId: "turn_1",
                        itemId: "cmd_1",
                        delta: .output(content: "M tests/test.rs")
                    ))),
                    .success(AgentSessionEventEnvelope(seq: 4, event: .itemCompleted(
                        turnId: "turn_1",
                        item: .command(CommandItem(
                            id: "cmd_1",
                            command: ["git", "status"],
                            status: .completed,
                            output: "M src/main.rs\nM tests/test.rs"
                        ))
                    ))),
                    .success(AgentSessionEventEnvelope(seq: 5, event: .turnCompleted(turnId: "turn_1", status: "completed"))),
                ])
            ]
        )

        let state = ChatState(waveId: "wave-test", sessionConfig: AgentSessionConfig(step: "design", repoRoot: "/tmp/repo"), waveService: service, userDefaults: makeUserDefaults("stable-item-id"))
        await state.send("run")
        await waitUntil { state.turnState == .completed }

        let items = itemEntries(in: state)
        #expect(items.count == 1)
        #expect(items[0].card.status == .completed)
        #expect(items[0].card.detail?.contains("tests/test.rs") == true)
    }

    @MainActor
    @Test("item completion without start still creates one row")
    func completionWithoutStartCreatesRow() async {
        let service = MockChatService(
            streamPlans: [
                .init(events: [
                    .success(AgentSessionEventEnvelope(seq: 0, event: .turnStarted(turnId: "turn_1"))),
                    .success(AgentSessionEventEnvelope(seq: 1, event: .itemCompleted(
                        turnId: "turn_1",
                        item: .tool(ToolItem(id: "tool_1", name: "Read", status: .completed, output: "done"))
                    ))),
                    .success(AgentSessionEventEnvelope(seq: 2, event: .turnCompleted(turnId: "turn_1", status: "completed"))),
                ])
            ]
        )

        let state = ChatState(waveId: "wave-test", sessionConfig: AgentSessionConfig(step: "design", repoRoot: "/tmp/repo"), waveService: service, userDefaults: makeUserDefaults("complete-no-start"))
        await state.send("run")
        await waitUntil { state.turnState == .completed }

        let items = itemEntries(in: state)
        #expect(items.count == 1)
        #expect(items[0].itemId == "tool_1")
        #expect(items[0].card.status == .completed)
    }

    @MainActor
    @Test("command output detail is truncated to bounded size")
    func truncatesLargeDetail() async {
        let largeOutput = String(repeating: "x", count: 20_000)
        let service = MockChatService(
            streamPlans: [
                .init(events: [
                    .success(AgentSessionEventEnvelope(seq: 0, event: .turnStarted(turnId: "turn_1"))),
                    .success(AgentSessionEventEnvelope(seq: 1, event: .itemStarted(
                        turnId: "turn_1",
                        item: .command(CommandItem(id: "cmd_1", command: ["cat"], status: .inProgress))
                    ))),
                    .success(AgentSessionEventEnvelope(seq: 2, event: .itemUpdated(
                        turnId: "turn_1",
                        itemId: "cmd_1",
                        delta: .output(content: largeOutput)
                    ))),
                    .success(AgentSessionEventEnvelope(seq: 3, event: .turnCompleted(turnId: "turn_1", status: "completed"))),
                ])
            ]
        )

        let state = ChatState(waveId: "wave-test", sessionConfig: AgentSessionConfig(step: "design", repoRoot: "/tmp/repo"), waveService: service, userDefaults: makeUserDefaults("detail-cap"))
        await state.send("run")
        await waitUntil { state.turnState == .completed }

        let items = itemEntries(in: state)
        let detail = items.first?.card.detail ?? ""
        #expect(detail.count <= 16_000)
        #expect(detail.hasSuffix("…truncated"))
    }

    @MainActor
    @Test("send is rejected while reconnecting")
    func sendRejectedDuringReconnect() async {
        let defaults = makeUserDefaults("reconnect-race")
        defaults.set("session_1", forKey: "chatSession.wave-test")

        let service = MockChatService(
            getSessionResults: [.success(activeSession(id: "session_1"))],
            streamPlans: [.init(events: [], holdOpen: true)]
        )

        let state = ChatState(waveId: "wave-test", sessionConfig: AgentSessionConfig(step: "design", repoRoot: "/tmp/repo"), waveService: service, userDefaults: defaults)
        await state.reconnectIfNeeded()
        await state.send("hello")

        #expect(service.sendInputCallCount == 0)
        #expect(messages(in: state, role: .system).contains { $0.content.contains("Please wait") })

        state.onDisappear()
    }

    @MainActor
    @Test("stale persisted session is cleared")
    func stalePersistedSessionCleared() async {
        let defaults = makeUserDefaults("stale-session")
        defaults.set("session_1", forKey: "chatSession.wave-test")

        let service = MockChatService(
            getSessionResults: [.success(endedSession(id: "session_1"))]
        )

        let state = ChatState(waveId: "wave-test", sessionConfig: AgentSessionConfig(step: "design", repoRoot: "/tmp/repo"), waveService: service, userDefaults: defaults)
        await state.reconnectIfNeeded()

        #expect(defaults.string(forKey: "chatSession.wave-test") == nil)
        #expect(state.streamPhase == .idle)
    }

    @MainActor
    @Test("end session stops once, cancels stream, and appends lifecycle message")
    func endSessionStopsAndCancelsStream() async {
        let defaults = makeUserDefaults("end-session")
        let service = MockChatService(streamPlans: [.init(events: [], holdOpen: true)])
        let state = ChatState(waveId: "wave-test", sessionConfig: AgentSessionConfig(step: "design", repoRoot: "/tmp/repo"), waveService: service, userDefaults: defaults)

        await state.send("hello")
        await waitUntil { service.sendInputCallCount == 1 }
        await state.endSession()

        #expect(service.stopSessionCallCount == 1)
        #expect(service.streamCancellationCount >= 1)
        #expect(messages(in: state, role: .system).contains { $0.content == "Session ended" })
    }

    @MainActor
    @Test("session creation uses configured provider and config")
    func sendUsesConfiguredSessionParameters() async {
        let service = MockChatService(
            streamPlans: [
                .init(events: [
                    .success(AgentSessionEventEnvelope(seq: 0, event: .turnStarted(turnId: "turn_1"))),
                    .success(AgentSessionEventEnvelope(seq: 1, event: .turnCompleted(turnId: "turn_1", status: "completed"))),
                ])
            ]
        )
        let config = AgentSessionConfig(
            step: "review",
            repoRoot: "/tmp/repo",
            directions: ["designer"],
            area: "swift/Concerto",
            wave: "wavemodel",
            message: "Focus on UX",
            model: "claude-sonnet-4-6",
            cwd: "/tmp/repo/swift",
            maxTurns: 2,
            yoloMode: true
        )
        let state = ChatState(
            waveId: "wave-test",
            sessionProvider: "codex",
            sessionWaveRunId: "run_123",
            sessionConfig: config,
            waveService: service,
            userDefaults: makeUserDefaults("config-passthrough")
        )

        await state.send("Start")
        await waitUntil { state.turnState == .completed }

        #expect(service.lastCreateSessionProvider == "codex")
        #expect(service.lastCreateSessionWaveRunId == "run_123")
        #expect(service.lastCreateSessionConfig == config)
    }
}

private struct StreamPlan {
    let events: [Result<AgentSessionEventEnvelope, Error>]
    let holdOpen: Bool

    init(events: [Result<AgentSessionEventEnvelope, Error>], holdOpen: Bool = false) {
        self.events = events
        self.holdOpen = holdOpen
    }
}

private final class MockChatService: ChatService, @unchecked Sendable {
    private let queue = DispatchQueue(label: "MockChatService")
    private var createSessionResults: [Result<AgentSession, Error>]
    private var streamPlans: [StreamPlan]
    private var getSessionResults: [Result<AgentSession, Error>]

    private(set) var createSessionCallCount = 0
    private(set) var getSessionCallCount = 0
    private(set) var sendInputCallCount = 0
    private(set) var stopSessionCallCount = 0
    private(set) var streamCancellationCount = 0
    private(set) var lastCreateSessionProvider: String?
    private(set) var lastCreateSessionWaveRunId: String?
    private(set) var lastCreateSessionConfig: AgentSessionConfig?

    init(
        createSessionResults: [Result<AgentSession, Error>] = [],
        getSessionResults: [Result<AgentSession, Error>] = [],
        streamPlans: [StreamPlan] = []
    ) {
        self.createSessionResults = createSessionResults
        self.getSessionResults = getSessionResults
        self.streamPlans = streamPlans
    }

    func createSession(
        provider: String,
        waveRunId: String?,
        config: AgentSessionConfig
    ) async throws -> AgentSession {
        let result = queue.sync { () -> Result<AgentSession, Error> in
            createSessionCallCount += 1
            lastCreateSessionProvider = provider
            lastCreateSessionWaveRunId = waveRunId
            lastCreateSessionConfig = config
            if createSessionResults.isEmpty {
                return .success(activeSession(id: "session_1"))
            }
            return createSessionResults.removeFirst()
        }

        switch result {
        case .success(let session):
            return session
        case .failure(let error):
            throw error
        }
    }

    func getSession(_ id: String) async throws -> AgentSession {
        let result = queue.sync { () -> Result<AgentSession, Error> in
            getSessionCallCount += 1
            if getSessionResults.isEmpty {
                return .success(activeSession(id: id))
            }
            return getSessionResults.removeFirst()
        }

        switch result {
        case .success(let session):
            return session
        case .failure(let error):
            throw error
        }
    }

    func sendSessionInput(sessionId: String, content: String) async throws -> AgentSession {
        queue.sync { sendInputCallCount += 1 }
        return activeSession(id: sessionId)
    }

    func streamSessionEvents(
        sessionId: String,
        afterSeq: Int?
    ) -> AsyncThrowingStream<AgentSessionEventEnvelope, Error> {
        let plan = queue.sync {
            streamPlans.isEmpty ? StreamPlan(events: [], holdOpen: false) : streamPlans.removeFirst()
        }

        return AsyncThrowingStream { continuation in
            continuation.onTermination = { [weak self] reason in
                if case .cancelled = reason {
                    self?.queue.sync {
                        self?.streamCancellationCount += 1
                    }
                }
            }

            for event in plan.events {
                switch event {
                case .success(let envelope):
                    continuation.yield(envelope)
                case .failure(let error):
                    continuation.finish(throwing: error)
                    return
                }
            }

            if !plan.holdOpen {
                continuation.finish()
            }
        }
    }

    func stopSession(_ id: String) async throws -> AgentSession {
        queue.sync { stopSessionCallCount += 1 }
        return endedSession(id: id)
    }
}

private func activeSession(id: String) -> AgentSession {
    session(id: id, status: "active")
}

private func startingSession(id: String) -> AgentSession {
    session(id: id, status: "starting")
}

private func endedSession(id: String) -> AgentSession {
    session(id: id, status: "ended", endedAt: Date())
}

private func session(id: String, status: String, endedAt: Date? = nil) -> AgentSession {
    AgentSession(
        id: id,
        provider: "claude",
        status: status,
        waveRunId: nil,
        providerSessionId: nil,
        config: AgentSessionConfig(step: "design", repoRoot: "/tmp/repo"),
        createdAt: nil,
        endedAt: endedAt
    )
}

private func makeUserDefaults(_ suffix: String) -> UserDefaults {
    let suiteName = "ChatStateTests.\(suffix).\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: suiteName)!
    defaults.removePersistentDomain(forName: suiteName)
    return defaults
}

@MainActor
private func waitUntil(
    timeoutNanos: UInt64 = 2_000_000_000,
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

@MainActor
private func messages(in state: ChatState, role: ChatRole? = nil) -> [ChatMessage] {
    let all = state.transcript.compactMap { entry -> ChatMessage? in
        guard case .message(let message) = entry else { return nil }
        return message
    }

    guard let role else { return all }
    return all.filter { $0.role == role }
}

@MainActor
private func itemEntries(in state: ChatState) -> [TranscriptItem] {
    state.transcript.compactMap { entry -> TranscriptItem? in
        guard case .item(let item) = entry else { return nil }
        return item
    }
}
