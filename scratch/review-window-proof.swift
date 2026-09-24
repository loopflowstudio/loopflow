#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@MainActor
@Suite("Watch retention CLI review", .serialized)
struct WatchWindowCLIReview {
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
            let path = URL(fileURLWithPath: "/tmp/loo293-window-query-\(UUID().uuidString)")
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

    @Test func retentionAndRecovery() async throws {
        let home = try String(contentsOfFile: "/tmp/loo293-window-proof-root", encoding: .utf8)
        let query = query(home: home)
        let store = TaskWatchStore()
        await store.refresh(issue: "LOO-293", query: query)
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.outputError == nil && store.output.count == 2)
        for _ in 0..<20 {
            if store.output.allSatisfy({ !$0.historyHasMore }) { break }
            await store.readOutput(issue: "LOO-293", query: query, history: true)
            #expect(store.outputError == nil)
        }
        #expect(store.output.allSatisfy { !$0.historyHasMore && $0.source.available })
        #expect(store.outputGaps.isEmpty)
        #expect(store.output.flatMap(\.retainedRecords).count == 4096)
        #expect(store.output.flatMap(\.rows).count == 4096)
        #expect(store.output.allSatisfy { $0.source.records.isEmpty })
        #expect(store.outputWindowTrimmed)
        for provider in ["claude", "codex"] {
            #expect(!store.output.flatMap(\.rows).contains { $0.text == "\(provider) record 0" })
            #expect(store.output.flatMap(\.rows).contains { $0.text == "\(provider) record 2303" })
        }
        try render(store)
        // Arrivals happen after the retained history filled, before restarting it.
        for (number, provider) in [(1, "claude"), (2, "codex")] {
            let run = String(format: "run_%032x", number)
            let file = try FileHandle(forWritingTo: URL(fileURLWithPath: home + "/runs/00/" + run + "/native.jsonl"))
            try file.seekToEnd()
            try file.write(contentsOf: Data(contentsOf: URL(fileURLWithPath: home + "/\(provider)-arrival.jsonl")))
            try file.close()
        }
        store.inspectRun(store.output[0].source.runId)
        let selected = store.runId
        var restart = false
        let view = TaskWatchOutputView(store: store, onHistory: { restart = $0 },
                                       onFollow: { store.followLive() }, onReload: {})
        try view.inspect().find(button: "Restart history").tap()
        #expect(restart)
        // A source failure is encoded inside a successful CLI envelope.
        let native = URL(fileURLWithPath: home + "/runs/00/run_00000000000000000000000000000001/native.jsonl")
        let unavailable = native.appendingPathExtension("unavailable")
        try FileManager.default.moveItem(at: native, to: unavailable)
        await store.readOutput(issue: "LOO-293", query: query, restartHistory: restart)
        #expect(store.output.flatMap(\.retainedRecords).count == 4096)
        #expect(store.outputWindowTrimmed)
        #expect(store.outputError != nil)
        try FileManager.default.moveItem(at: unavailable, to: native)
        await store.readOutput(issue: "LOO-293", query: query, restartHistory: restart)
        #expect(store.outputError == nil && !store.outputWindowTrimmed)
        #expect(store.runId == selected && !store.followsOutput)
        // Codex session metadata consumes one reader slot without rendering a row.
        #expect(store.output.flatMap(\.retainedRecords).count == 255)
        for provider in ["claude", "codex"] {
            #expect(store.output.flatMap(\.rows).contains { $0.text == "\(provider) record 0" })
        }
        try view.inspect().find(button: "Follow live").tap()
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.outputError == nil && store.runId == nil && store.followsOutput)
        for provider in ["claude", "codex"] {
            let text = "\(provider) record 2304"
            let arrivals = store.output.flatMap(\.rows).filter { $0.text == text }
            #expect(arrivals.count == 1)
        }
        #expect(store.output.flatMap(\.retainedRecords).count == 257)
        let target = store.latestOutputRow
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.output.flatMap(\.retainedRecords).count == 257)
        #expect(store.latestOutputRow == target)
        let reopened = TaskWatchStore()
        await reopened.readOutput(issue: "LOO-293", query: query)
        #expect(reopened.outputError == nil)
        #expect(reopened.output.flatMap(\.rows).contains { $0.text == "claude record 0" })
        print("PASS real CLI/native histories: 4608 records paged across two Runs -> 4096 retained; Restart recovers 255 early records; both arrivals survive live continuation once; quiet read deduplicates; fresh Watch retains source history")
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

    private func render(_ store: TaskWatchStore) throws {
        let host = NSHostingView(rootView: TaskWatchOutputView(store: store, onHistory: { _ in }, onFollow: {}, onReload: {})
            .frame(width: 1000, height: 650)
            .foregroundStyle(LoopflowPalette.light.text)
            .background(LoopflowPalette.light.background)
            .environment(\.palette, LoopflowPalette.light))
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1000, height: 650),
                              styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = host
        defer { window.close() }
        host.layoutSubtreeIfNeeded()
        window.displayIfNeeded()
        let bitmap = try #require(host.bitmapImageRepForCachingDisplay(in: host.bounds))
        host.cacheDisplay(in: host.bounds, to: bitmap)
        try #require(bitmap.representation(using: .png, properties: [:])).write(to: URL(fileURLWithPath: "/tmp/loo293-window-cli-view.png"))
    }
}
#endif
