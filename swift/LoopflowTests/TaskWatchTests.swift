#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@MainActor
@Suite("Task Watch")
struct TaskWatchTests {
    @Test("Diagram selects exact custom stages and recorded return targets without inferring completion")
    func diagramNavigation() async throws {
        var data = try #require(JSONSerialization.jsonObject(with: fixture()) as? [String: Any])
        var invocations = try #require(data["invocations"] as? [[String: Any]])
        var stages = try #require(invocations[0]["stages"] as? [[String: Any]])
        stages[1]["name"] = "implement"
        invocations[0]["stages"] = stages
        data["invocations"] = invocations
        let store = TaskWatchStore()
        await store.refresh(issue: "LOO-293", query: query(try JSONSerialization.data(withJSONObject: data)))
        let diagram = TaskWatchDiagram(store: store)
        _ = try diagram.inspect().find(text: "custom › slice")
        _ = try diagram.inspect().find(text: "Human checkpoint · iterated · 1 attempt")
        _ = try diagram.inspect().find(text: "op · Not entered · 0 attempts")
        try diagram.inspect().find(button: "2. implement").tap()
        #expect(store.stepIndex == 1)
        #expect(store.stage?.attempts.first?.runId == "run-review")
        #expect(store.filtersStage && !store.followsOutput)
        try diagram.inspect().find(button: "2. implement").callOnMoveCommand(.down)
        #expect(store.stepIndex == 2)
        try diagram.inspect().find(button: "3. pr publish").callOnMoveCommand(.up)
        #expect(store.stepIndex == 1)
        try diagram.inspect().find(button: "Iterated → stage 1 · iteration 1").tap()
        #expect(store.stepIndex == 0)
        #expect(store.stage?.attempts.map(\.runId) == ["run-first", "run-failed", "run-retry"])
        try diagram.inspect().find(button: "Retried → stage 1 · iteration 1").tap()
        #expect(store.stage?.attempts.count == 3)

        data["active_stage"] = NSNull()
        invocations[0]["settlement"] = "completed"
        data["invocations"] = invocations
        await store.refresh(issue: "LOO-293", query: query(try JSONSerialization.data(withJSONObject: data)))
        #expect(store.invocation?.settlement == .completed)
        _ = try diagram.inspect().find(text: "op · Not entered · 0 attempts")
        try diagram.inspect().find(button: "3. pr publish").tap()
        #expect(store.stage?.attempts.isEmpty == true)
        store.selectInvocation("old-flow")
        _ = try diagram.inspect().find(text: "No retained stages")
    }

    @Test("Inspection survives transitions, repeated names, completion, and failed refreshes")
    func selectionAndStaleEvidence() async throws {
        let store = TaskWatchStore()
        let initial = try fixture()
        await store.refresh(issue: "LOO-293", query: query(initial))
        #expect(store.invocationId == "flow-one")
        #expect(store.stepIndex == 0)
        #expect(store.stage?.attempts.count == 3)
        store.inspectStage(1)

        var next = try #require(JSONSerialization.jsonObject(with: initial) as? [String: Any])
        next["active_stage"] = ["invocation_id": "flow-one", "step_index": 2, "iteration": 1]
        var invocations = try #require(next["invocations"] as? [[String: Any]])
        var stages = try #require(invocations[0]["stages"] as? [[String: Any]])
        stages[1]["name"] = "implement" // Same name, different immutable stage.
        invocations[0]["stages"] = stages
        next["invocations"] = invocations
        await store.refresh(issue: "LOO-293", query: query(try JSONSerialization.data(withJSONObject: next)))
        #expect(store.stepIndex == 1)
        #expect(store.stage?.human == true)
        #expect(store.stage?.attempts.first?.runId == "run-review")

        let failed = RegistryQuery { _, _ in throw RegistryQueryError("registry unavailable") }
        await store.refresh(issue: "LOO-293", query: failed)
        #expect(store.error == "registry unavailable")
        #expect(store.snapshot?.runs.count == 3)
        #expect(store.stepIndex == 1)
        let staleView = TaskWatchView(issue: "LOO-293", store: store)
        _ = try staleView.inspect().find(text: "Refresh failed — showing the last snapshot")
        let edge = try #require(store.invocation?.transitions.first { $0.reason == .iterated })
        _ = try staleView.inspect().find(button: "Iterated: stage 2, iteration 0 → stage 1, iteration 1")
        store.selectStage(edge.to)
        #expect(store.stepIndex == 0)
        #expect(store.stage?.attempts.map(\.runId) == ["run-first", "run-failed", "run-retry"])

        next["active_stage"] = NSNull()
        invocations[0]["settlement"] = "completed"
        next["invocations"] = invocations
        await store.refresh(issue: "LOO-293", query: query(try JSONSerialization.data(withJSONObject: next)))
        #expect(store.error == nil)
        #expect(store.snapshot?.activeStage == nil)
        #expect(store.stepIndex == 0)
        #expect(store.invocation?.settlement == .completed)
        store.selectInvocation("old-flow")
        await store.refresh(issue: "LOO-293", query: query(initial))
        #expect(store.invocationId == "old-flow")
        #expect(store.stepIndex == nil)
    }

    @Test("Watch remains reachable without Task runtime or a worktree")
    func workspaceWithoutRuntime() throws {
        let task = TaskPlanningSnapshot(
            id: "issue-id", identifier: "LOO-293", name: "Retained history",
            description: "", rank: 1, completed: true, assignee: nil
        )
        let view = TaskWorkspaceView(
            task: task,
            reference: TaskReferenceSnapshot(issueUrl: nil, workspace: nil),
            runtime: nil,
            repoPath: "/absent/repo",
            terminalStore: TaskTerminalStore(),
            initialSection: .watch
        )
        _ = try view.inspect().find(TaskWatchView.self)
    }

    @Test("Reopening Watch reads again while the hidden query is still finishing")
    func reopeningDuringRead() async throws {
        let store = TaskWatchStore()
        let started = AsyncStream<Void>.makeStream()
        let response = AsyncStream<String>.makeStream()
        let slow = RegistryQuery { _, _ in
            started.continuation.yield(())
            var values = response.stream.makeAsyncIterator()
            return await values.next() ?? ""
        }
        let previous = Task { await store.refresh(issue: "LOO-293", query: slow) }
        var starts = started.stream.makeAsyncIterator()
        await starts.next()
        #expect(store.isRefreshing)
        await store.refresh(issue: "LOO-293", query: query(try fixture()))
        #expect(store.snapshot?.runs.count == 3)
        #expect(!store.isRefreshing)
        response.continuation.yield("invalid old response")
        await previous.value
        #expect(store.error == nil)
        #expect(store.snapshot?.runs.count == 3)
        #expect(!store.isRefreshing)
        started.continuation.finish()
        response.continuation.finish()
    }

    @Test("Hiding a query cancels its subprocess, including cancellation before spawn")
    func queryCancellation() async throws {
        let cancelled = QueryCancellation()
        cancelled.cancel()
        let neverStarted = Process()
        neverStarted.executableURL = URL(fileURLWithPath: "/bin/sleep")
        neverStarted.arguments = ["30"]
        #expect(throws: CancellationError.self) { try cancelled.start(neverStarted) }
        #expect(!neverStarted.isRunning)

        let cancellation = QueryCancellation()
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/sleep")
        process.arguments = ["30"]
        try cancellation.start(process)
        cancellation.cancel()
        await Task.detached { process.waitUntilExit() }.value
        cancellation.finished()
        #expect(!process.isRunning)
        #expect(process.terminationReason == .uncaughtSignal)
    }

    @Test("Watch renders retained attempts alone and inside the Work workspace", arguments: [false, true])
    func renderSnapshot(inWorkspace: Bool) async throws {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let roadmap = try String(contentsOf: root.appendingPathComponent(
            "tests/fixtures/dto/roadmap_snapshot.json"
        ), encoding: .utf8)
        let planning = RegistryQuery { args, _ in
            switch args.first {
            case "roadmap": return roadmap
            case "session", "ls": return "[]"
            case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            default: throw RegistryQueryError("Unexpected operation in Watch rendering")
            }
        }
        let model = PodiumModel(query: planning, repoPath: "/src/loopflow")
        await model.refresh()
        model.select(.task(id: "issue-review"))
        model.navigation.content = .watch
        model.navigation.showsList = true
        let store = model.navigation.watch(for: "issue-review")
        await store.refresh(issue: "LOO-293", query: query(try fixture()))
        let output = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/task_output.json"))
        let emptyHistory = #"{"task_id":"task-watch","sources":[],"gaps":[],"next_cursor":"history"}"#
        await store.readOutput(issue: "LOO-293", query: RegistryQuery { args, _ in
            args.contains("--tail") ? String(decoding: output, as: UTF8.self) : emptyHistory
        })
        var arrival = try #require(JSONSerialization.jsonObject(with: output) as? [String: Any])
        var sources = try #require(arrival["sources"] as? [[String: Any]])
        sources[0]["records"] = [["source_item_id": "later-prose", "revision": "1", "event": [
            "type": "text_delta", "turn_id": "turn", "content": "The command is still running; inspecting the next stage."
        ]]]
        arrival["sources"] = [sources[0]]
        await store.readOutput(issue: "LOO-293", query: query(try JSONSerialization.data(withJSONObject: arrival)))
        #expect(store.outputGroups.map { $0.source.runId } == [sources[0]["run_id"] as? String, sources[1]["run_id"] as? String, sources[0]["run_id"] as? String])
        #expect(store.visibleOutput.count == 3)
        #expect(store.visibleOutput.first?.rows.first?.title == "bash · running")
        let workspace = SessionsView(
            model: model, repoPath: "/src/loopflow",
            workspaces: SessionsWorkspaceRegistry(), query: planning
        )
        if inWorkspace {
            let selected = try #require(model.task(id: "issue-review"))
            _ = try workspace.inspect().find(text: "\(selected.task.task.identifier) · \(selected.task.task.name)")
            _ = try workspace.inspect().find(button: "Work details")
            #expect(try workspace.inspect().find(TaskWatchView.self).actualView().store === store)
        }
        let content = inWorkspace ? AnyView(workspace) : AnyView(TaskWatchView(issue: "LOO-293", store: store))
        let width: CGFloat = inWorkspace ? 1200 : 1000
        let view = content
            .frame(width: width, height: 720)
            .environment(\.palette, LoopflowPalette.light)
        let host = NSHostingView(rootView: view)
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: width, height: 720),
            styleMask: [.borderless], backing: .buffered, defer: false
        )
        window.isReleasedWhenClosed = false
        window.appearance = NSAppearance(named: .aqua)
        window.contentView = host
        defer { window.close() }
        host.layoutSubtreeIfNeeded()
        window.displayIfNeeded()
        let bitmap = try #require(host.bitmapImageRepForCachingDisplay(in: host.bounds))
        host.cacheDisplay(in: host.bounds, to: bitmap)
        let png = try #require(bitmap.representation(using: .png, properties: [:]))
        #expect(png.count > 10_000)
        if let path = ProcessInfo.processInfo.environment["LF_WATCH_RENDER_PATH"] {
            let destination = URL(fileURLWithPath: path)
            let output = inWorkspace
                ? destination.deletingPathExtension().appendingPathExtension("workspace.png")
                : destination
            try png.write(to: output)
            if !inWorkspace {
                let feed = NSHostingView(rootView: TaskWatchOutputView(
                    store: store, onHistory: { _ in }, onFollow: { store.followLive() }, onReload: {}
                ).frame(width: 1000, height: 720)
                    .foregroundStyle(LoopflowPalette.light.text)
                    .background(LoopflowPalette.light.background)
                    .environment(\.palette, LoopflowPalette.light))
                window.contentView = feed
                feed.layoutSubtreeIfNeeded()
                window.displayIfNeeded()
                let feedBitmap = try #require(feed.bitmapImageRepForCachingDisplay(in: feed.bounds))
                feed.cacheDisplay(in: feed.bounds, to: feedBitmap)
                let feedPNG = try #require(feedBitmap.representation(using: .png, properties: [:]))
                try feedPNG.write(to: destination.deletingPathExtension().appendingPathExtension("feed.png"))
            }
            if !inWorkspace {
                let diagram = NSHostingView(rootView: TaskWatchDiagram(store: store)
                    .frame(width: 360, height: 540)
                    .environment(\.palette, LoopflowPalette.light))
                window.setContentSize(NSSize(width: 360, height: 540))
                window.contentView = diagram
                diagram.layoutSubtreeIfNeeded()
                window.displayIfNeeded()
                let diagramBitmap = try #require(diagram.bitmapImageRepForCachingDisplay(in: diagram.bounds))
                diagram.cacheDisplay(in: diagram.bounds, to: diagramBitmap)
                let diagramPNG = try #require(diagramBitmap.representation(using: .png, properties: [:]))
                try diagramPNG.write(to: destination.deletingPathExtension().appendingPathExtension("plan.png"))
            }
        }
    }

    private func query(_ data: Data) -> RegistryQuery {
        RegistryQuery { _, cwd in
            #expect(cwd == nil)
            return String(decoding: data, as: UTF8.self)
        }
    }

    private func fixture() throws -> Data {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        return try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/task_watch.json"))
    }
}
#endif
