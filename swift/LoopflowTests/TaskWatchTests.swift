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
    @Test("Inspection survives transitions, repeated names, completion, and failed refreshes")
    func selectionAndStaleEvidence() async throws {
        let store = TaskWatchStore()
        let initial = try fixture()
        await store.refresh(issue: "LOO-293", query: query(initial))
        #expect(store.invocationId == "flow-one")
        #expect(store.stepIndex == 0)
        #expect(store.stage?.attempts.count == 3)
        store.stepIndex = 1

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

    @Test("The Watch fixture renders retained attempts without a provider client")
    func renderSnapshot() async throws {
        let store = TaskWatchStore()
        await store.refresh(issue: "LOO-293", query: query(try fixture()))
        let view = TaskWatchView(issue: "LOO-293", store: store)
            .frame(width: 1000, height: 720)
            .environment(\.palette, LoopflowPalette.light)
        let host = NSHostingView(rootView: view)
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1000, height: 720),
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
            try png.write(to: URL(fileURLWithPath: path))
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
