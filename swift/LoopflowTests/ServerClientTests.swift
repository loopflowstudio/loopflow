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
        let request = Task { try await ServerClient.read(.machineSelf, location: peer.location) }
        peer.waitForMessage { $0["type"] as? String == "request" }
        request.cancel()
        do {
            _ = try await request.value
            Issue.record("A cancelled read returned a reading")
        } catch { #expect(error is CancellationError) }
        // The connection stays open; the dropped late answer does not corrupt it.
        let reading = try await ServerClient.read(.machineSelf, location: peer.location)
        #expect(reading.contains("home_fixture"))
        #expect(!reading.contains("late"))
    }

    @Test func unansweredReadsExpireThenEndAStalledConnection() async throws {
        let peer = try ServerPeer()
        peer.serve { message, respond in
            if message["type"] as? String == "subscribe" {
                respond(["type": "response", "id": message["id"]!, "trace_id": "trace", "result": NSNull()])
            }
            return true // Reads are never answered.
        }
        let subscription = ServerClient.subscribe(.allPlans, location: peer.location)
        var frames = subscription.lines.makeAsyncIterator()
        peer.waitForMessage { $0["type"] as? String == "subscribe" }
        for _ in 0..<ServerLink.stalledReads {
            do {
                _ = try await ServerClient.read(.machineSelf, location: peer.location, deadline: .milliseconds(20))
                Issue.record("An unanswered read returned")
            } catch {
                #expect(error.localizedDescription.contains("did not answer in time"))
            }
        }
        do {
            _ = try await frames.next()
            Issue.record("A stalled connection kept its subscription")
        } catch {
            #expect(!(error is CancellationError))
        }
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
            #expect((error as? ServerError)?.code == .conflict)
            #expect((error as? ServerError)?.mayHaveHappened == false)
            #expect(error.localizedDescription == "Dashboard changed; read it again before editing")
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
        let first = Task { try await ServerClient.read(.machineSelf, location: peer.location) }
        let second = Task { try await ServerClient.read(.desktopPortfolio, location: peer.location) }
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
        #expect(try await ServerClient.read(.repoPath(id: "repo_1"), location: peer.location) == "/src/again")
    }

    @Test func disconnectedWatchResumesItsBaselineAndNewestConfirmation() async throws {
        let peer = try ServerPeer()
        peer.serve { message, respond in
            guard message["type"] as? String == "subscribe" else {
                // Lose the socket with a burst of confirmations still unanswered.
                return message["checkpoint"] as? Int != 100
            }
            #expect(message["resume"] is NSNull)
            let id = message["id"]!
            respond(["type": "response", "id": id, "trace_id": "trace", "result": NSNull()])
            respond(["type": "reading", "id": id, "checkpoint": "received-baseline",
                     "frame": ["answers": NSNull(), "unavailable": NSNull(), "body": "baseline"]])
            return true
        }
        let subscription = ServerClient.subscribe(.allPlans, location: peer.location)
        var frames = subscription.lines.makeAsyncIterator()
        defer { Task { await subscription.cancel() } }
        let baseline = try #require(try await frames.next())
        #expect(String(decoding: baseline, as: UTF8.self).contains("baseline"))
        let resumed = Mutex(false)
        peer.serve { message, respond in
            switch message["type"] as? String {
            case "subscribe":
                #expect(message["resume"] as? String == "received-baseline")
                resumed.set(true)
                respond(["type": "response", "id": message["id"]!, "trace_id": "trace", "result": NSNull()])
                // An unchanged reading sends no payload.
            case "checkpoint":
                #expect(resumed.get())
                #expect(message["checkpoint"] as? Int == 100)
                respond(["type": "reading", "id": message["id"]!, "checkpoint": "confirmed-baseline",
                         "frame": ["answers": message["checkpoint"]!, "unavailable": NSNull(), "body": "confirmed"]])
            case "cancel": return false
            default: break
            }
            return true
        }
        // Only the newest confirmation needs replay; it answers every earlier request.
        for id in 1...100 { subscription.send(Data("{\"id\":\(id)}".utf8)) }
        let confirmed = try #require(try await frames.next())
        let value = try JSONSerialization.jsonObject(with: confirmed) as! [String: Any]
        #expect(value["body"] as? String == "confirmed")
        #expect(value["answers"] as? Int == 100)
        await subscription.cancel()
    }

    @Test func disconnectedWatchKeepsItsQueuedBaselineUntilTheConsumerReadsIt() async throws {
        let peer = try ServerPeer()
        peer.serve { message, respond in
            guard message["type"] as? String == "subscribe" else { return true }
            let id = message["id"]!
            respond(["type": "response", "id": id, "trace_id": "trace", "result": NSNull()])
            respond(["type": "reading", "id": id, "checkpoint": "queued-baseline",
                     "frame": ["answers": NSNull(), "unavailable": NSNull(), "body": "baseline"]])
            return false
        }
        let subscription = ServerClient.subscribe(.allPlans, location: peer.location)
        defer { Task { await subscription.cancel() } }
        peer.waitForMessage { $0["type"] as? String == "subscribe" }
        let resumed = Mutex(false)
        peer.serve { message, respond in
            switch message["type"] as? String {
            case "subscribe":
                #expect(message["resume"] as? String == "queued-baseline")
                respond(["type": "response", "id": message["id"]!, "trace_id": "trace", "result": NSNull()])
                resumed.set(true) // The unchanged baseline is omitted on this connection.
            case "checkpoint":
                respond(["type": "reading", "id": message["id"]!, "checkpoint": "queued-baseline",
                         "frame": ["answers": message["checkpoint"]!, "unavailable": NSNull(), "body": "baseline"]])
            case "cancel": return false
            default: break
            }
            return true
        }
        // Do not consume the original stream until the replacement watch is live.
        let deadline = Date().addingTimeInterval(10)
        while !resumed.get(), Date() < deadline { try await Task.sleep(for: .milliseconds(5)) }
        try #require(resumed.get())
        var frames = subscription.lines.makeAsyncIterator()
        let baseline = try #require(try await frames.next())
        let value = try JSONSerialization.jsonObject(with: baseline) as! [String: Any]
        #expect(value["body"] as? String == "baseline")
        #expect(value["answers"] is NSNull)
        subscription.send(Data(#"{"id":1}"#.utf8))
        let confirmed = try #require(try await frames.next())
        let confirmation = try JSONSerialization.jsonObject(with: confirmed) as! [String: Any]
        #expect(confirmation["body"] as? String == "baseline")
        #expect(confirmation["answers"] as? Int == 1)
        await subscription.cancel()
        #expect(try await frames.next() == nil)
    }

    @Test func stalledHandshakeFailsWithinTheBoundAndTheNextRequestReopens() async throws {
        let peer = try ServerPeer()
        peer.stall()
        let started = Date()
        do {
            _ = try await ServerClient.read(.machineSelf, location: peer.location)
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
        #expect(try await ServerClient.read(.repoPath(id: "repo_1"), location: peer.location) == "/src/after-stall")
    }

    @Test func bulkAnswersAndOversizeRequestsTravelByTicket() async throws {
        let peer = try ServerPeer()
        let big = String(repeating: "x", count: ServerProtocol.maxMessage)
        peer.serve { message, respond in
            guard message["type"] as? String == "request", let id = message["id"] as? String,
                  let operation = message["operation"] as? [String: Any] else { return true }
            switch operation["kind"] as? String {
            case "repo.path":
                respond(["type": "bulk", "id": id, "ticket": "download", "bytes": big.utf8.count])
                try? peer.admit("download", initial: ["type": "response", "id": id, "trace_id": "trace", "result": big]) { _ in }
            case "request.upload":
                respond(["type": "response", "id": id, "trace_id": "trace", "result": ["ticket": "upload"]])
                try? peer.admit("upload") { connection in
                    let request = try connection.read()
                    let uploaded = ((request?["operation"] as? [String: Any])?["value"] as? String)?.utf8.count ?? 0
                    respond(["type": "response", "id": request?["id"] ?? "", "trace_id": "trace", "result": uploaded])
                }
            default: break
            }
            return true
        }
        #expect(try await ServerClient.read(.repoPath(id: "repo_1"), location: peer.location) == big)
        let output = try await ServerClient.configuration(.configTheme(path: "/unused", value: big), location: peer.location)
        #expect(String(decoding: output, as: UTF8.self) == "\(big.utf8.count)")
    }

    @Test func oversizedControlResponseIsRefusedEvenWhenItsLastReadContainsTheNewline() async throws {
        let peer = try ServerPeer()
        let big = String(repeating: "x", count: ServerProtocol.maxMessage + 1)
        peer.serve { message, respond in
            guard message["type"] as? String == "request" else { return true }
            respond(["type": "response", "id": message["id"]!, "trace_id": "trace", "result": big])
            return true
        }
        do {
            _ = try await ServerClient.read(.repoPath(id: "repo_1"), location: peer.location)
            Issue.record("An oversized response bypassed the control connection's limit")
        } catch { #expect(error is RegistryQueryError) }
    }

    @Test func serverEndingASubscriptionFailsItsWatchAlone() async throws {
        let peer = try ServerPeer()
        let subscriptionID = Mutex<String?>(nil)
        peer.serve { message, respond in
            switch (message["type"] as? String, (message["operation"] as? [String: Any])?["kind"] as? String) {
            case ("subscribe", _):
                let id = message["id"] as! String
                subscriptionID.set(id)
                respond(["type": "response", "id": id, "trace_id": "trace", "result": NSNull()])
                respond(["type": "reading", "id": id, "checkpoint": "fixture-checkpoint", "frame": ["answers": 1, "unavailable": NSNull(), "body": NSNull()]])
            case ("checkpoint", _):
                #expect(message["checkpoint"] as? Int == 4)
                let id = subscriptionID.get()!
                respond(["type": "reading", "id": id, "checkpoint": "fixture-checkpoint", "frame": ["answers": 4, "unavailable": "gone", "body": NSNull()]])
                respond(["type": "error", "id": id, "code": "unavailable", "message": "gone", "data": NSNull()])
            case ("request", _):
                respond(["type": "response", "id": message["id"]!, "trace_id": "trace", "result": "still here"])
            default: break
            }
            return true
        }
        let subscription = ServerClient.subscribe(.allPlans, location: peer.location)
        var frames = subscription.lines.makeAsyncIterator()
        let first = try #require(try await frames.next())
        #expect((try JSONSerialization.jsonObject(with: first) as? [String: Any])?["answers"] as? Int == 1)
        subscription.send(Data(#"{"id":4}"#.utf8 + [10]))
        let last = try #require(try await frames.next())
        #expect((try JSONSerialization.jsonObject(with: last) as? [String: Any])?["unavailable"] as? String == "gone")
        do {
            _ = try await frames.next()
            Issue.record("An ended subscription kept its stream open")
        } catch {
            #expect((error as? ServerError)?.code == .unavailable)
            #expect(error.localizedDescription == "gone")
        }
        // The connection outlives the subscription.
        #expect(try await ServerClient.read(.repoPath(id: "repo_1"), location: peer.location) == "still here")
        await subscription.cancel()
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
        let query = ServedRead.planList(directory: root.path, wave: nil, task: nil, all: true)
        do {
            _ = try await ServerClient.read(query, location: location)
            Issue.record("Initial setup must report its failure")
        } catch {
            #expect(error.localizedDescription.contains("fixture Background Items disabled"))
        }
        try Data().write(to: root.appendingPathComponent("setup-allowed"))
        // Both a refreshed reading and reopened subscriptions share one consumed
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
                        let subscription = ServerClient.subscribe(query, location: location)
                        do {
                            for try await _ in subscription.lines { Issue.record("The server is still unavailable") }
                            Issue.record("The subscription must report unavailability")
                        } catch {
                            #expect(error.localizedDescription.contains("fixture Background Items disabled"))
                        }
                        await subscription.cancel()
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
            let subscription = ServerClient.subscribe(query, location: location)
            do {
                var frames = subscription.lines.makeAsyncIterator()
                let first = try #require(try await frames.next())
                #expect(String(decoding: first, as: UTF8.self).contains("Recovered Work"))
            } catch {
                await subscription.cancel()
                throw error
            }
            await subscription.cancel()
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

/// One same-account peer speaking the current revision. Each `serve` accepts one
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
                    "type": "welcome", "client_id": "fixture-client", "revision": ServerProtocol.revision, "server_revision": ServerProtocol.revision, "min_revision": ServerProtocol.minRevision, "instance": "fixture",
                    "build": "test", "executable_sha256": String(repeating: "f", count: 64), "machine": NSNull(),
                    "limits": ["max_message": 1_048_576, "max_connections": 64],
                ])
                while let message = try connection.read() {
                    lock.withLock { messages.append(message) }
                    if !handle(message, { try? connection.write($0) }) { return }
                }
            } catch {}
        }
    }

    /// Accept one ticketed connection presenting `ticket`, admit it, then let
    /// `handle` exchange on it.
    func admit(_ ticket: String, initial: [String: Any]? = nil, _ handle: (PeerConnection) throws -> Void) throws {
        let fd = accept(listener, nil, nil)
        guard fd >= 0 else { throw RegistryQueryError("No ticketed connection arrived") }
        let connection = PeerConnection(fd: fd)
        defer { connection.close() }
        let presented = try connection.read()
        guard presented?["type"] as? String == "attach", presented?["ticket"] as? String == ticket else {
            throw RegistryQueryError("The ticketed connection presented \(presented ?? [:])")
        }
        // Coalesce admission and payload so the client's greeting read may
        // already contain the beginning of the bulk response.
        try connection.write([["type": "admitted"]] + (initial.map { [$0] } ?? []))
        try handle(connection)
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
        try write([message])
    }

    func write(_ messages: [[String: Any]]) throws {
        let bytes = try messages.reduce(into: Data()) { bytes, message in
            bytes.append(try JSONSerialization.data(withJSONObject: message))
            bytes.append(10)
        }
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
