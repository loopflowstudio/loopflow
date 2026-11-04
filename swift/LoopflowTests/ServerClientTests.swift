#if os(macOS)
import Darwin
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite(.serialized)
struct ServerClientTests {
    @Test func cancelledReadReleasesItsConnection() async throws {
        let peer = try ServerPeer()
        let request = Task { try await ServerClient.read(.machineId, location: peer.location) }
        async let disconnected = peer.exchange { handle, _ in
            request.cancel()
            // The peer stays alive without answering. Cancellation must close
            // the connection rather than wait for a response or stop the server.
            return try handle.read(upToCount: 1)?.isEmpty ?? true
        }
        do {
            _ = try await request.value
            Issue.record("A cancelled read returned a reading")
        } catch { #expect(error is CancellationError) }
        #expect(try await disconnected)
    }

    @Test func failedCommandPreservesOutputAndDiagnosis() async throws {
        let peer = try ServerPeer()
        async let answered: Void = peer.exchange { handle, id in
            let body = try JSONSerialization.data(withJSONObject: [
                "id": id, "instance": "fixture", "result": [
                    "kind": "command", "trace_id": "trace", "output": "saved partial result",
                    "stderr": "fixture command failed", "code": 7,
                ],
            ])
            var length = UInt32(body.count).bigEndian
            try handle.write(contentsOf: withUnsafeBytes(of: &length) { Data($0) } + body)
        }
        do {
            _ = try await ServerClient.write(.graphCustomize(directory: "/", name: "code"), location: peer.location)
            Issue.record("A failed command succeeded")
        } catch let error as RegistryQueryError {
            #expect(error.stdout == "saved partial result")
            #expect(error.localizedDescription == "fixture command failed")
        }
        try await answered
    }

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
                        let watch = ServerClient.watch(query, location: location)
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
        let server = LocalServerFixture(lf)
        do {
            _ = try await server.status()
            let reading = try await ServerClient.read(query, location: location)
            #expect(reading.contains("Recovered Work"))
            let watch = ServerClient.watch(query, location: location)
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
            await server.stop()
            throw error
        }
        await server.stop()
    }
}

/// One same-account peer exercising the production framing and socket lifetime.
private final class ServerPeer: @unchecked Sendable {
    private let listener: Int32
    private let path = "/tmp/lf-peer-\(UUID().uuidString).sock"

    init() throws {
        listener = socket(AF_UNIX, SOCK_STREAM, 0)
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        withUnsafeMutableBytes(of: &address.sun_path) { $0.copyBytes(from: path.utf8) }
        let bound = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.bind(listener, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard bound == 0, listen(listener, 1) == 0 else {
            Darwin.close(listener)
            throw RegistryQueryError("Cannot listen for fixture request")
        }
    }

    deinit {
        Darwin.close(listener)
        unlink(path)
    }

    var location: ServerLocation {
        let path = path
        return ServerLocation { _ in
            String(decoding: try JSONSerialization.data(withJSONObject: ["socket": path, "ready": true]), as: UTF8.self)
        }
    }

    func exchange<T: Sendable>(_ respond: @escaping @Sendable (FileHandle, String) throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            Thread.detachNewThread { [self] in
                do {
                    let fd = accept(listener, nil, nil)
                    guard fd >= 0 else { throw RegistryQueryError("Fixture accept failed") }
                    let handle = FileHandle(fileDescriptor: fd, closeOnDealloc: true)
                    defer { try? handle.close() }
                    var timeout = timeval(tv_sec: 5, tv_usec: 0)
                    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
                    let header = try #require(try handle.read(upToCount: 4))
                    let length = header.reduce(0) { ($0 << 8) | Int($1) }
                    let body = try #require(try handle.read(upToCount: length))
                    let request = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
                    continuation.resume(returning: try respond(handle, #require(request["id"] as? String)))
                } catch { continuation.resume(throwing: error) }
            }
        }
    }
}
#endif
