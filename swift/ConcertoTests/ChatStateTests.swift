import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

@Suite("ChatState")
struct ChatStateTests {
    @Test("memory blocks stay sorted by position and name")
    func memoryBlocksAreSorted() {
        var memory = MemoryStore()
        memory.setBlocks([
            ChatMemoryBlock(name: "prefs", content: "Use <short> answers & bullets", position: 1),
            ChatMemoryBlock(name: "aaa", content: "same position", position: 1),
            ChatMemoryBlock(name: "context", content: "Project \"Loopflow\"", position: 0),
        ])

        #expect(memory.blocks.map(\.name) == ["context", "aaa", "prefs"])
    }

    @MainActor
    @Test("send consumes harness events and persists memory edits")
    func sendConsumesHarnessEvents() async {
        let service = MockChatService(
            eventBatches: [[
                .success(.message(content: "Working on it", phase: .progress)),
                .success(.memoryEdit(op: "upsert", block: "prefs", detail: "Answer in bullets")),
                .success(.message(content: "Done", phase: .final)),
                .success(.done),
            ]]
        )

        let state = ChatState(waveId: "wave-test", waveService: service)
        await state.send("Hello")

        #expect(state.turnState == .completed)
        #expect(state.messages.count == 4)
        #expect(state.messages[0].role == .user)
        #expect(state.messages[1].role == .assistant)
        #expect(state.messages[2].role == .memory)
        #expect(state.messages[2].content.contains("Agent updated memory"))
        #expect(state.messages[3].role == .assistant)
        #expect(state.memory.blocks.contains(where: { $0.name == "prefs" && $0.content == "Answer in bullets" }))
    }

    @MainActor
    @Test("send surfaces failed turn events from server")
    func sendServerFailedTurn() async {
        let service = MockChatService(
            eventBatches: [[
                .success(.failed(code: "missing_final_message", message: "Turn failed: expected exactly one final message.")),
            ]]
        )

        let state = ChatState(waveId: "wave-test", waveService: service)
        await state.send("Hello")

        #expect(state.turnState == .failed)
        #expect(state.messages.last?.role == .error)
        #expect(state.messages.last?.content.contains("expected exactly one final message") == true)
    }

    @MainActor
    @Test("loadMessagesIfNeeded populates messages from DB")
    func loadMessagesFromDB() async {
        let records = [
            ChatMessageRecord(id: "msg-1", role: "user", content: "Hello", createdAt: Date()),
            ChatMessageRecord(id: "msg-2", role: "assistant", content: "Hi there", createdAt: Date()),
        ]
        let service = MockChatService(messageRecords: records)

        let state = ChatState(waveId: "wave-test", waveService: service)
        #expect(state.messages.isEmpty)

        await state.loadMessagesIfNeeded()
        #expect(state.messages.count == 2)
        #expect(state.messages[0].role == .user)
        #expect(state.messages[0].content == "Hello")
        #expect(state.messages[1].role == .assistant)
        #expect(state.messages[1].content == "Hi there")

        // Second call is a no-op.
        await state.loadMessagesIfNeeded()
        #expect(state.messages.count == 2)
    }

    @MainActor
    @Test("send refreshes from DB when SSE stream fails")
    func sendRefreshesFromDBOnSSEFailure() async {
        let records = [
            ChatMessageRecord(id: "msg-1", role: "user", content: "Hello", createdAt: Date()),
            ChatMessageRecord(id: "msg-2", role: "assistant", content: "Hi there!", createdAt: Date()),
        ]
        let service = MockChatService(
            eventBatches: [[
                .failure(.commandFailed("chat turn already completed")),
            ]],
            messageRecords: records
        )

        let state = ChatState(waveId: "wave-test", waveService: service)
        await state.send("Hello")

        #expect(state.turnState == .completed)
        #expect(state.messages.count == 2)
        #expect(state.messages[0].role == .user)
        #expect(state.messages[0].content == "Hello")
        #expect(state.messages[1].role == .assistant)
        #expect(state.messages[1].content == "Hi there!")
    }

    @MainActor
    @Test("loadMemoryIfNeeded retries after a failed load")
    func loadMemoryRetriesAfterFailure() async {
        let service = MockChatService(
            listResults: [
                .failure(.commandFailed("temporary failure")),
                .success([ChatMemoryBlock(name: "context", content: "Loaded", position: 0)]),
            ]
        )

        let state = ChatState(waveId: "wave-test", waveService: service)

        await state.loadMemoryIfNeeded()
        #expect(state.memory.blocks.isEmpty)
        #expect(state.memoryError == "temporary failure")

        await state.loadMemoryIfNeeded()
        #expect(state.memory.blocks.count == 1)
        #expect(state.memoryError == nil)

        let calls = service.listCallCount()
        #expect(calls == 2)
    }
}

private final class MockChatService: ChatService, @unchecked Sendable {
    private let queue = DispatchQueue(label: "MockChatService")
    private var listResults: [Result<[ChatMemoryBlock], WaveServiceError>]
    private var blocks: [ChatMemoryBlock] = []
    private var listCalls = 0
    private var eventBatches: [[Result<ChatTurnEvent, WaveServiceError>]]
    private var messageRecords: [ChatMessageRecord]

    init(
        listResults: [Result<[ChatMemoryBlock], WaveServiceError>] = [.success([])],
        eventBatches: [[Result<ChatTurnEvent, WaveServiceError>]] = [],
        messageRecords: [ChatMessageRecord] = []
    ) {
        self.listResults = listResults
        self.eventBatches = eventBatches
        self.messageRecords = messageRecords
    }

    func listMemoryBlocks(waveId: String) async throws -> [ChatMemoryBlock] {
        try queue.sync {
            listCalls += 1
            if !listResults.isEmpty {
                let next = listResults.removeFirst()
                switch next {
                case .success(let nextBlocks):
                    blocks = nextBlocks
                    return nextBlocks
                case .failure(let error):
                    throw error
                }
            }
            return blocks
        }
    }

    func upsertMemoryBlock(
        waveId: String,
        name: String,
        content: String,
        position: Int?
    ) async throws -> ChatMemoryBlock {
        queue.sync {
            let block = ChatMemoryBlock(
                name: name,
                content: content,
                position: position ?? blocks.count
            )
            blocks.removeAll { $0.name == name }
            blocks.append(block)
            return block
        }
    }

    func deleteMemoryBlock(waveId: String, name: String) async throws {
        queue.sync {
            blocks.removeAll { $0.name == name }
        }
    }

    func listChatMessages(waveId: String) async throws -> [ChatMessageRecord] {
        queue.sync { messageRecords }
    }

    func startChat(
        waveId: String,
        message: String
    ) async throws {}

    func streamChatEvents(
        waveId: String
    ) -> AsyncThrowingStream<ChatTurnEvent, Error> {
        let batch = queue.sync {
            eventBatches.isEmpty ? [] : eventBatches.removeFirst()
        }
        return AsyncThrowingStream { continuation in
            for event in batch {
                switch event {
                case .success(let item):
                    continuation.yield(item)
                case .failure(let error):
                    continuation.finish(throwing: error)
                    return
                }
            }
            continuation.finish()
        }
    }

    func listCallCount() -> Int {
        queue.sync { listCalls }
    }
}
