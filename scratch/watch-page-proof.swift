#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

// Disposable source integration, not configured provider or human-demo evidence.
@MainActor
@Suite("Watch normalized page CLI proof", .serialized)
struct WatchPageCLIProof {
    @Test func messagePagesReachTheDesktop() async throws {
        let home = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: home) }
        let run = "run_00000000000000000000000000000001"
        let directory = home.appendingPathComponent("runs/00/\(run)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let path = directory.appendingPathComponent("native.jsonl")
        func write(_ name: String, _ value: [String: Any]) throws {
            try JSONSerialization.data(withJSONObject: value).write(to: directory.appendingPathComponent(name))
        }
        try write("manifest.json", [
            "schema_version": 1, "run_id": run, "parent_run_id": NSNull(),
            "created_at": "2026-09-24T12:00:00Z", "harness": "claude", "model": NSNull(),
            "surface": "tui", "cwd": "/tmp", "repo": NSNull(), "worktree": NSNull(), "skill": NSNull(),
            "subjects": [["selector": "task:LOO-293", "source": "declared"]],
            "launch": NSNull(), "context": NSNull(), "runtime_path": NSNull(), "runtime_digest": NSNull(),
            "host": "page-proof", "boot_id": NSNull()
        ])
        try write("provider-session.json", [
            "schema_version": 1, "provider_session_id": "session", "account_id": NSNull(),
            "native_source": ["kind": "jsonl", "path": path.path]
        ])
        func line(_ id: String, _ blocks: [[String: Any]]) throws -> Data {
            var data = try JSONSerialization.data(withJSONObject: [
                "type": "assistant", "sessionId": "session", "uuid": id,
                "message": ["id": id, "content": blocks]
            ])
            data.append(10)
            return data
        }
        var blocks: [[String: Any]] = (0..<259).map { ["type": "text", "text": "block \($0)"] }
        blocks[127] = ["type": "tool_use", "id": "call", "name": "bash", "input": ["command": "echo proof"]]
        blocks[128] = ["type": "tool_result", "tool_use_id": "call", "content": "proof"]
        try line("history", blocks).write(to: path)
        let query = RegistryQuery { args, _ in
            #expect(args.prefix(2) == ["task", "output"])
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/Users/jack/src/loopflow.restore-task-watching-with-live/target/debug/lf")
            process.arguments = args
            process.currentDirectoryURL = URL(fileURLWithPath: "/tmp")
            var environment = ProcessInfo.processInfo.environment
            environment["LF_CONTROL_HOME"] = home.path
            environment["LF_CONTROL_DB_PATH"] = "/Users/jack/.lf/loopflow.db"
            environment.removeValue(forKey: "LF_RUN_ID")
            process.environment = environment
            let destination = home.appendingPathComponent(UUID().uuidString)
            FileManager.default.createFile(atPath: destination.path, contents: nil)
            let output = try FileHandle(forWritingTo: destination)
            defer { try? output.close(); try? FileManager.default.removeItem(at: destination) }
            process.standardOutput = output
            process.standardError = FileHandle.nullDevice
            try process.run()
            process.waitUntilExit()
            #expect(process.terminationStatus == 0)
            let data = try Data(contentsOf: destination)
            let page = try JSONDecoder().decode(TaskOutputPage.self, from: data)
            #expect(page.sources.count == 1)
            #expect(page.sources[0].records.count <= 128)
            #expect(page.sources[0].gaps.isEmpty && page.gaps.isEmpty)
            print("records=\(page.sources[0].records.count) hasMore=\(page.sources[0].hasMore)")
            return String(decoding: data, as: UTF8.self)
        }
        let store = TaskWatchStore()
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.outputError == nil)
        #expect(store.output.flatMap(\.retainedRecords).count == 128)
        store.inspectRun(run)
        for _ in 0..<2 { await store.readOutput(issue: "LOO-293", query: query, history: true) }
        #expect(store.outputError == nil && store.runId == run)
        #expect(store.output.flatMap(\.retainedRecords).count == 259)
        #expect(store.output.allSatisfy { !$0.historyHasMore })
        let tool = try #require(store.visibleOutput.flatMap(\.rows).first { $0.title == "bash · completed" })
        #expect(tool.text.contains("echo proof") && tool.text.contains("proof"))
        store.inspectRun("another-run")
        #expect(store.visibleOutput.isEmpty)
        let writer = try FileHandle(forWritingTo: path)
        try writer.seekToEnd()
        try writer.write(contentsOf: line("arrival", [["type": "text", "text": "new arrival"]]))
        try writer.close()
        store.followLive()
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.outputError == nil && store.runId == nil)
        #expect(store.output.flatMap(\.retainedRecords).count == 260)
        #expect(store.visibleOutput.flatMap(\.rows).filter { $0.text == "new arrival" }.count == 1)
        await store.readOutput(issue: "LOO-293", query: query)
        #expect(store.outputError == nil)
        #expect(store.output.flatMap(\.retainedRecords).count == 260)
    }
}
#endif
