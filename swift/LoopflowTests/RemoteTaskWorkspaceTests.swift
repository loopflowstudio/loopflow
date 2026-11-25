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
        try await withTwoMachines { world in
            let query = world.laptop.query(world.laptopServer)
            let task = try await world.createTask("Remote work")
            _ = try await world.laptop.run(["task", "move", task, "--machine", "mini", "--json"], cwd: world.repo)

            // Not started: mini owns the Task but has no checkout for it yet.
            let files = TaskFilesStore(issue: task, cwd: "", machine: world.miniID, localMachineID: world.laptopID, query: query)
            files.autosave = false
            await files.workingTree.refresh()
            #expect(files.notStarted)
            await files.tree.readDirectory("", more: false)
            #expect(files.notStarted)
            // The watch stays open on the unstarted Task and names mini as the laptop does.
            files.observeFiles()
            try await eventually("the watch names mini before a checkout") { files.machineName == "mini" }
            #expect(files.notStarted)
            // A shell opens from the plan's owner before any conversation says where the Task runs.
            let workspace = SessionsWorkspace(key: WorkWorkspaceKey(repository: world.repo.path, kind: .task, id: task),
                title: "Remote work", location: nil, localMachineId: world.laptopID)
            workspace.follow(owner: world.miniID)
            _ = workspace.sessionStore(repoPath: world.repo.path, query: query)
            let shell = try #require(try workspace.newShell(command: [], focus: false)?.content.selectedShell)
            #expect(shell.location?.machineId == StructuredIdentity.prefixed(world.miniID, "home_"))
            // The attachment ends at the server's refusal; Desktop asks why in typed form.
            NotificationCenter.default.post(name: .ghosttySurfaceClosed, object: shell.terminal)
            try await eventually("the ended shell says Not started") { workspace.notStartedShells.contains(shell.id) }
            let notStarted = try await world.laptop.terminal(world.shell(task, id: "unstarted", run: "pwd"))
            #expect(notStarted.status != 0)

            let checkout = try await world.miniCheckout(task)
            try "changed on mini\n".write(to: checkout.appendingPathComponent("notes.txt"), atomically: true, encoding: .utf8)
            // The same watch goes live once mini records the checkout; nothing asks again.
            try await eventually("Files fills in from mini", showing: { "\(files.tree.listings.keys) \(String(describing: files.error))" }) {
                !files.notStarted && files.tree.listings[""]?.entries.map(\.path).contains("document.txt") == true
            }
            #expect(files.machineName == "mini")

            // An unchanged file reads from mini, even though the laptop never had a checkout.
            let entries = files.tree.listings[""]?.entries.map(\.path) ?? []
            #expect(entries.contains("document.txt"), "\(entries)")
            files.selectFile("document.txt", mode: .read)
            await files.loadFile()
            #expect(files.document("document.txt").snapshot?.content == "unchanged on mini\n")

            // Changes shows mini's uncommitted hunk; Save writes it there.
            await files.workingTree.refresh()
            #expect(files.workingTree.error == nil)
            #expect(files.workingChanges?.files.map(\.path).contains("notes.txt") == true)
            files.selectFile("notes.txt", mode: .edit)
            await files.loadFile()
            let notes = files.document("notes.txt")
            #expect(notes.snapshot?.content == "changed on mini\n")
            notes.editor.string = "saved from the laptop\n"
            await notes.save(issue: task, query: query)
            #expect(!notes.saveFailed, "\(notes.saveMessage ?? "")")
            #expect(try String(contentsOf: checkout.appendingPathComponent("notes.txt"), encoding: .utf8) == "saved from the laptop\n")

            // An ignored scratch write on mini arrives through task.files.watch with badge + Updated notice.
            let scratchDir = checkout.appendingPathComponent("scratch")
            try FileManager.default.createDirectory(at: scratchDir, withIntermediateDirectories: true)
            try "<h1>Peer report</h1>\n".write(to: scratchDir.appendingPathComponent("report.html"), atomically: true, encoding: .utf8)
            try await eventually("peer scratch write reaches Open & Scratch", showing: {
                "updated=\(files.browser.updated) scratch=\(files.scratchPaths) notice=\(String(describing: files.browser.notice))"
            }) {
                files.browser.updated.contains("scratch/report.html")
            }
            #expect(files.browser.notice == .updated("scratch/report.html"))
            #expect(files.scratchPaths.contains("scratch/report.html"))

            // Shell runs on mini, in mini's checkout.
            let ran = try await world.laptop.terminal(world.shell(task, id: "shell-1", run: "hostname; pwd"))
            #expect(ran.output.contains(checkout.path), "\(ran.output)")

            // Session: the argv Desktop's pane runs attaches to mini's provider and takes input.
            let session = try await world.mini.run(["session", "resume", "--prepare", "--json", "-a", "claude"], cwd: checkout)
            let id = try JSONDecoder().decode(SessionRecord.self, from: Data(session.utf8)).id
            let record = try await query.prepareSession(id: id, work: WorkReference(kind: .task, id: task))
            // Every command the record names runs on the laptop, never mini's `lf resume`.
            let printed = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
            #expect(!printed.contains("lf resume"), "\(printed)")
            #expect(record.openArgv.dropFirst().first == "open", "\(record.openArgv)")
            // Mini's server may start the prepared conversation before the viewer
            // attaches, so the provider's reply, not its first line, says where it runs.
            let viewer = try world.laptop.startTerminal(record.openArgv)
            viewer.send("hello\r")
            try await viewer.waitFor("mini received hello in \(checkout.path)")
            viewer.send("quit\r")
            _ = try await viewer.finish()

            // Mini goes away: the open watch and a new read both name it.
            // Keep ssh offline whenever mini's server is stopped so a watch retry
            // cannot run `lf server connect` into `ensure()` / `launchctl`.
            try world.disconnect(true)
            await world.miniServer.stop()
            let reopened = TaskFilesStore(issue: task, cwd: "", machine: world.miniID, query: query)
            await reopened.tree.readDirectory("", more: false)
            let failure = try #require(reopened.tree.listings[""]?.error)
            #expect(failure.contains("mini"), "\(failure)")
            files.observeFiles()
            try await eventually("the watch reports mini's outage", showing: { "\(String(describing: files.error))" }) { files.error?.contains("mini") == true }

            // Mini returns: Retry reads again, and a reopened Files reads the same checkout.
            _ = try await world.miniServer.status()
            try world.disconnect(false)
            await files.retry()
            try await eventually("Files recovers", showing: { "\(String(describing: files.error))" }) { files.error == nil }
            await reopened.retry()
            #expect(reopened.tree.listings[""]?.entries.map(\.path).contains("document.txt") == true)
            await files.loadFile()
            #expect(files.document("notes.txt").snapshot?.content == "saved from the laptop\n")

            // The laptop's own server restarts: the watch subscribes again and follows mini's next change.
            await world.laptopServer.stop()
            try await eventually("the watch reports the stopped server") { files.error != nil }
            _ = try await world.laptopServer.status()
            try "after restart\n".write(to: checkout.appendingPathComponent("restart.txt"), atomically: true, encoding: .utf8)
            try await eventually("Files recovers after the server restarts", showing: { "\(String(describing: files.error))" }) {
                files.error == nil && files.tree.listings[""]?.entries.map(\.path).contains("restart.txt") == true
            }
        }
    }

    @Test("A Task moving to mini keeps its drafts by base revision and its old shell on the laptop")
    func aMoveFollowsTheTaskAndLeavesRunningShells() async throws {
        try await withTwoMachines { world in
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

            // Desktop holds the Task open on the laptop, Files placed in its checkout.
            let registry = SessionsWorkspaceRegistry(localMachineId: world.laptopID)
            let store = registry.sessionStore(repository: world.repo.path, repoPath: world.repo.path, query: query)
            let onLaptop = WorkspaceIdentity(machineId: world.laptopID, worktree: checkout)
            let workspace = registry.workspace(for: WorkWorkspaceKey(repository: world.repo.path, kind: .task, id: task),
                title: "Moving work", location: onLaptop)
            #expect(!workspace.follow(owner: world.laptopID))
            workspace.multiplexer.showBrowser(.files, location: onLaptop, focus: false)

            // A shell started on the laptop before the move keeps running there.
            let marker = world.root.appendingPathComponent("old-shell.pid")
            world.laptopShells.append((task, "old"))
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

            // The Session pane showed the conversation on the laptop; it keeps its ID on mini.
            let resumed = try await world.mini.run(["session", "resume", "--prepare", "--json", "-a", "claude"], cwd: moved)
            let session = try JSONDecoder().decode(SessionRecord.self, from: Data(resumed.utf8)).id
            workspace.multiplexer.reveal(sessionId: session, location: onLaptop)
            // Desktop's plan names mini: the panes let go of the laptop checkout.
            #expect(workspace.follow(owner: world.miniID))
            #expect(workspace.location == nil)
            #expect(workspace.multiplexer.sessionPane?.content.location == nil)
            #expect(workspace.multiplexer.browserPane?.content.location == nil)
            // Preparing it through the laptop's server places the same pane on mini.
            let record = try await query.prepareSession(id: session, work: WorkReference(kind: .task, id: task))
            store.reconcile([record])
            store.adoptPreparation(record)
            let place = try #require(workspace.multiplexer.sessionPane?.content.location)
            #expect(place.machineId == StructuredIdentity.prefixed(world.miniID, "home_"))
            #expect(place.worktree == moved.path)
            #expect(record.action.map { !$0.help.contains("lf resume") } != false)

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
        }
    }
}

private struct Checkout: Decodable { let worktree: String }

/// Both Machines serving; afterwards, however `body` ends, its shells close and both servers stop.
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
        try "scratch/\n".write(to: repo.appendingPathComponent(".gitignore"), atomically: true, encoding: .utf8)
        try "unchanged on mini\n".write(to: repo.appendingPathComponent("document.txt"), atomically: true, encoding: .utf8)
        try "first notes\n".write(to: repo.appendingPathComponent("notes.txt"), atomically: true, encoding: .utf8)
        for args in [["add", "."], ["commit", "-qm", "Shared files"], ["push", "-q", "origin", "main"]] {
            _ = try await laptop.run(args, cwd: repo, executable: "/usr/bin/git")
        }
    }

    /// Start both servers, whose first opens migrate each store, then register
    /// mini: its reachability probe must not wait out a migration.
    func serve() async throws {
        struct Identity: Decodable { let id: String }
        _ = try await miniServer.status()
        _ = try await laptopServer.status()
        miniID = try await laptop.json(["machine", "add", "mini", "--json"], as: Identity.self).id
        laptopID = try await laptop.json(["machine", "id", "--json"], as: Identity.self).id
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

    /// Shells started on the laptop, closed when the test ends however it ends.
    var laptopShells: [(task: String, id: String)] = []

    func stop() async {
        for (task, id) in laptopShells {
            _ = try? await laptop.run(["desktop", "shell", task, "--id", id, "--close", "--started-on", laptopID])
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
        try await eventually("terminal shows \(expected)", at: location, showing: { self.text }) { self.text.contains(expected) }
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
