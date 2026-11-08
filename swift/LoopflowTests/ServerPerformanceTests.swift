#if os(macOS)
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
        // Prepare starts no provider but still resolves one; a host with several
        // harnesses installed and no saved preference needs it named.
        let prepared = try await lf.run(["session", "resume", "--prepare", "--json", "-a", "claude"])
        let session = try #require((JSONSerialization.jsonObject(with: Data(prepared.utf8)) as? [String: Any])?["id"] as? String)
        let diagnostics = root.appendingPathComponent("server.log")
        try Data().write(to: diagnostics)
        let server = LocalServerFixture(lf, diagnostics: diagnostics)
        let location = server.location()
        let startup = ContinuousClock.now
        _ = try await location.path()
        var results: [String: [Double]] = ["server_ready_ms": [startup.elapsedMs]]
        // Readiness probes connect too; the workload's own connections start here.
        let admittedBeforeWorkload = try server.admittedConnections()
        for _ in 0..<31 {
            var start = ContinuousClock.now
            let reading = try await ServerClient.read(.allPlans, location: location)
            #expect(reading.contains("Performance fixture"))
            results["read_ms", default: []].append(start.elapsedMs)
            start = .now
            _ = try await ServerClient.write(.sessionRename(directory: root.path, id: session, name: "Measured Session"), location: location)
            results["write_ms", default: []].append(start.elapsedMs)
            start = .now
            let subscription = ServerClient.subscribe(.allPlans, location: location)
            var frames = subscription.lines.makeAsyncIterator()
            _ = try #require(try await frames.next())
            results["watch_first_ms", default: []].append(start.elapsedMs)
            start = .now
            subscription.send(Data(#"{"id":7}"#.utf8))
            let frame = try #require(try await frames.next())
            #expect(String(decoding: frame, as: UTF8.self).contains("\"answers\":7"))
            results["checkpoint_ms", default: []].append(start.elapsedMs)
            await subscription.cancel()
        }
        // Every read, write, subscription and checkpoint above shared one connection.
        results["workload_connections"] = [Double(try server.admittedConnections() - admittedBeforeWorkload)]
        let subscription = ServerClient.subscribe(.allPlans, location: location)
        var frames = subscription.lines.makeAsyncIterator()
        _ = try #require(try await frames.next())
        let pid = try #require(await server.processID)
        func resourceSample() async throws -> (cpu: Double, rss: Double) {
            let text = try await lf.run(["-p", String(pid), "-o", "time=,rss="], executable: "/bin/ps")
            let fields = text.split(whereSeparator: { $0.isWhitespace })
            let time = fields[0].split(separator: ":").compactMap { Double($0) }
            return (time.reduce(0) { $0 * 60 + $1 }, try #require(Double(fields[1])))
        }
        let before = try await resourceSample()
        let idle = ContinuousClock.now
        try await Task.sleep(for: .seconds(5))
        let after = try await resourceSample()
        results["idle_cpu_percent"] = [(after.cpu - before.cpu) / (idle.elapsedMs / 1000) * 100]
        results["idle_server_rss_kib"] = [after.rss]
        await subscription.cancel()
        // Reuse the same client after server replacement, without registration.
        await server.stop()
        let recovery = ContinuousClock.now
        _ = try await server.status()
        _ = try await ServerClient.read(.allPlans, location: location)
        results["server_restart_read_ms"] = [recovery.elapsedMs]
        await server.stop()
        let path = try #require(ProcessInfo.processInfo.environment["LOOPFLOW_BOUNDARY_PERF"])
        try JSONSerialization.data(withJSONObject: results, options: [.prettyPrinted, .sortedKeys])
            .write(to: URL(fileURLWithPath: path))
    }
}
#endif
