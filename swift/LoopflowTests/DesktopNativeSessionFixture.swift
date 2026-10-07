#if os(macOS)
import Darwin
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// Owned synthetic native history and executable, prepared by desktop_performance.py.
/// The public CLI supplies Session identity and launch commands; no live Home is copied.
struct DesktopNativeSessionFixture: Decodable, Sendable {
    let cli: String
    let home: String
    let checkout: String
    let repo: String
    let taskId: String
    let issue: String
    let environment: [String: String]
    let sessionId: String
    let nativeId: String
    let transcript: String
    let history: String

    static func load() throws -> Self {
        let path = try #require(ProcessInfo.processInfo.environment["LOOPFLOW_TEST_NATIVE_FIXTURE"])
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(Self.self, from: Data(contentsOf: URL(fileURLWithPath: path)))
    }

    var query: RegistryQuery {
        RegistryQuery { args, _ in try await read(args) }
    }

    func read(_ args: [String]) async throws -> String {
        if args.prefix(2) == ["session", "connect"] {
            guard args.count == 4, args[2] == sessionId, args[3] == "--json" else {
                throw RegistryQueryError("Only the owned Session may connect")
            }
            let output = try await Task.detached { try capture([cli] + args) }.value
            var record = try #require(JSONSerialization.jsonObject(with: Data(output.utf8)) as? [String: Any])
            let argv = try #require(record["open_argv"] as? [String])
            record["open_argv"] = isolatedCommand(argv)
            return String(decoding: try JSONSerialization.data(withJSONObject: record), as: UTF8.self)
        }
        guard ["roadmap", "activity"].contains(args.first ?? "") || ["wave list", "home id", "session list", "session history", "task status", "task files", "task file", "task diff", "flow list"].contains(args.prefix(2).joined(separator: " ")) else {
            throw RegistryQueryError("Fixture does not execute this command")
        }
        return try await Task.detached { try capture([cli] + args) }.value
    }

    func records() async throws -> [SessionRecord] {
        let output = try await Task.detached {
            try capture([cli, "session", "list", "--all", "--history", "--json"])
        }.value
        return try JSONDecoder().decode([SessionRecord].self, from: Data(output.utf8))
    }

    func resume(_ record: SessionRecord) async throws -> String {
        try await Task.detached { try capture(record.openArgv) }.value
    }

    func isolatedCommand(_ argv: [String]) -> [String] {
        ["/usr/bin/env", "-i"] + environment.sorted { $0.key < $1.key }.map { "\($0.key)=\($0.value)" } + argv
    }

    func historyIsPreserved() throws -> Bool {
        try String(contentsOfFile: transcript, encoding: .utf8) == history
    }

    private func capture(_ argv: [String]) throws -> String {
        let output = URL(fileURLWithPath: home).appendingPathComponent("query-\(UUID().uuidString)")
        try Data().write(to: output)
        let errors = output.appendingPathExtension("stderr")
        try Data().write(to: errors)
        let errorHandle = try FileHandle(forWritingTo: errors)
        let handle = try FileHandle(forWritingTo: output)
        defer {
            try? handle.close(); try? errorHandle.close()
            try? FileManager.default.removeItem(at: output)
            try? FileManager.default.removeItem(at: errors)
        }
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = argv
        process.environment = environment
        // Exact Task destinations resolve within the Wave's repository. The
        // owned checkout remains the cwd for native execution. File commands use
        // the Task’s recorded checkout through the shared CLI reader.
        process.currentDirectoryURL = URL(fileURLWithPath: ["roadmap", "activity", "task", "flow", "wave"].contains(argv.dropFirst().first ?? "") ? repo : checkout)
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = handle
        process.standardError = errorHandle
        try process.run()
        let deadline = ContinuousClock.now + .seconds(60)
        while process.isRunning && ContinuousClock.now < deadline {
            Thread.sleep(forTimeInterval: 0.01)
        }
        if process.isRunning {
            process.terminate()
            Thread.sleep(forTimeInterval: 0.1)
            if process.isRunning { kill(process.processIdentifier, SIGKILL) }
            process.waitUntilExit()
            throw RegistryQueryError("Native fixture CLI timed out")
        }
        guard process.terminationStatus == 0 else {
            let detail = try String(contentsOf: errors, encoding: .utf8)
            throw RegistryQueryError("Native fixture CLI \(argv.dropFirst().joined(separator: " ")) failed (\(process.terminationStatus)): \(detail)")
        }
        return try String(contentsOf: output, encoding: .utf8)
    }
}
@Suite("Native Session reopen fixture", .serialized)
@MainActor
struct DesktopNativeSessionTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["LOOPFLOW_TEST_CANARY"] != nil))
    func ownedChildrenCanExitButHostSignalsStayDenied() async throws {
        #expect(kill(getppid(), 0) == -1 && errno == EPERM)
        let child = Process()
        let input = Pipe()
        child.executableURL = URL(fileURLWithPath: "/bin/cat")
        child.standardInput = input
        child.standardOutput = FileHandle.nullDevice
        child.standardError = FileHandle.nullDevice
        try child.run()
        child.terminate()
        let deadline = ContinuousClock.now + .seconds(3)
        while child.isRunning && ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        // EOF also releases a child if signaling fails, without hanging tests.
        try input.fileHandleForWriting.close()
        child.waitUntilExit()
        #expect(child.terminationReason == .uncaughtSignal)
        #expect(child.terminationStatus == SIGTERM)
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["LOOPFLOW_TEST_CANARY"] != nil))
    func ownedTemporaryFilesAndPTYRemainAvailable() throws {
        let directory = terminalTemporaryDirectory()
        let path = directory.appendingPathComponent("terminal-proof")
        defer { try? FileManager.default.removeItem(at: path) }
        try Data("owned".utf8).write(to: path)
        #expect(try Data(contentsOf: path) == Data("owned".utf8))
        var master: Int32 = -1
        var slave: Int32 = -1
        let result = openpty(&master, &slave, nil, nil, nil)
        defer {
            if master >= 0 { close(master) }
            if slave >= 0 { close(slave) }
        }
        #expect(result == 0, "Owned PTY allocation failed: errno \(errno)")
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["LOOPFLOW_TEST_CANARY"] != nil))
    func inheritedSandboxRejectsExternalEffects() throws {
        let canary = try #require(ProcessInfo.processInfo.environment["LOOPFLOW_TEST_CANARY"])
        for script in ["IFS= read -r line < \"$1\"", ": > \"$1\"", "exec \"$1\"", "exec /bin/launchctl list"] {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/bin/sh")
            process.arguments = ["-c", script, "probe", canary]
            process.standardOutput = FileHandle.nullDevice
            process.standardError = FileHandle.nullDevice
            try process.run()
            process.waitUntilExit()
            #expect(process.terminationStatus != 0)
        }
        let descriptor = socket(AF_INET, SOCK_STREAM, 0)
        if descriptor >= 0 {
            defer { close(descriptor) }
            var address = sockaddr_in()
            address.sin_family = sa_family_t(AF_INET)
            address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
            address.sin_addr.s_addr = inet_addr("127.0.0.1")
            let result = withUnsafePointer(to: &address) {
                $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(descriptor, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) }
            }
            #expect(result == -1 && errno == EPERM)
        } else { #expect(errno == EPERM) }
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["LF_DESKTOP_TASK_SNAPSHOT"] != nil))
    func combinedWorkspacePreservesTaskMembershipAndRejectsCopiedClients() async throws {
        let fixture = try DesktopNativeSessionFixture.load()
        let snapshot = try #require(ProcessInfo.processInfo.environment["LF_DESKTOP_TASK_SNAPSHOT"])
        let reads = SnapshotReads()
        let query = RegistryQuery { args, _ in
            try await reads.read(binary: fixture.cli, home: snapshot, args: args, cwd: fixture.repo, fixture: fixture).get()
        }
        let destination = try await query.taskDestination(issue: fixture.issue, repo: fixture.repo)
        #expect(destination.waves.flatMap { $0.tasks.items }.contains { $0.runtime?.workId == fixture.taskId })
        let copied = try await reads.read(binary: fixture.cli, home: snapshot,
            args: ["session", "list", "--all", "--history", "--json"], cwd: fixture.repo).get()
        let copiedRecords = try JSONDecoder().decode([SessionRecord].self, from: Data(copied.utf8))
        let copiedRecord = try #require(copiedRecords.first)
        let activityJSON = try await reads.read(binary: fixture.cli, home: snapshot,
            args: ["activity", "--since", "7d", "--limit", "50", "--json"], cwd: fixture.repo).get()
        let activity = try JSONDecoder().decode(WorkActivitySnapshot.self, from: Data(activityJSON.utf8))
        #expect(activity.items.contains { $0.work == copiedRecord.work })
        await #expect(throws: (any Error).self) { try await query.openSession(id: copiedRecord.id) }
        let model = WorkModel(query: query, repoPath: fixture.repo)
        await model.refresh()
        let originalIDs = Set(model.projection.waves.flatMap { $0.tasks.map { $0.task.id } })
        #expect(originalIDs.contains(fixture.taskId))
        // Closed copied conversations belong to explicit history, not the
        // current working set. The unfiltered transport retains them unchanged.
        let history = try await reads.read(binary: fixture.cli, home: snapshot,
            args: ["session", "list", "--all", "--history", "--json"], cwd: fixture.repo, fixture: fixture).get()
        #expect(try JSONDecoder().decode([SessionRecord].self, from: Data(history.utf8)).count == copiedRecords.count + 1)
        let owned = try #require(try await fixture.records().first)
        let registry = SessionsWorkspaceRegistry(localHomeId: try await query.localHomeId())
        let workspace = registry.workspace(for: try #require(owned.workspace).identity)
        let store = workspace.sessionStore(repoPath: fixture.repo, query: query)
        workspace.multiplexer.load(sessionId: fixture.sessionId)
        workspace.multiplexer.newShell()
        let layout = workspace.multiplexer.layout
        let files = workspace.files(taskId: fixture.taskId, issue: fixture.issue, cwd: fixture.checkout, query: query)
        files.autosave = false
        files.selection = "notes.txt"
        await files.loadFile()
        let document = try #require(files.selectedDocument)
        #expect(document.snapshot != nil)
        document.editor.insertText("Preserved draft", replacementRange: NSRange(location: 0, length: document.editor.string.utf16.count))
        document.editor.setSelectedRange(NSRange(location: 2, length: 3))
        var link = URLComponents()
        link.scheme = "loopflow"; link.host = "task"; link.path = "/" + fixture.issue
        link.queryItems = [URLQueryItem(name: "repo", value: fixture.repo), URLQueryItem(name: "session", value: fixture.sessionId)]
        for _ in 0..<3 {
            await model.openTaskLink(try #require(link.url))
            #expect(model.selection?.id == fixture.taskId)
            #expect(model.linkedSession?.taskIds.contains(fixture.taskId) == true)
            store.reconcile(try await fixture.records())
            await store.select(fixture.sessionId)
            let prepared = try #require(store.sessions.first?.surface)
            #expect(try await fixture.resume(prepared).contains("native:\(fixture.nativeId):retained-history"))
            store.recordPaneLive(fixture.sessionId)
            store.noteSurfaceClosed(.session(fixture.sessionId))
            await model.refresh()
            await files.refresh()
            await files.loadFile()
            #expect(document.text == "Preserved draft")
            #expect(document.editor.selectedRange() == NSRange(location: 2, length: 3))
            #expect(workspace.multiplexer.layout == layout)
            #expect(Set(model.projection.waves.flatMap { $0.tasks.map { $0.task.id } }) == originalIDs)
            #expect(try fixture.historyIsPreserved())
        }
        let after = try await reads.read(binary: fixture.cli, home: snapshot,
            args: ["session", "list", "--all", "--history", "--json"], cwd: fixture.repo).get()
        #expect(try JSONDecoder().decode([SessionRecord].self, from: Data(after.utf8)) == copiedRecords)
        await reads.finish()
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["LOOPFLOW_TEST_NATIVE_FIXTURE"] != nil))
    func reopenPreservesSessionAndNativeHistory() async throws {
        let fixture = try DesktopNativeSessionFixture.load()
        let store = SessionsStore(repoPath: fixture.home, query: fixture.query)
        for _ in 0..<3 {
            store.reconcile(try await fixture.records())
            await store.select(fixture.sessionId)
            let prepared = try #require(store.sessions.first?.surface)
            #expect(prepared.id == fixture.sessionId)
            #expect(prepared.taskIds.contains(fixture.taskId))
            let result = try await fixture.resume(prepared)
            #expect(result.contains("native:\(fixture.nativeId):retained-history"))
            store.recordPaneLive(fixture.sessionId)
            store.noteSurfaceClosed(.session(fixture.sessionId))
            let records = try await fixture.records()
            #expect(records.map(\.id) == [fixture.sessionId])
            #expect(try fixture.historyIsPreserved())
        }
    }
}
#endif
