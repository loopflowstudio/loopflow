#if os(macOS)
import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@MainActor
@Suite("Watch cursor recovery CLI review", .serialized)
struct WatchPageCLIReview {
    private func query(previousVersion: Bool = false) -> RegistryQuery {
        RegistryQuery { args, _ in
            #expect(args.prefix(2) == ["task", "output"])
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            defer { try? FileManager.default.removeItem(at: directory) }
            let stdout = directory.appendingPathComponent("stdout")
            let stderr = directory.appendingPathComponent("stderr")
            FileManager.default.createFile(atPath: stdout.path, contents: nil)
            FileManager.default.createFile(atPath: stderr.path, contents: nil)
            let output = try FileHandle(forWritingTo: stdout)
            let errors = try FileHandle(forWritingTo: stderr)
            defer { try? output.close(); try? errors.close() }
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/Users/jack/src/loopflow.restore-task-watching-with-live/target/debug/lf")
            process.arguments = args
            process.currentDirectoryURL = URL(fileURLWithPath: "/tmp")
            var environment = ProcessInfo.processInfo.environment
            environment["LF_CONTROL_HOME"] = "/Users/jack/.lf"
            environment["LF_CONTROL_DB_PATH"] = "/Users/jack/.lf/loopflow.db"
            environment.removeValue(forKey: "LF_RUN_ID")
            process.environment = environment
            process.standardOutput = output
            process.standardError = errors
            try process.run()
            process.waitUntilExit()
            if process.terminationStatus != 0 {
                throw RegistryQueryError(try String(contentsOf: stderr, encoding: .utf8))
            }
            let data = try Data(contentsOf: stdout)
            let page = try JSONDecoder().decode(TaskOutputPage.self, from: data)
            for source in page.sources {
                #expect(source.records.count <= 128)
            }
            if !previousVersion { return String(decoding: data, as: UTF8.self) }
            // Simulate the viewer retaining a pre-upgrade continuation only.
            // Provider storage, registry and the CLI's response files stay untouched.
            var envelope = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
            var encoded = page.nextCursor.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
            encoded += String(repeating: "=", count: (4 - encoded.count % 4) % 4)
            let cursorData = try #require(Data(base64Encoded: encoded))
            var cursor = try #require(JSONSerialization.jsonObject(with: cursorData) as? [String: Any])
            cursor["version"] = 2
            envelope["next_cursor"] = try JSONSerialization.data(withJSONObject: cursor).base64EncodedString()
                .replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
            return String(decoding: try JSONSerialization.data(withJSONObject: envelope), as: UTF8.self)
        }
    }

    @Test func configuredOutputRecoversFromOldCursor() async throws {
        let store = TaskWatchStore()
        await store.readOutput(issue: "LOO-293", query: query(previousVersion: true))
        #expect(store.outputError == nil)
        let before = Set(store.output.flatMap { source in
            source.rows.map { "\(source.source.runId):\($0.id):\($0.text)" }
        })
        #expect(!before.isEmpty)
        await store.readOutput(issue: "LOO-293", query: query())
        #expect(store.outputError?.contains("reload output") == true)
        #expect(!store.needsOutputReload)
        #expect(Set(store.output.flatMap { source in
            source.rows.map { "\(source.source.runId):\($0.id):\($0.text)" }
        }) == before)
        var reload = false
        let view = TaskWatchOutputView(store: store, onHistory: { _ in }, onFollow: {}, onReload: { reload = true })
        try view.inspect().find(button: "Reload output").tap()
        await store.readOutput(issue: "LOO-293", query: query(), reload: reload)
        #expect(store.outputError == nil)
        let recovered = Set(store.output.flatMap { source in
            source.rows.map { "\(source.source.runId):\($0.id):\($0.text)" }
        })
        #expect(before.isSubset(of: recovered))
        store.followLive()
        await store.readOutput(issue: "LOO-293", query: query())
        #expect(store.outputError == nil)
        for source in store.output { #expect(Set(source.rows.map(\.id)).count == source.rows.count) }
        print("PASS configured read-only recovery: sources=\(store.output.count), priorRows=\(before.count), recoveredRows=\(recovered.count); old cursor rejected, Reload recovers, Follow identities unique")
    }
}
#endif
