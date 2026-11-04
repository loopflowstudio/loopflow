#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite(.serialized)
struct ServerClientTests {
    @Test(arguments: [false, true])
    func failedSetupDoesNotRegisterOnWorkRefresh(unreadyReply: Bool) async throws {
        let root = URL(fileURLWithPath: "/tmp").appendingPathComponent("server-\(UUID().uuidString)")
        let fm = FileManager.default
        try fm.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: root) }
        let lf = try BuiltLf(home: root, overriding: ["TEST_LF": BuiltLf.binary("lf")])
        for args in [["init", "-b", "main"], ["config", "user.name", "Fixture Person"],
                     ["config", "user.email", "fixture@example.invalid"], ["commit", "--allow-empty", "-m", "Fixture"]] {
            _ = try await lf.run(args, executable: "/usr/bin/git")
        }
        _ = try await lf.run(["task", "create", "--title", "Recovered Work", "--json"])
        let installer = root.appendingPathComponent("installer")
        let failure = unreadyReply
            ? #"printf '%s\n' '{"socket":"/unused","ready":false,"instance":null,"error":"fixture Background Items disabled"}'"#
            : "echo 'fixture Background Items disabled' >&2; exit 1"
        try """
        #!/bin/sh
        if [ "$1" = install ] && [ "$2" = server ]; then
            printf 'attempt\\n' >> "$HOME/setup-attempts"
            if [ ! -f "$HOME/setup-allowed" ]; then
                \(failure)
                exit 0
            fi
            touch "$HOME/registered-again"
            exec "$TEST_LF" server status
        fi
        sleep 0.1
        exec "$TEST_LF" "$@"
        """.write(to: installer, atomically: true, encoding: .utf8)
        try fm.setAttributes([.posixPermissions: 0o755], ofItemAtPath: installer.path)
        let location = ServerLocation { try await lf.run($0, executable: installer.path) }
        let query = ServedRead.roadmap(directory: root.path, wave: nil, task: nil, all: true)
        do {
            _ = try await ServerClient.read(query, location: location)
            Issue.record("Initial setup must report its failure")
        } catch {
            #expect(error.localizedDescription.contains("fixture Background Items disabled"))
        }
        try Data().write(to: root.appendingPathComponent("setup-allowed"))
        // Both a refreshed reading and reopened watches share one consumed
        // readiness attempt. Installation is now possible but remains untouched.
        try await withThrowingTaskGroup(of: Void.self) { group in
            for attempt in 0..<3 {
                group.addTask {
                    if attempt == 0 {
                        do {
                            _ = try await ServerClient.read(query, location: location)
                            Issue.record("The server is still unavailable")
                        } catch {
                            #expect(error.localizedDescription.contains("fixture Background Items disabled"))
                        }
                    } else {
                        let watch = try ServerClient.watch(query, location: location)
                        do {
                            for try await _ in watch.lines { Issue.record("The server is still unavailable") }
                            Issue.record("The watch must report unavailability")
                        } catch {
                            #expect(error.localizedDescription.contains("fixture Background Items disabled"))
                        }
                        await watch.cancel()
                    }
                }
            }
            try await group.waitForAll()
        }
        #expect(try String(contentsOf: root.appendingPathComponent("setup-attempts"), encoding: .utf8) == "attempt\n")
        #expect(!fm.fileExists(atPath: root.appendingPathComponent("registered-again").path))

        // Installation repairs availability independently of the shared client.
        let server = lf.process(["server", "run"])
        server.standardInput = FileHandle.nullDevice
        server.standardOutput = FileHandle.nullDevice
        server.standardError = FileHandle.nullDevice
        let (exits, ended) = AsyncStream<Int32>.makeStream()
        server.terminationHandler = { ended.yield($0.terminationStatus); ended.finish() }
        try server.run()
        do {
            let deadline = ContinuousClock.now + .seconds(15)
            while true {
                let status = try await lf.run(["server", "status"])
                let reading = try #require(JSONSerialization.jsonObject(with: Data(status.utf8)) as? [String: Any])
                if reading["ready"] as? Bool == true { break }
                try #require(ContinuousClock.now < deadline, "Independent server did not become ready")
                try await Task.sleep(for: .milliseconds(20))
            }
            let reading = try await ServerClient.read(query, location: location)
            #expect(reading.contains("Recovered Work"))
            let watch = try ServerClient.watch(query, location: location)
            do {
                var frames = watch.lines.makeAsyncIterator()
                let first = try #require(try await frames.next())
                #expect(String(decoding: first, as: UTF8.self).contains("Recovered Work"))
            } catch {
                await watch.cancel()
                throw error
            }
            await watch.cancel()
            #expect(try String(contentsOf: root.appendingPathComponent("setup-attempts"), encoding: .utf8) == "attempt\n")
            #expect(!fm.fileExists(atPath: root.appendingPathComponent("registered-again").path))
        } catch {
            server.terminate()
            for await _ in exits {}
            throw error
        }
        server.terminate()
        for await _ in exits {}
    }
}
#endif
