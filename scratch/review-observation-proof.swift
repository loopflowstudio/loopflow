#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@MainActor
@Suite("Watch observation CLI review", .serialized)
struct WatchObservationCLIReview {
    private let checkout = "/Users/jack/src/loopflow.restore-task-watching-with-live"
    private func query(home: String) -> RegistryQuery {
        RegistryQuery { args, _ in
            // Only the branch's early read-only Task dispatch is exercised.
            #expect(args.prefix(2) == ["task", "watch"] || args.prefix(2) == ["task", "output"])
            let process = Process()
            process.executableURL = URL(fileURLWithPath: checkout + "/target/debug/lf")
            process.arguments = args
            process.currentDirectoryURL = URL(fileURLWithPath: "/tmp")
            var env = ProcessInfo.processInfo.environment
            env["LF_CONTROL_HOME"] = home
            env["LF_CONTROL_DB_PATH"] = "/Users/jack/.lf/loopflow.db"
            env.removeValue(forKey: "LF_RUN_ID")
            process.environment = env
            let path = URL(fileURLWithPath: "/tmp/loo293-observation-query-\(UUID().uuidString)")
            FileManager.default.createFile(atPath: path.path, contents: nil)
            let output = try FileHandle(forWritingTo: path)
            defer { try? output.close(); try? FileManager.default.removeItem(at: path) }
            process.standardOutput = output
            process.standardError = FileHandle.nullDevice
            try process.run()
            process.waitUntilExit()
            #expect(process.terminationStatus == 0)
            return try String(contentsOf: path, encoding: .utf8)
        }
    }

    @Test func nativeArrivals() async throws {
        let home = try String(contentsOfFile: "/tmp/loo293-observation-proof-root", encoding: .utf8)
        let query = query(home: home)
        let store = TaskWatchStore()
        await store.refresh(issue: "LOO-293", query: query)
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.outputError == nil)
        #expect(store.output.count == 2)
        #expect(store.output.allSatisfy { $0.rows.first?.title == "bash · running" })
        func append(_ provider: String, _ suffix: String) throws {
            let run = String(format: "run_%032x", provider == "claude" ? 1 : 2)
            let file = try FileHandle(forWritingTo: URL(fileURLWithPath: home + "/runs/00/" + run + "/native.jsonl"))
            defer { try? file.close() }
            try file.seekToEnd()
            try file.write(contentsOf: Data(contentsOf: URL(fileURLWithPath: home + "/\(provider)-\(suffix).jsonl")))
        }
        for (provider, suffix) in [("claude", "prose-1"), ("codex", "prose-1"), ("claude", "prose-2")] {
            try append(provider, suffix)
            await store.readOutput(issue: "LOO-293", query: query)
            #expect(store.outputError == nil)
        }
        let arrivals = store.outputGroups.filter { !$0.history }
        #expect(arrivals.map { $0.source.provider } == ["claude", "codex", "claude"])
        #expect(arrivals.flatMap { $0.rows.map(\.text) } == ["claude arrival 1", "codex arrival 1", "claude arrival 2"])
        let blocks = store.outputGroups.map(\.id)
        let before = store.latestOutputRow
        try append("claude", "arrival")
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.outputError == nil)
        #expect(store.outputGroups.map(\.id) == blocks)
        let tool = try #require(store.output.first { $0.source.provider == "claude" }?.rows.first)
        #expect(tool.title == "bash · completed")
        #expect(tool.text.contains("echo review-proof") && tool.text.hasSuffix("\nreview-proof"))
        #expect(store.latestOutputRow?.row == tool.id)
        #expect(store.latestOutputRow != before)
        let target = store.latestOutputRow
        let rows = store.output.map { $0.rows.map(\.text) }
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.output.map { $0.rows.map(\.text) } == rows)
        #expect(store.latestOutputRow == target)
        #expect(store.outputGroups.map(\.id) == blocks)
        store.inspectRun(store.output[0].source.runId)
        #expect(store.visibleOutput.count == 1)
        let view = TaskWatchOutputView(store: store, onHistory: {}, onFollow: { store.followLive() }, onReload: {})
        try view.inspect().find(button: "Follow live").tap()
        #expect(store.visibleOutput.count == 2)
        try render(TaskWatchView(issue: "LOO-293", store: store))
        print("PASS real CLI -> RegistryQuery -> Store -> View: A/B/A arrivals, original tool row updated, exact Follow target, quiet deduplication, Run filter and Follow action")
    }

    @Test func configuredHistory() async throws {
        let store = TaskWatchStore()
        let query = query(home: "/Users/jack/.lf")
        await store.refresh(issue: "LOO-293", query: query)
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.error == nil && store.outputError == nil)
        #expect(store.snapshot?.taskId == "task_97e04da37e5e4231bde30f4d1d93a5fa")
        #expect(store.output.contains { !$0.rows.isEmpty })
        print("PASS configured passive history: sources=\(store.output.count), rows=\(store.output.reduce(0) { $0 + $1.rows.count }), invocations=\(store.snapshot?.invocations.count ?? 0)")
    }

    private func render(_ view: TaskWatchView) throws {
        let host = NSHostingView(rootView: view.frame(width: 1100, height: 850).environment(\.palette, LoopflowPalette.light))
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1100, height: 850), styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.appearance = NSAppearance(named: .aqua)
        window.contentView = host
        defer { window.close() }
        host.layoutSubtreeIfNeeded()
        window.displayIfNeeded()
        let bitmap = try #require(host.bitmapImageRepForCachingDisplay(in: host.bounds))
        host.cacheDisplay(in: host.bounds, to: bitmap)
        try #require(bitmap.representation(using: .png, properties: [:])).write(to: URL(fileURLWithPath: "/tmp/loo293-observation-cli-view.png"))
    }
}
#endif
