#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@MainActor
@Suite("Task Watch feed")
struct TaskWatchFeedTests {
    @Test("Unread history cannot delay new output or overwrite a newer revision")
    func independentHistoryAndLive() async throws {
        let store = TaskWatchStore()
        let responses = [
            "tail": try page("live-1", sources: [source("run-first", records: [])]),
            "start": try page("history-1", sources: [source("run-first", records: [message("old", "Earlier output")])]),
            "live-1": try page("live-2", sources: [
                source("run-first", records: [message("mutable", "Latest revision", revision: "new")]),
                source("run-aux", records: [message("aux", "Concurrent auxiliary output")], step: nil)
            ]),
            "history-1": try page("history-2", sources: [source("run-first", records: [
                message("middle", "Middle history"), message("mutable", "Stale revision", revision: "old")
            ])]),
            "live-2": try page("live-3", sources: [source("run-first", records: [
                message("mutable", "Latest revision", revision: "new"), message("last", "Arrived after history")
            ])])
        ]
        let query = reader(responses)
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.output.first?.rows.map(\.text) == ["Earlier output"])
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.output.map { $0.source.runId } == ["run-first", "run-aux"])
        await store.readOutput(issue: "LOO-293", query: query, history: true)
        #expect(store.output.first?.rows.map(\.text) == ["Earlier output", "Middle history", "Latest revision"])
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.output.first?.rows.map(\.text) == ["Earlier output", "Middle history", "Latest revision", "Arrived after history"])
        #expect(store.output.last?.rows.map(\.text) == ["Concurrent auxiliary output"])
        #expect(store.outputError == nil)
    }

    @Test("Exact stage and Run filters survive transitions; Follow live rejoins every Run")
    func filtersAndFollowLive() async throws {
        let store = TaskWatchStore()
        await store.refresh(issue: "LOO-293", query: try snapshot(active: 0))
        let data = try page("cursor", sources: [
            source("run-first", records: [message("a", "First attempt")]),
            source("run-retry", records: [message("b", "Retry")]),
            source("run-review", records: [message("c", "Review")], step: 1),
            source("run-aux", records: [message("d", "Auxiliary")], step: nil)
        ])
        await store.readOutput(issue: "LOO-293", query: RegistryQuery { _, _ in data })
        store.inspectStage(0)
        #expect(store.visibleOutput.map { $0.source.runId } == ["run-first", "run-retry"])
        store.inspectRun("run-retry")
        await store.refresh(issue: "LOO-293", query: try snapshot(active: 1))
        #expect(store.stepIndex == 0)
        #expect(store.visibleOutput.map { $0.source.runId } == ["run-retry"])
        #expect(!store.followsOutput)
        store.followLive()
        #expect(store.stepIndex == 1)
        #expect(store.visibleOutput.count == 4)
        #expect(store.followsOutput)
        await store.refresh(issue: "LOO-293", query: try snapshot(active: 2))
        #expect(store.stepIndex == 2)
        #expect(store.visibleOutput.count == 4)
    }

    @Test("Source reset preserves evidence until both continuations are explicitly reloaded")
    func resetAndReload() async throws {
        let store = TaskWatchStore()
        let initial = try page("cursor", sources: [source("run-first", records: [message("a", "Original evidence")])])
        await store.readOutput(issue: "LOO-293", query: RegistryQuery { _, _ in initial })
        let reset = try page("replacement", sources: [source("run-first", records: [message("a", "Replacement")], reset: true)])
        await store.readOutput(issue: "LOO-293", query: RegistryQuery { _, _ in reset })
        #expect(store.needsOutputReload)
        #expect(store.outputError?.contains("source changed") == true)
        #expect(store.output.first?.rows.map(\.text) == ["Original evidence"])
        let reloaded = reader([
            "tail": try page("new-live", sources: []),
            "start": try page("new-history", sources: [source("run-first", records: [message("a", "Replacement")])])
        ])
        await store.readOutput(issue: "LOO-293", query: reloaded, reload: true)
        #expect(!store.needsOutputReload)
        #expect(store.outputError == nil)
        #expect(store.output.first?.rows.map(\.text) == ["Replacement"])
    }

    @Test("Failure and a superseded response cannot destroy newer displayed output")
    func staleAndSupersededReads() async throws {
        let store = TaskWatchStore()
        let data = try page("cursor", sources: [source("run-first", records: [message("a", "Retained")])])
        await store.readOutput(issue: "LOO-293", query: RegistryQuery { _, _ in data })
        await store.readOutput(issue: "LOO-293", query: RegistryQuery { _, _ in throw RegistryQueryError("offline") })
        #expect(store.outputError == "offline")
        #expect(store.output.first?.rows.map(\.text) == ["Retained"])

        let started = AsyncStream<Void>.makeStream()
        let response = AsyncStream<String>.makeStream()
        let slow = RegistryQuery { _, _ in
            started.continuation.yield(())
            var iterator = response.stream.makeAsyncIterator()
            return await iterator.next() ?? ""
        }
        let previous = Task { await store.readOutput(issue: "LOO-293", query: slow) }
        var starts = started.stream.makeAsyncIterator()
        await starts.next()
        let newer = try page("new", sources: [source("run-first", records: [message("a", "Newer")])])
        await store.readOutput(issue: "LOO-293", query: RegistryQuery { _, _ in newer })
        response.continuation.yield(data)
        await previous.value
        #expect(store.output.first?.rows.map(\.text) == ["Newer"])
        #expect(store.outputError == nil)
        #expect(!store.isReadingOutput)
        started.continuation.finish()
        response.continuation.finish()
    }

    @Test("Text before and after a command retains its source order")
    func interleavedTextAndTools() throws {
        let records: [[String: Any]] = [
            ["source_item_id": "1", "revision": "1", "event": ["type": "text_delta", "turn_id": "turn", "content": "Before command"]],
            ["source_item_id": "2", "revision": "1", "event": ["type": "item_completed", "turn_id": "turn", "item": [
                "type": "command", "id": "cmd", "command": ["echo", "ok"], "cwd": "/tmp", "status": "completed", "output": "ok", "exit_code": 0, "duration_ms": 1
            ]]],
            ["source_item_id": "3", "revision": "1", "event": ["type": "text_delta", "turn_id": "turn", "content": "After command"]]
        ]
        let page = try JSONDecoder().decode(TaskOutputPage.self, from: Data(page("next", sources: [source("run-first", records: records)]).utf8))
        let source = try #require(page.sources.first)
        var observation = 0
        var output = TaskWatchOutput(source: source)
        output.merge(source, history: false, observation: &observation)
        #expect(output.rows.count == 3)
        #expect(output.rows.first?.text == "Before command")
        #expect(output.rows.last?.text == "After command")
        #expect(output.rows.contains { $0.text.contains("echo ok") && $0.text.contains("/tmp") })
    }

    @Test("Hiding output cancels its read without accepting a late page")
    func cancelledReadRetainsEvidence() async throws {
        let store = TaskWatchStore()
        let data = try page("cursor", sources: [source("run-first", records: [message("a", "Retained")])])
        await store.readOutput(issue: "LOO-293", query: RegistryQuery { _, _ in data })
        let started = AsyncStream<Void>.makeStream()
        let response = AsyncStream<String>.makeStream()
        let slow = RegistryQuery { _, _ in
            started.continuation.yield(())
            var iterator = response.stream.makeAsyncIterator()
            return await iterator.next() ?? ""
        }
        let request = Task { await store.readOutput(issue: "LOO-293", query: slow) }
        var starts = started.stream.makeAsyncIterator()
        await starts.next()
        request.cancel()
        response.continuation.yield(try page("late", sources: [source("run-first", records: [message("a", "Late")])]))
        await request.value
        #expect(store.output.first?.rows.map(\.text) == ["Retained"])
        #expect(store.outputError == nil)
        #expect(!store.isReadingOutput)
        started.continuation.finish()
        response.continuation.finish()
    }

    @Test("A quiet historical page cannot hide a failed live start")
    func tailGapSurvivesHistory() async throws {
        let store = TaskWatchStore()
        let failedTail = #"{"task_id":"task-watch","sources":[],"gaps":[{"code":"tail_unavailable","message":"Live boundary unavailable; reading history"}],"next_cursor":"live"}"#
        let query = reader(["tail": failedTail, "start": try page("history", sources: [])])
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.outputGaps.contains { $0.code == "tail_unavailable" })
    }

    @Test("Alternating Runs stay in observation order, including split prose and late overlapping history")
    func observationBlocks() async throws {
        func delta(_ id: String, _ text: String, revision: String = "1") -> [String: Any] {
            ["source_item_id": id, "revision": revision, "event": [
                "type": "text_delta", "turn_id": "same-turn", "content": text
            ]]
        }
        let store = TaskWatchStore()
        let query = reader([
            "tail": try page("live-1", sources: []),
            "start": try page("history-1", sources: []),
            "live-1": try page("live-2", sources: [
                source("run-a", records: [delta("a1", "First arrival")]),
                source("run-b", records: [delta("a1", "Auxiliary arrival")], step: nil)
            ]),
            "live-2": try page("live-3", sources: [source("run-a", records: [
                delta("a2", "Next "), delta("a3", "part")
            ])]),
            "live-3": try page("live-3", sources: [source("run-a", records: [delta("a3", "part")])]),
            "history-1": try page("history-2", sources: [
                source("run-a", records: [message("old-a", "Earlier A"), delta("a1", "stale", revision: "old")]),
                source("run-b", records: [message("old-b", "Earlier B")], step: nil)
            ])
        ])
        await store.readOutput(issue: "LOO-293", query: query)
        await store.readOutput(issue: "LOO-293", query: query)
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.outputGroups.map { $0.source.runId } == ["run-a", "run-b", "run-a"])
        #expect(store.outputGroups.flatMap { $0.rows.map(\.text) } == ["First arrival", "Auxiliary arrival", "Next part"])
        #expect(store.outputGroups.allSatisfy { !$0.history })
        let latest = try #require(store.latestOutputRow)
        #expect(latest.row.item == "a2") // Adjacent deltas share one display row.
        let blocks = store.outputGroups.map(\.id)
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.outputGroups.map(\.id) == blocks)
        #expect(store.latestOutputRow == latest)

        await store.readOutput(issue: "LOO-293", query: query, history: true)
        #expect(store.outputGroups.map { $0.source.runId } == ["run-a", "run-b", "run-b", "run-a"])
        #expect(store.outputGroups.map(\.history) == [true, true, false, false])
        #expect(store.outputGroups.flatMap { $0.rows.map(\.text) } == [
            "Earlier A", "First arrival", "Earlier B", "Auxiliary arrival", "Next part"
        ])
        #expect(store.latestOutputRow == latest)
        store.inspectRun("run-a")
        #expect(store.outputGroups.map { $0.source.runId } == ["run-a", "run-a"])
        store.followLive()
        #expect(store.outputGroups.count == 4)
    }

    @Test("A late tool completion updates its original block and becomes the exact follow target")
    func followsToolRevision() async throws {
        func tool(_ status: String, id: String, text: String) -> [String: Any] {
            ["source_item_id": id, "revision": "1", "event": [
                "type": "item_completed", "turn_id": "turn", "item": [
                    "type": "tool", "id": "tool", "name": "bash", "status": status,
                    "input": "echo hello", "output": text
                ]
            ]]
        }
        let store = TaskWatchStore()
        let query = reader([
            "tail": try page("live-1", sources: []),
            "start": try page("history", sources: []),
            "live-1": try page("live-2", sources: [
                source("run-a", records: [tool("running", id: "start", text: "")]),
                source("run-b", records: [message("other", "Other Run")], step: nil)
            ]),
            "live-2": try page("live-3", sources: [source("run-a", records: [message("later", "Later prose")])]),
            "live-3": try page("live-4", sources: [source("run-a", records: [tool("completed", id: "end", text: "hello")])]),
            "live-4": try page("live-4", sources: [])
        ])
        await store.readOutput(issue: "LOO-293", query: query)
        await store.readOutput(issue: "LOO-293", query: query)
        await store.readOutput(issue: "LOO-293", query: query)
        let blocks = store.outputGroups.map(\.id)
        #expect(store.latestOutputRow?.row.item == "later")
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.outputGroups.map(\.id) == blocks)
        #expect(store.outputGroups.map { $0.source.runId } == ["run-a", "run-b", "run-a"])
        #expect(store.outputGroups.first?.rows.first?.title == "bash · completed")
        #expect(store.outputGroups.first?.rows.first?.text == "echo hello\nhello")
        #expect(store.latestOutputRow?.row.item == "tool")
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.latestOutputRow?.row.item == "tool")
    }

    private func reader(_ responses: [String: String]) -> RegistryQuery {
        RegistryQuery { args, _ in
            let position: String
            if args.contains("--tail") {
                position = "tail"
            } else if let index = args.firstIndex(of: "--cursor") {
                position = try String(contentsOfFile: args[index + 1], encoding: .utf8)
            } else {
                position = "start"
            }
            guard let value = responses[position] else { throw RegistryQueryError("No fixture at \(position)") }
            return value
        }
    }

    private func snapshot(active: Int) throws -> RegistryQuery {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        var value = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/task_watch.json"))) as? [String: Any])
        value["active_stage"] = ["invocation_id": "flow-one", "step_index": active, "iteration": 1]
        let data = String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        return RegistryQuery { _, _ in data }
    }

    private func page(_ cursor: String, sources: [[String: Any]]) throws -> String {
        String(decoding: try JSONSerialization.data(withJSONObject: [
            "task_id": "task-watch", "sources": sources, "gaps": [], "next_cursor": cursor
        ]), as: UTF8.self)
    }

    private func source(_ run: String, records: [[String: Any]], step: Int? = 0, reset: Bool = false) -> [String: Any] {
        ["run_id": run, "provider": "codex", "source": "journal", "available": true,
         "has_more": true, "reset": reset, "gaps": [], "records": records,
         "stage": step.map { ["invocation_id": "flow-one", "step_index": $0, "iteration": 1] as [String: Any] } as Any? ?? NSNull()]
    }

    private func message(_ id: String, _ text: String, revision: String = "1") -> [String: Any] {
        ["source_item_id": id, "revision": revision, "event": [
            "type": "item_completed", "turn_id": "turn", "item": ["type": "message", "id": id, "text": text, "phase": "commentary"]
        ]]
    }
}
#endif
