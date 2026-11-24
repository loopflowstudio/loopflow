import Darwin
import Foundation
import Loopflow
import XCTest
@testable import LoopflowiOS

/// Two private Machines behind the phone: the laptop, the one Machine the
/// phone is paired with, and mini, which the laptop's server reaches over a
/// local stand-in for `ssh`. The phone's control connection is the laptop
/// server's own socket: the Simulator shares the Mac's Tailscale identity, and
/// the TLS WebSocket that carries it on a real phone is covered by the
/// server's paired-phone fixture. A terminal's ticket is redeemed over the
/// same socket. Everything is set up with the CLI a person would run, run by
/// the host `scripts/test_ios.py` starts.
@MainActor
final class RemoteTaskWorkspaceTests: XCTestCase {
    func testEveryViewOpensOnTheOwningMachine() async throws {
        try await withTwoMachines { world in
            let client = try await world.phone()
            let task = try await world.createTask("Remote work")
            _ = try await world.laptop.run(["task", "move", task.id, "--machine", "mini", "--json"], cwd: world.repo)

            // Not started: mini owns the Task but has no checkout for it yet.
            let files = TaskFilesTree(client: client, issue: task.id)
            await files.load("")
            XCTAssertTrue(files.notStarted)
            do {
                _ = try await client.query.taskChanges(issue: task.id)
                XCTFail("Changes of an unstarted Task must say Not started")
            } catch let refusal as ServerError { XCTAssertTrue(refusal.notStarted, "\(refusal)") }
            let shell = TerminalLink(client: client, task: task, target: .shell)
            shell.fit(columns: 80, rows: 24)
            try await eventually("the shell says Not started", showing: { "\(shell.state)" }) { shell.state == .notStarted }
            // The one watch reads the unstarted Task, then fills Files in when mini checks it out.
            let watching = Task { await files.follow() }
            defer { watching.cancel() }
            try await eventually("the watch names mini before a checkout") { files.machineID != nil }
            XCTAssertTrue(files.notStarted)
            XCTAssertNil(files.checkout)

            let checkout = try await world.miniCheckout(task.id)
            try "changed on mini\n".write(to: checkout.appendingPathComponent("notes.txt"), atomically: true, encoding: .utf8)

            // Files: the tree and an unchanged file read from mini; the laptop never had a checkout.
            try await eventually("Files fills in once mini has the checkout", showing: { "\(files.listings)" }) {
                !files.notStarted && files.listing("")?.entries.map(\.path).contains("document.txt") == true
            }
            XCTAssertEqual(files.checkout, checkout.path)
            let entries = try XCTUnwrap(files.listing("")).entries.map(\.path)
            XCTAssertTrue(entries.contains("document.txt"), "\(entries)")
            let document = try await client.query.taskFile(issue: task.id, path: "document.txt")
            XCTAssertEqual(document.content, "unchanged on mini\n")

            // Changes: mini's uncommitted hunk.
            let changes = try await client.query.taskChanges(issue: task.id)
            XCTAssertTrue(changes.files.map(\.path).contains("notes.txt"), "\(changes.files)")
            let diff = try await client.query.taskDiff(issue: task.id, path: "notes.txt")
            XCTAssertTrue(diff.patch.contains("+changed on mini"), diff.patch)

            // Shell: the phone's shell starts in mini's checkout, and repeating its ID starts nothing.
            let size = TerminalGeometry(columns: 80, rows: 24)
            world.shells.append(task.id)
            try await client.start(.shell, task: task.id, request: "", size: size)
            try await client.start(.shell, task: task.id, request: "", size: size)
            let terminal = try await world.redeem(try await client.ticket(.shell, task: task.id))
            try terminal.type("pwd\r")
            try await terminal.waitFor(checkout.path)

            // Session: the Task's primary, saved on mini, opens from the phone and takes input.
            let prepared = try await world.mini.run(["session", "resume", "--task", task.id, "--prepare", "--json", "-a", "claude"],
                                                    cwd: checkout)
            let session = try JSONDecoder().decode(SessionRecord.self, from: Data(prepared.utf8)).id
            let primary = try await client.primarySession(task: task.id)
            XCTAssertEqual(primary, session)
            try await client.start(.session(session), task: task.id, request: UUID().uuidString.lowercased(), size: size)
            let viewer = try await world.redeem(try await client.ticket(.session(session), task: task.id))
            // Mini's server may start the prepared conversation before the viewer
            // attaches, so the provider's reply, not its first line, says where it runs.
            try viewer.type("hello\r")
            try await viewer.waitFor("mini received hello in \(checkout.path)")
            try viewer.type("quit\r")
            try await world.closeShell(task.id, startedOn: nil)

            // Mini goes away: the open watch and a new read both name it.
            await world.miniServer.stop()
            try world.disconnect(true)
            await files.load("")
            guard case .failed(let failure)? = files.listings[""] else { return XCTFail("\(String(describing: files.listings[""]))") }
            XCTAssertTrue(failure.contains("mini"), failure)
            try await eventually("the watch reports mini's outage", showing: { "\(String(describing: files.unavailable))" }) {
                files.unavailable?.contains("mini") == true
            }

            // Mini returns: Try again reads the same checkout.
            try world.disconnect(false)
            try await world.miniServer.start()
            try await eventually("the watch recovers") { files.unavailable == nil }
            await files.load("")
            XCTAssertTrue(files.listing("")?.entries.map(\.path).contains("document.txt") == true)
        }
    }

    func testAMoveIsFollowedAndAnOldShellAsksForANewOne() async throws {
        try await withTwoMachines { world in
            let client = try await world.phone()
            let task = try await world.createTask("Moving work")
            let laptopCheckout = try await world.laptop.json(["task", "checkout", task.id, "--json"], cwd: world.repo,
                                                             as: Checkout.self).worktree
            let size = TerminalGeometry(columns: 80, rows: 24)
            try await client.start(.shell, task: task.id, request: "", size: size)
            world.shells.append(task.id)

            let files = TaskFilesTree(client: client, issue: task.id)
            await files.load("")
            let watching = Task { await files.follow() }
            defer { watching.cancel() }
            try await eventually("the watch reads the laptop's checkout") { files.checkout == laptopCheckout }

            _ = try await world.laptop.run(["task", "move", task.id, "--machine", "mini", "--json"], cwd: world.repo)
            let moved = try await world.miniCheckout(task.id)
            try "only on mini\n".write(to: moved.appendingPathComponent("mini.txt"), atomically: true, encoding: .utf8)

            // Files follows the Task to mini and reads its tree again.
            try await eventually("Files follows the Task to mini", showing: { "\(String(describing: files.machineID)) \(String(describing: files.unavailable))" }) {
                files.machineID.map { StructuredIdentity.prefixed($0, "home_") } == StructuredIdentity.prefixed(world.miniID, "home_")
                    && files.listing("")?.entries.map(\.path).contains("mini.txt") == true
            }
            XCTAssertEqual(files.checkout, moved.path)
            let changes = try await client.query.taskChanges(issue: task.id)
            XCTAssertTrue(changes.files.map(\.path).contains("mini.txt"), "\(changes.files)")

            // The phone's shell was started on the laptop. Reattaching reaches mini,
            // which never started it: the shell has ended there, and New shell starts one in mini's checkout.
            do {
                _ = try await client.ticket(.shell, task: task.id)
                XCTFail("mini must not attach a shell the laptop started")
            } catch is SessionNotRunning {}
            try await client.start(.shell, task: task.id, request: "", size: size)
            let terminal = try await world.redeem(try await client.ticket(.shell, task: task.id))
            try terminal.type("pwd\r")
            try await terminal.waitFor(moved.path)
            try await world.closeShell(task.id, startedOn: nil)
            try await world.closeShell(task.id, startedOn: world.laptopID)
        }
    }
}

private struct Checkout: Decodable { let worktree: String }

extension TaskFilesTree {
    /// What `path` lists now, once read.
    fileprivate func listing(_ path: String) -> Listing? {
        guard case .value(let listing)? = listings[path] else { return nil }
        return listing
    }
}

/// Both Machines serving; afterwards, however `body` ends, shells close and both servers stop.
@MainActor
private func withTwoMachines(_ body: (TwoMachines) async throws -> Void) async throws {
    let world = try await TwoMachines()
    defer { world.remove() }
    do {
        try await world.serve()
        try await body(world)
    } catch {
        await world.stop()
        throw error
    }
    await world.stop()
}

/// The laptop and mini, each with its own home, store and server.
@MainActor
private final class TwoMachines {
    let root: URL
    let laptop: HostLf
    let mini: HostLf
    let laptopServer: HostServer
    let miniServer: HostServer
    /// The laptop's clone of the shared origin.
    let repo: URL
    private(set) var laptopID = ""
    private(set) var miniID = ""
    /// Tasks given a phone shell, closed on both Machines however the test ends.
    var shells: [String] = []

    init() async throws {
        root = URL(fileURLWithPath: "/tmp").appendingPathComponent("lf-ios-\(UUID().uuidString.prefix(8))")
        let fm = FileManager.default
        let local = root.appendingPathComponent("local"), remote = root.appendingPathComponent("remote")
        for dir in [root.appendingPathComponent("bin"), local, remote.appendingPathComponent("bin"),
                    remote.appendingPathComponent(".local/bin"), remote.appendingPathComponent("store")] {
            try fm.createDirectory(at: dir, withIntermediateDirectories: true)
        }
        let binary = try HostLf.binary()
        let remotePath = remote.appendingPathComponent("bin").path + ":/usr/bin:/bin"
        // `ssh mini <command>` runs the command in mini's home, as a login there would.
        try Self.executable(root.appendingPathComponent("bin/ssh"), """
            #!/bin/sh
            [ -e "$0.offline" ] && exit 255
            for arg in "$@"; do remote_command="$arg"; done
            cd \(shellEscape(remote.path)) || exit 1
            exec env -i HOME=\(shellEscape(remote.path)) PATH=\(shellEscape(remotePath)) bash -c "$remote_command"
            """)
        try Self.executable(remote.appendingPathComponent(".local/bin/lf"), """
            #!/bin/sh
            export LF_HOME=\(shellEscape(remote.appendingPathComponent("store").path)) LF_BIN=\(shellEscape(binary))
            exec \(shellEscape(binary)) "$@"
            """)
        // Mini's provider: it answers each line typed into it with where it runs.
        try Self.executable(remote.appendingPathComponent("bin/claude"), """
            #!/bin/bash
            if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
            while IFS= read -r -t 60 line < /dev/tty; do
              printf 'mini received %s in %s\\n' "$line" "$PWD"
              [ "$line" = quit ] && exit 0
            done
            """)
        try Data(#"{"projects":{"/":{"hasTrustDialogAccepted":true}}}"#.utf8)
            .write(to: remote.appendingPathComponent(".claude.json"))
        laptop = HostLf(binary: binary, home: local,
                        overriding: ["PATH": root.appendingPathComponent("bin").path + ":/usr/bin:/bin"])
        mini = HostLf(binary: binary, home: remote,
                      overriding: ["LF_HOME": remote.appendingPathComponent("store").path, "PATH": remotePath])
        laptopServer = HostServer(laptop)
        miniServer = HostServer(mini)

        let origin = root.appendingPathComponent("origin.git")
        repo = local.appendingPathComponent("repo")
        _ = try await laptop.run(["init", "-q", "--bare", "-b", "main", origin.path], executable: "/usr/bin/git")
        _ = try await laptop.run(["clone", "-q", origin.path, repo.path], executable: "/usr/bin/git")
        for args in [["config", "user.name", "Fixture Person"], ["config", "user.email", "fixture@example.invalid"]] {
            _ = try await laptop.run(args, cwd: repo, executable: "/usr/bin/git")
        }
        try "unchanged on mini\n".write(to: repo.appendingPathComponent("document.txt"), atomically: true, encoding: .utf8)
        try "first notes\n".write(to: repo.appendingPathComponent("notes.txt"), atomically: true, encoding: .utf8)
        for args in [["add", "."], ["commit", "-qm", "Shared files"], ["push", "-q", "origin", "main"]] {
            _ = try await laptop.run(args, cwd: repo, executable: "/usr/bin/git")
        }
    }

    /// Migrate both stores, start mini's server, register mini, then start the laptop's.
    /// A server's readiness is measured on a migrated store, never on its first open.
    func serve() async throws {
        struct Identity: Decodable { let id: String }
        for lf in [mini, laptop] { _ = try await lf.run(["desktop", "portfolio", "--json"]) }
        try await miniServer.start()
        miniID = try await laptop.json(["machine", "add", "mini", "--json"], as: Identity.self).id
        laptopID = try await laptop.json(["machine", "id", "--json"], as: Identity.self).id
        try await laptopServer.start()
    }

    /// The phone, paired with the laptop: its one control connection is the laptop's server.
    func phone() async throws -> RemoteMobileClient {
        let endpoint = ServerEndpoint(
            client: ServerClientIdentity(kind: "mobile", platform: "ios", name: "Loopflow", version: "test"),
            control: { [socket = laptopServer.socket] in try UnixCarrier(path: socket, limit: ServerProtocol.maxMessage) },
            ticketed: { [socket = laptopServer.socket] in try UnixCarrier(path: socket, limit: ServerProtocol.maxBulk) })
        let link = try await ServerLink.open(endpoint)
        defer { Task { await link.close() } }
        guard case .object(let machine) = link.welcome.machine, case .string(let id)? = machine["id"] else {
            throw RegistryQueryError("The laptop did not say who it is")
        }
        // Terminals open their own connection there; the phone's WebSocket address is never dialled here.
        return RemoteMobileClient(savedMachine: SavedMachine(endpoint: URL(string: "https://laptop.invalid")!, machineID: id),
                                  endpoint: endpoint)
    }

    func createTask(_ title: String) async throws -> MobileTask {
        struct Created: Decodable {
            struct Task: Decodable { let id: String }
            let task: Task
        }
        let id = try await laptop.json(["task", "create", "--title", title, "--json"], cwd: repo, as: Created.self).task.id
        let row = try JSONSerialization.data(withJSONObject: [
            "work": ["repository": "repo_fixture", "kind": "task", "id": id], "title": title, "states": [],
            "identifier": NSNull(), "primary_session": NSNull(), "location": NSNull(), "argv": NSNull(), "unavailable": [],
        ] as [String: Any])
        return MobileTask(try JSONDecoder().decode(WorkViewPlan.Content.self, from: row))
    }

    /// Mini makes the Task's checkout, cloning the shared origin.
    func miniCheckout(_ task: String) async throws -> URL {
        URL(fileURLWithPath: try await laptop.json(["--machine", "mini", "task", "checkout", task, "--json"],
                                                   cwd: repo, as: Checkout.self).worktree)
    }

    /// Present `ticket` on the laptop's socket, as a phone's terminal connection does.
    func redeem(_ ticket: String) async throws -> TerminalConnection {
        try await TerminalConnection.open(socket: laptopServer.socket, ticket: ticket)
    }

    func disconnect(_ offline: Bool) throws {
        let marker = root.appendingPathComponent("bin/ssh.offline")
        if offline { try Data().write(to: marker) } else { try FileManager.default.removeItem(at: marker) }
    }

    /// End the phone's shell in `task` on the Machine that started it; nil is where the Task runs.
    func closeShell(_ task: String, startedOn machine: String?) async throws {
        _ = try await laptop.run(["desktop", "shell", task, "--id", RemoteMobileClient.shellID(task: task), "--close"]
                                 + (machine.map { ["--started-on", $0] } ?? []), cwd: repo)
    }

    func stop() async {
        for task in shells {
            try? await closeShell(task, startedOn: laptopID)
            try? await closeShell(task, startedOn: nil)
        }
        await laptopServer.stop()
        await miniServer.stop()
    }

    nonisolated func remove() {
        try? FileManager.default.removeItem(at: root)
    }

    private static func executable(_ url: URL, _ script: String) throws {
        try Data((script + "\n").utf8).write(to: url)
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: url.path)
    }
}

// MARK: The host's commands

/// The source-built `lf` in a private Machine, run by the host
/// `scripts/test_ios.py` started (`LOOPFLOW_FIXTURE_HOST`).
private struct HostLf {
    let binary: String
    let environment: [String: String]
    let home: URL

    init(binary: String, home: URL, overriding: [String: String]) {
        self.binary = binary
        self.home = home
        environment = ["PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": home.path,
                       "LF_HOME": home.path, "LF_BIN": binary, "LF_USER_NAME": "Fixture Person",
                       "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
                       "GIT_ALLOW_PROTOCOL": "file", "GIT_TERMINAL_PROMPT": "0"]
            .merging(overriding) { $1 }
    }

    static func binary() throws -> String {
        let path = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("target/debug/lf").path
        guard FileManager.default.isExecutableFile(atPath: path) else {
            throw RegistryQueryError("Build with cargo build -p loopflow --bin lf")
        }
        return path
    }

    /// Captured stdout; a nonzero exit throws its stderr.
    func run(_ args: [String], cwd: URL? = nil, executable: String? = nil) async throws -> String {
        let argv = [executable ?? binary] + args
        let answer = try await FixtureHost.request(["argv": argv, "env": environment, "cwd": (cwd ?? home).path])
        guard answer["status"] as? Int == 0 else {
            throw RegistryQueryError("\(argv.joined(separator: " ")) exited \(answer["status"] ?? "?"): \(answer["stderr"] ?? "")")
        }
        return answer["stdout"] as? String ?? ""
    }

    func json<T: Decodable>(_ args: [String], cwd: URL? = nil, as type: T.Type) async throws -> T {
        try JSONDecoder().decode(type, from: Data(try await run(args, cwd: cwd).utf8))
    }

    func start(_ args: [String]) async throws -> Int {
        let answer = try await FixtureHost.request(["argv": [binary] + args, "env": environment, "cwd": home.path,
                                                    "background": true])
        guard let pid = answer["pid"] as? Int else { throw RegistryQueryError("Not started: \(answer)") }
        return pid
    }
}

/// One Machine's `lf server run`, started and stopped through the host.
@MainActor
private final class HostServer {
    let lf: HostLf
    private(set) var socket = ""
    private var pid: Int?

    init(_ lf: HostLf) { self.lf = lf }

    func start() async throws {
        pid = try await lf.start(["server", "run"])
        let deadline = ContinuousClock.now + .seconds(20)
        while true {
            let status = try JSONSerialization.jsonObject(with: Data(try await lf.run(["server", "status"]).utf8)) as? [String: Any]
            if status?["ready"] as? Bool == true, let socket = status?["socket"] as? String {
                self.socket = socket
                return
            }
            guard ContinuousClock.now < deadline else { throw RegistryQueryError("Private server did not become ready: \(String(describing: status))") }
            try await Task.sleep(for: .milliseconds(50))
        }
    }

    func stop() async {
        guard let pid else { return }
        self.pid = nil
        _ = try? await FixtureHost.request(["stop": pid])
    }
}

private enum FixtureHost {
    /// One request line, one answer line, on a connection of its own.
    static func request(_ request: [String: Any]) async throws -> [String: Any] {
        guard let path = ProcessInfo.processInfo.environment["LOOPFLOW_FIXTURE_HOST"] else {
            throw RegistryQueryError("Run through scripts/test_ios.py, which hosts the Machines these tests start")
        }
        var line = try JSONSerialization.data(withJSONObject: request)
        line.append(10)
        let answer = try await Task.detached {
            let fd = try connectUnix(path)
            defer { Darwin.close(fd) }
            try writeAll(fd, line)
            return try readLine(fd)
        }.value
        return try XCTUnwrap(JSONSerialization.jsonObject(with: answer) as? [String: Any])
    }
}

// MARK: The laptop server's socket

/// The phone's control connection, carried over the laptop server's socket.
private final class UnixCarrier: ServerCarrier, @unchecked Sendable {
    private let fd: Int32
    private let limit: Int
    private let reads = DispatchQueue(label: "phone.read")
    private var buffer = Data()

    init(path: String, limit: Int) throws {
        fd = try connectUnix(path)
        self.limit = limit
    }

    deinit { Darwin.close(fd) }

    func send(_ message: Data) async throws {
        var line = message
        line.append(10)
        try writeAll(fd, line)
    }

    func receive() async throws -> Data? {
        try await withCheckedThrowingContinuation { continuation in
            reads.async { [self] in
                continuation.resume(with: Result {
                    while true {
                        if let newline = buffer.firstIndex(of: 10) {
                            let message = buffer[buffer.startIndex..<newline]
                            buffer.removeSubrange(buffer.startIndex...newline)
                            return Data(message)
                        }
                        guard buffer.count <= limit else { throw RegistryQueryError("Server message too large") }
                        let chunk = try readSome(fd)
                        if chunk.isEmpty { return nil }
                        buffer.append(chunk)
                    }
                })
            }
        }
    }

    func close() async { Darwin.shutdown(fd, SHUT_RDWR) }
}

/// A terminal ticket presented on the laptop's socket: after `bridged`, the
/// owner protocol, decoded as the phone's attachment decodes it.
private final class TerminalConnection: @unchecked Sendable {
    private let fd: Int32
    private var decoder = OwnerStream()
    private var text = ""
    private var pending = Data()

    private init(fd: Int32) { self.fd = fd }

    deinit { Darwin.close(fd) }

    static func open(socket: String, ticket: String) async throws -> TerminalConnection {
        let fd = try connectUnix(socket)
        let connection = TerminalConnection(fd: fd)
        var line = try JSONEncoder().encode(ServerClientMessage.attach(ticket: ticket))
        line.append(10)
        try writeAll(fd, line)
        let answer = try await Task.detached { try readLine(fd) }.value
        guard case .bridged = try ServerMessage(line: Data(answer.prefix(while: { $0 != 10 }))) else {
            throw RegistryQueryError("The ticket was not bridged: \(String(decoding: answer, as: UTF8.self))")
        }
        connection.decoder.expectAttachment(1)
        try writeAll(fd, OwnerStream.attachRequest(id: 1, columns: 80, rows: 24, name: "Loopflow iPhone"))
        return connection
    }

    func type(_ text: String) throws { try writeAll(fd, OwnerStream.input(Data(text.utf8))) }

    /// Read the terminal until it has shown `expected`. Line breaks are ignored: a snapshot
    /// soft-wraps a line longer than the 80 columns, as a long checkout path is on CI.
    func waitFor(_ expected: String) async throws {
        let deadline = ContinuousClock.now + .seconds(20)
        while !text.filter({ $0 != "\r" && $0 != "\n" && $0 != "\r\n" }).contains(expected) {
            guard ContinuousClock.now < deadline else { throw RegistryQueryError("The terminal never showed \(expected): \(text)") }
            let fd = fd
            let bytes = try await Task.detached { try readSome(fd, timeout: 1) }.value
            for event in try decoder.receive(bytes) {
                switch event {
                case .snapshot(let data), .output(let data): text += String(decoding: data, as: UTF8.self)
                default: break
                }
            }
        }
    }
}

private func connectUnix(_ path: String) throws -> Int32 {
    var address = sockaddr_un()
    address.sun_family = sa_family_t(AF_UNIX)
    guard path.utf8.count < MemoryLayout.size(ofValue: address.sun_path) else { throw RegistryQueryError("Socket path too long: \(path)") }
    withUnsafeMutableBytes(of: &address.sun_path) { $0.copyBytes(from: path.utf8) }
    let fd = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
    let connected = withUnsafePointer(to: &address) {
        $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) }
    }
    guard fd >= 0, connected == 0 else {
        Darwin.close(fd)
        throw RegistryQueryError("Cannot connect to \(path): \(String(cString: strerror(errno)))")
    }
    var enabled: Int32 = 1
    setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &enabled, socklen_t(MemoryLayout<Int32>.size))
    return fd
}

private func writeAll(_ fd: Int32, _ bytes: Data) throws {
    try bytes.withUnsafeBytes { buffer in
        var offset = 0
        while offset < buffer.count {
            let sent = Darwin.write(fd, buffer.baseAddress!.advanced(by: offset), buffer.count - offset)
            if sent < 0 && errno == EINTR { continue }
            guard sent > 0 else { throw RegistryQueryError("Socket write failed") }
            offset += sent
        }
    }
}

/// What has arrived, waiting up to `timeout` seconds; empty at the end of the stream or after the wait.
private func readSome(_ fd: Int32, timeout: Int? = nil) throws -> Data {
    if let timeout {
        var poll = pollfd(fd: fd, events: Int16(POLLIN), revents: 0)
        guard Darwin.poll(&poll, 1, Int32(timeout * 1000)) > 0 else { return Data() }
    }
    var chunk = [UInt8](repeating: 0, count: 65536)
    let count = Darwin.read(fd, &chunk, chunk.count)
    guard count >= 0 else { throw RegistryQueryError("Socket read failed") }
    return Data(chunk.prefix(count))
}

/// One newline-ended line, read a byte at a time so nothing after it is taken.
private func readLine(_ fd: Int32) throws -> Data {
    var line = Data()
    var byte: UInt8 = 0
    while Darwin.read(fd, &byte, 1) == 1 {
        line.append(byte)
        if byte == 10 { return line }
    }
    throw RegistryQueryError("The connection closed before answering: \(String(decoding: line, as: UTF8.self))")
}

private func shellEscape(_ value: String) -> String {
    "'" + value.replacingOccurrences(of: "'", with: "'\\''") + "'"
}

@MainActor
private func eventually(_ what: String, timeout: Duration = .seconds(20),
                        showing state: () -> String = { "" },
                        _ condition: () -> Bool) async throws {
    let deadline = ContinuousClock.now + timeout
    while !condition() {
        guard ContinuousClock.now < deadline else { throw RegistryQueryError("Timed out: \(what) \(state())") }
        try await Task.sleep(for: .milliseconds(50))
    }
}
