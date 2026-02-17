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
}
