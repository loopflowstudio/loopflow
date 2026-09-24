#if os(macOS)
import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@MainActor
@Suite("Task Watch output presentation")
struct TaskWatchOutputTests {
    @Test("Follow tracks arrivals in an earlier Run and quiet pages do not move it")
    func followsUpdatedRun() async throws {
        let store = TaskWatchStore()
        let original = try fixture()
        await store.readOutput(issue: "LOO-293", query: query(original))
        let first = try #require(store.output.first?.id)
        #expect(store.latestOutputRow?.source == store.output[1].id)

        var changed = try #require(JSONSerialization.jsonObject(with: original) as? [String: Any])
        var sources = try #require(changed["sources"] as? [[String: Any]])
        sources[0]["records"] = [["source_item_id": "new-message", "revision": "1", "event": [
            "type": "text_delta", "turn_id": "turn", "content": "New output from the first Run"
        ]]]
        sources[1]["records"] = []
        changed["sources"] = sources
        await store.readOutput(issue: "LOO-293", query: query(try JSONSerialization.data(withJSONObject: changed)))
        #expect(store.latestOutputRow?.source == first)
        #expect(store.output.first?.rows.last?.text == "New output from the first Run")
        sources[0]["records"] = []
        changed["sources"] = sources
        await store.readOutput(issue: "LOO-293", query: query(try JSONSerialization.data(withJSONObject: changed)))
        #expect(store.latestOutputRow?.source == first)

        store.inspectRun("missing-run")
        #expect(store.visibleOutput.isEmpty)
        let view = TaskWatchOutputView(store: store, onHistory: { _ in }, onFollow: { store.followLive() }, onReload: {})
        try view.inspect().find(button: "Follow live").tap()
        #expect(store.visibleOutput.count == 3)
        #expect(store.followsOutput)
    }

    @Test("A completed tool snapshot replaces its accumulated output without replay")
    func completedTool() {
        let records = [
            OutputRecord(sourceItemId: "start", revision: "1", event: .itemStarted(turnId: "turn", item: .tool(id: "tool", name: "bash", status: .running, input: nil, output: nil))),
            OutputRecord(sourceItemId: "delta", revision: "1", event: .itemUpdated(turnId: "turn", itemId: "tool", data: .output("hello"))),
            OutputRecord(sourceItemId: "end", revision: "1", event: .itemCompleted(turnId: "turn", item: .tool(id: "tool", name: "bash", status: .completed, input: nil, output: "hello")))
        ]
        let source = TaskOutputSource(runId: "run", provider: "codex", stage: nil, records: records, source: .journal, available: true, hasMore: false, reset: false, gaps: [])
        var observation = 0
        var output = TaskWatchOutput(source: source)
        output.merge(source, history: false, observation: &observation)
        output.merge(source, history: true, observation: &observation)
        #expect(output.rows.count == 1)
        #expect(output.rows.first?.text == "hello")
        #expect(output.rows.first?.title == "bash · completed")
    }

    @Test("Native tool results retain their exact call name and input", arguments: [OutputSource.claude, .codex])
    func nativeToolResults(source: OutputSource) {
        func page(_ records: [OutputRecord]) -> TaskOutputSource {
            TaskOutputSource(runId: "run", provider: source.rawValue, stage: nil, records: records,
                             source: source, available: true, hasMore: false, reset: false, gaps: [])
        }
        let calls = page(["first", "second"].map { id in
            OutputRecord(sourceItemId: "call-\(id)", revision: "1", event: .itemCompleted(
                turnId: "assistant", item: .tool(id: id, name: "bash", status: .running,
                                                input: .string("echo \(id)"), output: nil)))
        })
        let results = page(["second", "first"].map { id in
            OutputRecord(sourceItemId: "result-\(id)", revision: "1", event: .itemCompleted(
                turnId: source == .claude ? "user-\(id)" : "assistant",
                item: .tool(id: id, name: "tool_result", status: id == "first" ? .failed : .completed,
                            input: .object(["call_id": .string(id)]), output: "result \(id)")))
        })
        var observation = 0
        var output = TaskWatchOutput(source: calls)
        output.merge(results, history: false, observation: &observation)
        output.merge(calls, history: true, observation: &observation)
        #expect(output.rows.map(\.title) == ["bash · failed", "bash · completed"])
        #expect(output.rows.map(\.text) == ["echo first\nresult first", "echo second\nresult second"])
        output.merge(results, history: true, observation: &observation)
        #expect(output.rows.count == 2)
    }

    private func fixture() throws -> Data {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        return try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/task_output.json"))
    }

    private func query(_ data: Data) -> RegistryQuery {
        RegistryQuery { _, _ in String(decoding: data, as: UTF8.self) }
    }
}
#endif
