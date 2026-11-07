#if os(macOS)
import Darwin
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite(.serialized)
struct ServerClientTests {
    @Test func cancelledReadSendsCancelDropsLateAnswerAndKeepsTheConnection() async throws {
        let peer = try ServerPeer()
        let firstId = Mutex<String?>(nil)
        let sawCancel = Mutex<Bool>(false)
        peer.serve { message, respond in
            switch message["type"] as? String {
            case "request":
                let id = message["id"] as! String
                if sawCancel.get() {
                    respond(["type": "response", "id": id, "trace_id": "trace", "result": ["id": "home_fixture"]])
                } else {
                    firstId.set(id) // The cancelled read: answer it late, after the cancel.
                }
            case "cancel":
                sawCancel.set(true)
                if let late = firstId.get() {
                    respond(["type": "response", "id": late, "trace_id": "trace", "result": ["id": "late"]])
                }
            default: break
            }
            return true
        }
        let request = Task { try await ServerClient.read(.machineId, location: peer.location) }
        peer.waitForMessage { $0["type"] as? String == "request" }
        request.cancel()
        do {
            _ = try await request.value
            Issue.record("A cancelled read returned a reading")
        } catch { #expect(error is CancellationError) }
        // The connection stays open; the dropped late answer does not corrupt it.
        let reading = try await ServerClient.read(.machineId, location: peer.location)
        #expect(reading.contains("home_fixture"))
        #expect(!reading.contains("late"))
    }

    @Test func refusedWriteThrowsItsCodeAndWritesReturnTheirValue() async throws {
        let peer = try ServerPeer()
        let refused = Mutex<Bool>(false)
        peer.serve { message, respond in
            guard message["type"] as? String == "request" else { return true }
            if refused.get() {
                respond(["type": "response", "id": message["id"]!, "trace_id": "trace", "result": ["path": "/r/.lf/graphs/code.yaml"]])
            } else {
                refused.set(true)
                respond(["type": "error", "id": message["id"]!, "code": "conflict",
                         "message": "Dashboard changed; read it again before editing", "data": NSNull()])
            }
            return true
        }
        do {
            _ = try await ServerClient.write(.graphCustomize(directory: "/", name: "code"), location: peer.location)
            Issue.record("A refused write succeeded")
        } catch {
            #expect(error.localizedDescription == "conflict: Dashboard changed; read it again before editing")
        }
        let written = try await ServerClient.write(.graphCustomize(directory: "/", name: "code"), location: peer.location)
        #expect(try JSONDecoder().decode([String: String].self, from: Data(written.utf8)) == ["path": "/r/.lf/graphs/code.yaml"])
    }

    @Test func disconnectFailsEveryPendingRequestAndTheNextReopens() async throws {
        let peer = try ServerPeer()
        let seen = Mutex<Int>(0)
        // The first connection reads two requests, then closes, answering neither.
        peer.serve { message, _ in
            message["type"] as? String != "request" || seen.increment() < 2
        }
        let first = Task { try await ServerClient.read(.machineId, location: peer.location) }
        let second = Task { try await ServerClient.read(.portfolio, location: peer.location) }
        for pending in [first, second] {
            do {
                _ = try await pending.value
                Issue.record("A request outlived its connection")
            } catch {
                #expect(error.localizedDescription.contains("Server disconnected"))
            }
        }
        peer.serve { message, respond in
            if message["type"] as? String == "request" {
                respond(["type": "response", "id": message["id"]!, "trace_id": "trace", "result": "/src/again"])
            }
            return true
        }
        #expect(try await ServerClient.read(.repositoryPath(id: "repo_1"), location: peer.location) == "/src/again")
    }

    @Test func stalledHandshakeFailsWithinTheBoundAndTheNextRequestReopens() async throws {
        let peer = try ServerPeer()
        peer.stall()
        let started = Date()
        do {
            _ = try await ServerClient.read(.machineId, location: peer.location)
            Issue.record("A stalled handshake produced a reading")
        } catch {
            #expect(error.localizedDescription == "Loopflow server did not answer within 10s; check `lf server status`")
        }
        let elapsed = Date().timeIntervalSince(started)
        #expect(elapsed >= 9 && elapsed < 20, "waited \(elapsed)s")
        peer.serve { message, respond in
            if message["type"] as? String == "request" {
                respond(["type": "response", "id": message["id"]!, "trace_id": "trace", "result": "/src/after-stall"])
            }
            return true
        }
        #expect(try await ServerClient.read(.repositoryPath(id: "repo_1"), location: peer.location) == "/src/after-stall")
    }

    @Test func serverEndingAWatchFailsItsWatchAlone() async throws {
        let peer = try ServerPeer()
        let watchID = Mutex<String?>(nil)
        peer.serve { message, respond in
            switch (message["type"] as? String, (message["operation"] as? [String: Any])?["kind"] as? String) {
            case ("watch", _):
                let id = message["id"] as! String
                watchID.set(id)
                respond(["type": "response", "id": id, "trace_id": "trace", "result": NSNull()])
                respond(["type": "reading", "id": id, "frame": ["answers": 1, "unavailable": NSNull(), "body": NSNull()]])
            case ("checkpoint", _):
                #expect(message["checkpoint"] as? Int == 4)
                let id = watchID.get()!
                respond(["type": "reading", "id": id, "frame": ["answers": 4, "unavailable": "gone", "body": NSNull()]])
                respond(["type": "error", "id": id, "code": "unavailable", "message": "gone", "data": NSNull()])
            case ("request", _):
                respond(["type": "response", "id": message["id"]!, "trace_id": "trace", "result": "still here"])
            default: break
            }
            return true
        }
        let watch = ServerClient.watch(.allPlans, location: peer.location)
        var frames = watch.lines.makeAsyncIterator()
        let first = try #require(try await frames.next())
        #expect((try JSONSerialization.jsonObject(with: first) as? [String: Any])?["answers"] as? Int == 1)
        watch.send(Data(#"{"id":4}"#.utf8 + [10]))
        let last = try #require(try await frames.next())
        #expect((try JSONSerialization.jsonObject(with: last) as? [String: Any])?["unavailable"] as? String == "gone")
        do {
            _ = try await frames.next()
            Issue.record("An ended watch kept its stream open")
        } catch {
            #expect(error.localizedDescription == "unavailable: gone")
        }
        // The connection outlives the watch.
        #expect(try await ServerClient.read(.repositoryPath(id: "repo_1"), location: peer.location) == "still here")
        await watch.cancel()
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

/// A tiny lock around one value, for test coordination across threads.
private final class Mutex<Value>: @unchecked Sendable {
    let lock = NSLock()
    private var value: Value
    init(_ value: Value) { self.value = value }
    func get() -> Value { lock.withLock { value } }
    func set(_ next: Value) { lock.withLock { value = next } }
}

extension Mutex where Value == Int {
    func increment() -> Int { lock.withLock { value += 1; return value } }
}

/// One same-account peer speaking revision 2. Each `serve` accepts one
/// connection, welcomes it, then runs `handle` for every client message on a
/// reader thread until the handler closes it or the client disconnects.
private final class ServerPeer: @unchecked Sendable {
    private let listener: Int32
    private let path: String
    let location: ServerLocation
    private let lock = NSLock()
    private var messages: [[String: Any]] = []

    init() throws {
        let path = "/tmp/lf-peer-\(UUID().uuidString).sock"
        self.path = path
        location = ServerLocation { _ in
            String(decoding: try JSONSerialization.data(withJSONObject: ["socket": path, "ready": true]), as: UTF8.self)
        }
        listener = socket(AF_UNIX, SOCK_STREAM, 0)
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        withUnsafeMutableBytes(of: &address.sun_path) { $0.copyBytes(from: path.utf8) }
        let bound = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.bind(listener, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard bound == 0, listen(listener, 2) == 0 else {
            Darwin.close(listener)
            throw RegistryQueryError("Cannot listen for fixture request")
        }
    }

    deinit {
        Darwin.close(listener)
        unlink(path)
    }

    /// `handle` returns false to close the connection after this message.
    func serve(_ handle: @escaping @Sendable (_ message: [String: Any], _ respond: (_ line: [String: Any]) -> Void) -> Bool) {
        Thread.detachNewThread { [self] in
            let fd = accept(listener, nil, nil)
            guard fd >= 0 else { return }
            let connection = PeerConnection(fd: fd)
            defer { connection.close() }
            do {
                _ = try connection.read() // hello
                try connection.write([
                    "type": "welcome", "client_id": "fixture-client", "revision": 2, "server_revision": 2, "min_revision": 2, "instance": "fixture",
                    "build": "test", "executable_sha256": String(repeating: "f", count: 64), "machine": NSNull(),
                    "limits": ["max_message": 33_554_432, "max_connections": 64],
                ])
                while let message = try connection.read() {
                    lock.withLock { messages.append(message) }
                    if !handle(message, { try? connection.write($0) }) { return }
                }
            } catch {}
        }
    }

    /// Accept one connection and answer nothing until the client closes it.
    func stall() {
        Thread.detachNewThread { [self] in
            let fd = accept(listener, nil, nil)
            guard fd >= 0 else { return }
            let connection = PeerConnection(fd: fd)
            defer { connection.close() }
            while (try? connection.read()) != nil {}
        }
    }

    func waitForMessage(where matches: @escaping ([String: Any]) -> Bool) {
        let deadline = Date().addingTimeInterval(10)
        while Date() < deadline {
            if lock.withLock({ messages.contains(where: matches) }) { return }
            usleep(5000)
        }
    }
}

/// The peer's side of one connection: whole JSON lines in and out.
private final class PeerConnection: @unchecked Sendable {
    private let fd: Int32
    private var buffer = Data()

    init(fd: Int32) { self.fd = fd }

    /// The next message, or nil once the client closes.
    func read() throws -> [String: Any]? {
        while true {
            if let newline = buffer.firstIndex(of: 10) {
                let line = buffer.subdata(in: buffer.startIndex..<newline)
                buffer.removeSubrange(buffer.startIndex...newline)
                return try JSONSerialization.jsonObject(with: line) as? [String: Any]
            }
            var chunk = [UInt8](repeating: 0, count: 65536)
            let received = chunk.withUnsafeMutableBytes { Darwin.read(fd, $0.baseAddress!, $0.count) }
            if received < 0 && errno == EINTR { continue }
            guard received > 0 else { return nil }
            buffer.append(contentsOf: chunk[..<received])
        }
    }

    func write(_ message: [String: Any]) throws {
        let bytes = try JSONSerialization.data(withJSONObject: message) + [10]
        try bytes.withUnsafeBytes { buffer in
            var offset = 0
            while offset < buffer.count {
                let sent = Darwin.write(fd, buffer.baseAddress!.advanced(by: offset), buffer.count - offset)
                if sent < 0 && errno == EINTR { continue }
                guard sent > 0 else { throw RegistryQueryError("Fixture write failed") }
                offset += sent
            }
        }
    }

    func close() { Darwin.close(fd) }
}
#endif
