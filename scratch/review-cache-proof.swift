#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@MainActor
@Suite("Watch cache CLI review", .serialized)
struct WatchCacheCLIReview {
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
            let path = URL(fileURLWithPath: "/tmp/loo293-cache-query-\(UUID().uuidString)")
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

    @Test func configuredEvictionRecovery() async throws {
        let navigation = WorkspaceNavigation()
        let task = "task_97e04da37e5e4231bde30f4d1d93a5fa"
        let query = query(home: "/Users/jack/.lf")
        weak var released: TaskWatchStore?
        var before: Set<String> = []
        do {
            let store = navigation.watch(for: task)
            released = store
            await store.refresh(issue: "LOO-293", query: query)
            await store.readOutput(issue: "LOO-293", query: query)
            #expect(store.error == nil && store.outputError == nil)
            #expect(store.snapshot?.taskId == task)
            before = Set(store.output.flatMap { source in
                source.rows.map { "\(source.source.runId):\($0.id):\($0.text)" }
            })
            #expect(!before.isEmpty)
            store.inspectRun(try #require(store.output.first?.source.runId))
            #expect(navigation.watch(for: task) === store)
        }
        // These local navigation entries do not create or query production Tasks.
        for index in 0..<4 { _ = navigation.watch(for: "review-local-\(index)") }
        #expect(released == nil)
        let reopened = navigation.watch(for: task)
        #expect(reopened.snapshot == nil && reopened.output.isEmpty)
        #expect(reopened.runId == nil && reopened.followsOutput)
        await reopened.refresh(issue: "LOO-293", query: query)
        await reopened.readOutput(issue: "LOO-293", query: query)
        #expect(reopened.error == nil && reopened.outputError == nil)
        #expect(reopened.snapshot?.taskId == task)
        let recovered = Set(reopened.output.flatMap { source in
            source.rows.map { "\(source.source.runId):\($0.id):\($0.text)" }
        })
        #expect(before.isSubset(of: recovered))
        reopened.followLive()
        await reopened.readOutput(issue: "LOO-293", query: query)
        #expect(reopened.outputError == nil)
        for source in reopened.output {
            #expect(Set(source.rows.map(\.id)).count == source.rows.count)
        }
        print("PASS configured cache eviction and fresh recovery: sources=\(reopened.output.count), recoveredRows=\(recovered.count), invocations=\(reopened.snapshot?.invocations.count ?? 0); Follow has unique row identities")
    }
}
#endif
