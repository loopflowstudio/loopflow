#if os(macOS)
import Darwin
import Foundation
import Testing
import Loopflow
@testable import LoopflowMac

/// Two private Machines behind Desktop: the laptop, whose server Desktop asks,
/// and mini, reached by the laptop's server over a local stand-in for `ssh`.
/// Everything is set up with the CLI a person would run; nothing writes SQL.
@Suite("A Task on another Machine through Desktop", .serialized)
@MainActor
struct RemoteTaskWorkspaceTests {
    @Test("Files, Changes, Save, Shell and Session reach mini's checkout; Not started and an outage say so")
    func everyViewOpensOnTheOwningMachine() async throws {
        let world = try await TwoMachines()
        defer { world.remove() }
        try await world.serve()
        let query = world.laptop.query(world.laptopServer)
        let task = try await world.createTask("Remote work")
        _ = try await world.laptop.run(["task", "move", task, "--machine", "mini", "--json"], cwd: world.repo)

        // Not started: mini owns the Task but has no checkout for it yet.
        let files = TaskFilesStore(issue: task, cwd: "", machine: world.miniID, machineLabel: "mini", query: query)
        files.autosave = false
        await files.refreshChanges()
        #expect(files.notStarted)
        await files.readDirectory("", more: false)
        #expect(files.notStarted)
        files.observeFiles()
        let workspace = SessionsWorkspace(key: WorkWorkspaceKey(repository: world.repo.path, kind: .task, id: task),
            title: "Remote work", location: WorkspaceIdentity(machineId: world.miniID, worktree: ""),
            localMachineId: world.laptopID)
        _ = workspace.sessionStore(repoPath: world.repo.path, query: query)
        let shell = try #require(try workspace.newShell(command: [], focus: false)?.content.selectedShell)
        // The attachment ends at the server's refusal; Desktop asks why in typed form.
        NotificationCenter.default.post(name: .ghosttySurfaceClosed, object: shell.terminal)
        try await eventually("the ended shell says Not started") { workspace.notStartedShells.contains(shell.id) }
        let notStarted = try await world.laptop.terminal(world.shell(task, id: "unstarted", run: "pwd"))
        #expect(notStarted.status != 0)

        let checkout = try await world.miniCheckout(task)
        try "changed on mini\n".write(to: checkout.appendingPathComponent("notes.txt"), atomically: true, encoding: .utf8)
        await files.retry()
        #expect(!files.notStarted)
        #expect(files.machineName == "mini")

        // An unchanged file reads from mini, even though the laptop never had a checkout.
        let entries = files.tree.listings[""]?.entries.map(\.path) ?? []
        #expect(entries.contains("document.txt"), "\(entries)")
        files.selectFile("document.txt", mode: .read)
        await files.loadFile()
        #expect(files.document("document.txt").snapshot?.content == "unchanged on mini\n")

        // Changes shows mini's uncommitted hunk; Save writes it there.
        await files.refreshChanges()
        #expect(files.changesError == nil)
        #expect(files.changes?.files.map(\.path).contains("notes.txt") == true)
        files.selectFile("notes.txt", mode: .edit)
        await files.loadFile()
        let notes = files.document("notes.txt")
        #expect(notes.snapshot?.content == "changed on mini\n")
        notes.editor.string = "saved from the laptop\n"
        await notes.save(issue: task, query: query)
        #expect(!notes.saveFailed, "\(notes.saveMessage ?? "")")
        #expect(try String(contentsOf: checkout.appendingPathComponent("notes.txt"), encoding: .utf8) == "saved from the laptop\n")

        // Shell runs on mini, in mini's checkout.
        let ran = try await world.laptop.terminal(world.shell(task, id: "shell-1", run: "hostname; pwd"))
        #expect(ran.output.contains(checkout.path), "\(ran.output)")

        // Session: the argv Desktop's pane runs attaches to mini's provider and takes input.
        let session = try await world.mini.run(["session", "resume", "--prepare", "--json", "-a", "claude"], cwd: checkout)
        let id = try JSONDecoder().decode(SessionRecord.self, from: Data(session.utf8)).id
        let record = try await query.prepareSession(id: id, work: WorkReference(kind: .task, id: task))
        let viewer = try world.laptop.startTerminal(record.openArgv)
        try await viewer.waitFor("provider on mini in \(checkout.path)")
        viewer.send("hello\r")
        try await viewer.waitFor("mini received hello")
        viewer.send("quit\r")
        _ = try await viewer.finish()

        // Mini goes away: the open watch and a new read both name it.
        await world.miniServer.stop()
        try world.disconnect(true)
        let reopened = TaskFilesStore(issue: task, cwd: "", machine: world.miniID, query: query)
        await reopened.readDirectory("", more: false)
        let failure = try #require(reopened.tree.listings[""]?.error)
        #expect(failure.contains("mini"), "\(failure)")
        files.observeFiles()
        try await eventually("the watch reports mini's outage", showing: { "\(String(describing: files.error))" }) { files.error?.contains("mini") == true }

        // Mini returns: Retry reads again, and a reopened Files reads the same checkout.
        try world.disconnect(false)
        _ = try await world.miniServer.status()
        await files.retry()
        try await eventually("Files recovers") { files.error == nil }
        await reopened.retry()
        #expect(reopened.tree.listings[""]?.entries.map(\.path).contains("document.txt") == true)
        await files.loadFile()
        #expect(files.document("notes.txt").snapshot?.content == "saved from the laptop\n")
        await world.stop()
    }

    @Test("A Task moving to mini keeps its drafts by base revision and its old shell on the laptop")
    func aMoveFollowsTheTaskAndLeavesRunningShells() async throws {
        let world = try await TwoMachines()
        defer { world.remove() }
        try await world.serve()
        let query = world.laptop.query(world.laptopServer)
        let task = try await world.createTask("Moving work")
        let checkout = try await world.laptop.json(["task", "checkout", task, "--json"], cwd: world.repo,
                                                   as: Checkout.self).worktree

        let files = TaskFilesStore(issue: task, cwd: checkout, query: query)
        // Drafts stay drafts: nothing saves them to the laptop before the move.
        files.autosave = false
        for path in ["document.txt", "notes.txt"] {
            files.selectFile(path, mode: .edit)
            await files.loadFile()
            files.document(path).editor.string = "draft of \(path)\n"
        }
        #expect(files.documents.values.filter(\.dirty).count == 2)

        // A shell started on the laptop before the move keeps running there.
        let marker = world.root.appendingPathComponent("old-shell.pid")
        let old = try world.laptop.startTerminal(
            world.shell(task, id: "old", run: "echo $$ > \(shellEscape(marker.path)); exec sleep 300"))
        try await eventually("the old shell started") { FileManager.default.fileExists(atPath: marker.path) }
        await old.detach()
        let oldShell = try #require(Int32(String(contentsOf: marker, encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines)))

        _ = try await world.laptop.run(["task", "move", task, "--machine", "mini", "--json"], cwd: world.repo)
        let moved = try await world.miniCheckout(task)
        try "changed on mini\n".write(to: moved.appendingPathComponent("notes.txt"), atomically: true, encoding: .utf8)
        // The pane's next reading names mini; the Task's Files follow it.
        files.relocate(machine: world.miniID, label: "mini", cwd: moved.path)
        #expect(files.machineName == "mini")
        #expect(kill(oldShell, 0) == 0, "moving the Task leaves the laptop's shell running")

        // Files re-reads its documents on mini. A draft whose file there is at
        // another revision stays whole beside it; it is never merged into it.
        let notes = files.document("notes.txt")
        try await eventually("notes.txt is read on mini", showing: {
            "error \(String(describing: files.error)) message \(String(describing: notes.saveMessage)) snapshot \(String(describing: notes.snapshot?.content))"
        }) { notes.external != nil }
        #expect(notes.external?.content == "changed on mini\n")
        #expect(!notes.canSave)

        // The same base revision on mini: the draft saves there.
        let document = files.document("document.txt")
        await document.save(issue: task, query: query)
        #expect(!document.saveFailed, "\(document.saveMessage ?? "")")
        #expect(try String(contentsOf: moved.appendingPathComponent("document.txt"), encoding: .utf8)
                == "draft of document.txt\n")
        // A different revision there: nothing is written and the draft stays.
        await notes.save(issue: task, query: query)
        #expect(notes.dirty)
        #expect(notes.text == "draft of notes.txt\n")
        #expect(try String(contentsOf: moved.appendingPathComponent("notes.txt"), encoding: .utf8) == "changed on mini\n")

        // Close ends the old shell on the Machine that started it.
        _ = try await world.laptop.run(["desktop", "shell", task, "--id", "old", "--close", "--started-on", world.laptopID])
        try await eventually("the old shell ended on the laptop") { kill(oldShell, 0) != 0 }
        await world.stop()
    }
}

private struct Checkout: Decodable { let worktree: String }

/// The laptop and mini, each with its own home, store and server.
@MainActor
private final class TwoMachines {
    let root: URL
    let laptop: BuiltLf
    let mini: BuiltLf
    let laptopServer: LocalServerFixture
    let miniServer: LocalServerFixture
    /// The laptop's clone of the shared origin.
    let repo: URL
    private(set) var laptopID = ""
    private(set) var miniID = ""

    init() async throws {
        // Short: server sockets live under these homes' hashes, and checkouts in /tmp.
        root = URL(fileURLWithPath: "/tmp").appendingPathComponent("lf-two-\(UUID().uuidString.prefix(8))")
        let fm = FileManager.default
        let local = root.appendingPathComponent("local"), remote = root.appendingPathComponent("remote")
        for dir in [root.appendingPathComponent("bin"), local, remote.appendingPathComponent("bin"),
                    remote.appendingPathComponent(".local/bin"), remote.appendingPathComponent("store")] {
            try fm.createDirectory(at: dir, withIntermediateDirectories: true)
        }
        let binary = try BuiltLf.binary("lf")
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
        // Mini's provider: it names where it runs and answers each line typed into it.
        try Self.executable(remote.appendingPathComponent("bin/claude"), """
            #!/bin/bash
            if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
            printf 'provider on mini in %s\\n' "$PWD"
            while IFS= read -r -t 60 line < /dev/tty; do
              printf 'mini received %s\\n' "$line"
              [ "$line" = quit ] && exit 0
            done
            """)
        try Data(#"{"projects":{"/":{"hasTrustDialogAccepted":true}}}"#.utf8)
            .write(to: remote.appendingPathComponent(".claude.json"))
        laptop = try BuiltLf(home: local, overriding: ["PATH": root.appendingPathComponent("bin").path + ":/usr/bin:/bin"])
        mini = try BuiltLf(home: remote, overriding: ["LF_HOME": remote.appendingPathComponent("store").path,
                                                      "PATH": remotePath])
        laptopServer = LocalServerFixture(laptop)
        miniServer = LocalServerFixture(mini)

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

    /// Register mini, then start both servers; their first opens migrate each store.
    func serve() async throws {
        struct Identity: Decodable { let id: String }
        miniID = try await laptop.json(["machine", "add", "mini", "--json"], as: Identity.self).id
        laptopID = try await laptop.json(["machine", "id", "--json"], as: Identity.self).id
        _ = try await miniServer.status()
        _ = try await laptopServer.status()
    }

    func createTask(_ title: String) async throws -> String {
        struct Created: Decodable {
            struct Task: Decodable { let id: String }
            let task: Task
        }
        return try await laptop.json(["task", "create", "--title", title, "--json"], cwd: repo, as: Created.self).task.id
    }

    /// Mini makes the Task's checkout, cloning the shared origin.
    func miniCheckout(_ task: String) async throws -> URL {
        URL(fileURLWithPath: try await laptop.json(["--machine", "mini", "task", "checkout", task, "--json"],
                                                   cwd: repo, as: Checkout.self).worktree)
    }

    /// What a Desktop shell tab runs, with `command` in place of a login shell.
    func shell(_ task: String, id: String, run command: String) -> [String] {
        [laptop.binary, "desktop", "shell", task, "--id", id, "--create", "--", "/bin/sh", "-c", command]
    }

    func disconnect(_ offline: Bool) throws {
        let marker = root.appendingPathComponent("bin/ssh.offline")
        if offline { try Data().write(to: marker) } else { try FileManager.default.removeItem(at: marker) }
    }

    func stop() async {
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

/// A command in a terminal of its own, as a Ghostty pane would run it.
@MainActor
private final class TerminalRun {
    private let process: Process
    private let input = Pipe()
    private let output: URL
    private let exited: AsyncStream<Int32>

    init(_ process: Process, output: URL) throws {
        self.process = process
        self.output = output
        try Data().write(to: output)
        process.standardInput = input
        process.standardOutput = try FileHandle(forWritingTo: output)
        process.standardError = process.standardOutput
        let (exited, ended) = AsyncStream<Int32>.makeStream()
        self.exited = exited
        process.terminationHandler = { ended.yield($0.terminationStatus); ended.finish() }
        try process.run()
    }

    var text: String { (try? String(contentsOf: output, encoding: .utf8)) ?? "" }

    func waitFor(_ expected: String, at location: SourceLocation = #_sourceLocation) async throws {
        try await eventually("terminal shows \(expected)", at: location) { self.text.contains(expected) }
    }

    func send(_ text: String) { input.fileHandleForWriting.write(Data(text.utf8)) }

    /// Close the terminal's input and wait for the command to end.
    func finish() async throws -> (status: Int32, output: String) {
        try? input.fileHandleForWriting.close()
        var status: Int32 = -1
        for await code in exited { status = code }
        return (status, text)
    }

    /// Close this viewer only, as closing its window would; what it attached to keeps running.
    func detach() async {
        process.terminate()
        for await _ in exited {}
    }
}

extension BuiltLf {
    func json<T: Decodable>(_ args: [String], cwd: URL? = nil, as type: T.Type) async throws -> T {
        try JSONDecoder().decode(type, from: Data(try await run(args, cwd: cwd).utf8))
    }

    /// `argv` under `script(1)`, which gives it a terminal.
    @MainActor
    fileprivate func startTerminal(_ argv: [String]) throws -> TerminalRun {
        let output = FileManager.default.temporaryDirectory.appendingPathComponent("terminal-\(UUID().uuidString)")
        return try TerminalRun(process(["-q", "/dev/null"] + argv, executable: "/usr/bin/script"), output: output)
    }

    /// Run `argv` in a terminal to its end.
    @MainActor
    fileprivate func terminal(_ argv: [String]) async throws -> (status: Int32, output: String) {
        try await startTerminal(argv).finish()
    }
}

@MainActor
private func eventually(_ what: String, timeout: Duration = .seconds(20),
                        at location: SourceLocation = #_sourceLocation,
                        showing state: () -> String = { "" },
                        _ condition: () -> Bool) async throws {
    let deadline = ContinuousClock.now + timeout
    while !condition() {
        try #require(ContinuousClock.now < deadline, "Timed out: \(what) \(state())", sourceLocation: location)
        try await Task.sleep(for: .milliseconds(50))
    }
}
#endif
