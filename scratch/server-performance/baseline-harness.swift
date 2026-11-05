#if os(macOS)
import Darwin
import Foundation
import Testing
import Loopflow
@testable import LoopflowMac

@Suite(.serialized)
struct ServerPerformanceTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["LOOPFLOW_BOUNDARY_PERF"] != nil))
    func measureBoundary() async throws {
        let root = URL(fileURLWithPath: "/tmp").appendingPathComponent("lf-perf-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let lf = try BuiltLf(home: root)
        for args in [["init", "-b", "main"], ["config", "user.name", "Fixture"],
                     ["config", "user.email", "fixture@example.invalid"], ["commit", "--allow-empty", "-m", "Fixture"]] {
            _ = try await lf.run(args, executable: "/usr/bin/git")
        }
        _ = try await lf.run(["task", "create", "--title", "Performance fixture", "--json"])
        let prepared = try await lf.run(["session", "resume", "--prepare", "--json"])
        let session = try #require((JSONSerialization.jsonObject(with: Data(prepared.utf8)) as? [String: Any])?["id"] as? String)
        setenv("LF_HOME", root.path, 1)
        defer { unsetenv("LF_HOME") }
        var results: [String: [Double]] = [:]
        for _ in 0..<31 {
            var start = ContinuousClock.now
            let reading = try await LoopflowLibrary.read(.allPlans)
            #expect(reading.contains("Performance fixture"))
            results["read_ms", default: []].append(start.elapsedMs)
            start = .now
            _ = try await LoopflowLibrary.write(.sessionRename(directory: root.path, id: session, name: "Measured Session"))
            results["write_ms", default: []].append(start.elapsedMs)
            start = .now
            let watch = try LoopflowLibrary.watch(.allPlans)
            var frames = watch.lines.makeAsyncIterator()
            _ = try #require(try await frames.next())
            results["subscribe_first_ms", default: []].append(start.elapsedMs)
            start = .now
            watch.send(Data(#"{"id":7}"#.utf8))
            let frame = try #require(try await frames.next())
            #expect(String(decoding: frame, as: UTF8.self).contains("\"answers\":7"))
            results["checkpoint_ms", default: []].append(start.elapsedMs)
            await watch.cancel()
        }
        let path = try #require(ProcessInfo.processInfo.environment["LOOPFLOW_BOUNDARY_PERF"])
        try JSONSerialization.data(withJSONObject: results, options: [.prettyPrinted, .sortedKeys])
            .write(to: URL(fileURLWithPath: path))
    }
}
#endif
