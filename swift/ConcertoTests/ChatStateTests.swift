import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

@Suite("ChatState")
struct ChatStateTests {
    @Test("memory system prompt keeps position order and XML-escapes content")
    func memoryPrompt() {
        var memory = MemoryStore()
        memory.setBlocks([
            ChatMemoryBlock(name: "prefs", content: "Use <short> answers & bullets", position: 1),
            ChatMemoryBlock(name: "context", content: "Project \"Loopflow\"", position: 0),
        ])

        let prompt = memory.systemPrompt()
        #expect(prompt.contains("<block name=\"context\">"))
        #expect(prompt.contains("Project &quot;Loopflow&quot;"))
        #expect(prompt.contains("Use &lt;short&gt; answers &amp; bullets"))
        let contextIndex = prompt.range(of: "<block name=\"context\">")?.lowerBound
        let prefsIndex = prompt.range(of: "<block name=\"prefs\">")?.lowerBound
        #expect(contextIndex != nil)
        #expect(prefsIndex != nil)
        if let contextIndex, let prefsIndex {
            #expect(contextIndex < prefsIndex)
        }
    }

    @MainActor
    @Test("send appends error bubble when API key is missing")
    func sendWithoutAPIKey() async {
        let state = ChatState(
            waveId: "wave-test",
            anthropic: AnthropicClient(apiKey: nil)
        )

        await state.send("Hello")

        #expect(state.messages.count == 2)
        #expect(state.messages.first?.role == .user)
        #expect(state.messages.last?.role == .error)
    }

    @MainActor
    @Test("loadMemoryIfNeeded retries after a failed load")
    func loadMemoryRetriesAfterFailure() async {
        let service = MockChatMemoryService(
            listResults: [
                .failure(.commandFailed("temporary failure")),
                .success([ChatMemoryBlock(name: "context", content: "Loaded", position: 0)]),
            ]
        )
        let state = ChatState(
            waveId: "wave-test",
            waveService: service,
            anthropic: AnthropicClient(apiKey: nil)
        )

        await state.loadMemoryIfNeeded()
        #expect(state.memory.blocks.isEmpty)
        #expect(state.memoryError == "temporary failure")

        await state.loadMemoryIfNeeded()
        #expect(state.memory.blocks.count == 1)
        #expect(state.memoryError == nil)

        let calls = await service.listCallCount()
        #expect(calls == 2)
    }
}

private actor MockChatMemoryService: ChatMemoryService {
    private var listResults: [Result<[ChatMemoryBlock], WaveServiceError>]
    private var blocks: [ChatMemoryBlock] = []
    private var listCalls = 0

    init(listResults: [Result<[ChatMemoryBlock], WaveServiceError>]) {
        self.listResults = listResults
    }

    func listMemoryBlocks(waveId: String) async throws -> [ChatMemoryBlock] {
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

    func upsertMemoryBlock(
        waveId: String,
        name: String,
        content: String,
        position: Int?
    ) async throws -> ChatMemoryBlock {
        let block = ChatMemoryBlock(
            name: name,
            content: content,
            position: position ?? blocks.count
        )
        blocks.removeAll { $0.name == name }
        blocks.append(block)
        return block
    }

    func deleteMemoryBlock(waveId: String, name: String) async throws {
        blocks.removeAll { $0.name == name }
    }

    func listCallCount() -> Int {
        listCalls
    }
}
