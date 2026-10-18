#if os(macOS)
import Darwin
import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

/// Real CLI writes and the production stream/projection; no provider or display.
@Suite("CLI to retained Work sidebar", .serialized)
@MainActor
struct WorkCLISubscriptionTests {
    @Test(arguments: ["complete", "abandon"])
    func taskLifecycleUpdatesOpenSidebar(action: String) async throws {
        let fixture = try WorkCLIFixture()
        defer { try? FileManager.default.removeItem(at: fixture.root) }
        try await fixture.prepare()
        let seed = try await fixture.json(["task", "create", "--wave", "proof", "--title", "Backlog", "--json"])
        let seedID = try #require(seed["id"] as? String)
        let status = try await fixture.json(["task", "status", seedID, "--json"])
        let waveName = try #require((status["execution"] as? [String: Any])?["wave"] as? String)
        _ = try await fixture.run(["wave", "workflow", "set", "proof", waveName, "--file",
                                   fixture.root.appendingPathComponent("proof.yaml").path])
        let query = RegistryQuery(watchWork: {
            try LocalWorkObservation.start(
                process: fixture.process(["monitor", "work", "--watch", "--json"]),
                configurationChanged: { false })
        }) { args, _ in try await fixture.run(args) }
        let model = WorkModel(query: query, repoPath: fixture.repo.path)
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await eventually { model.roadmap.value?.waves.first?.tasks.items.count == 1 }
        let wave = try #require(model.visibleRoadmaps.first)
        let machine = try await fixture.json(["machine", "id", "--json"])
        let machineID = try #require(machine["id"] as? String)
        let registry = SessionsWorkspaceRegistry(localMachineId: machineID)
        model.confirmMachine(machineID)
        model.bindWorkspaces(registry, repository: "repo")
        model.select(.wave(id: wave.wave.id))
        let original = try #require(registry.selectedWork.flatMap(registry.retained))
        original.multiplexer.newShell(command: ["/bin/cat"], location: original.location)
        let originalLayout = original.multiplexer.layout
        let originalFiles = original.files(issue: seedID, cwd: fixture.repo.path, query: query)
        let originalDraft = originalFiles.document("notes.txt")
        originalDraft.editor.string = "retain original draft"

        // The reader and selected workspace are already open before either CLI mutation.
        let created = try await fixture.json(["task", "create", "--wave", waveName, "--title", "Arrives through CLI", "--json"])
        let id = try #require(created["id"] as? String)
        try await eventually { model.task(id: id) != nil }
        #expect(selectedPlanMembers(model).allSatisfy { $0.work.kind != .task }) // Unstarted backlog stays hidden.
        _ = try await fixture.run(["-b", "task", "run", id])
        try await eventually { selectedPlanMembers(model).filter { $0.work.id == id }.count == 1 }
        #expect(model.selection == .wave(id: wave.wave.id))
        #expect(registry.retained(original.key) === original)
        #expect(original.multiplexer.layout == originalLayout)
        #expect(originalFiles.document("notes.txt") === originalDraft)
        #expect(originalDraft.editor.string == "retain original draft")

        model.select(.task(id: id))
        let workspace = try #require(registry.selectedWork.flatMap(registry.retained))
        let task = try #require(model.task(id: id)?.task)
        let checkout = try #require(task.reference.workspace?.identity)
        // Only the retained native-view identity is synthetic. No provider is launched.
        var session = try renameFixtureRecord("retained-cli-task", title: "Retained conversation", work: .task(id: id))
        session.workspace = try JSONDecoder().decode(SessionWorkspace.self, from: JSONSerialization.data(withJSONObject: [
            "machine_id": machineID, "worktree": checkout.worktree, "task_id": id
        ]))
        registry.revealSession(session, in: workspace)
        workspace.presentation.selectedSessionId = session.id
        #if canImport(GhosttyKit)
        let surfaceID = TerminalIdentity.session(session.id, machineId: machineID)
        let surface = registry.surfaces.view(for: surfaceID)
        #endif
        workspace.multiplexer.show(.files(taskId: id))
        let files = workspace.files(issue: task.task.identifier, cwd: checkout.worktree, query: query)
        let draft = files.document("notes.txt")
        draft.editor.string = "unsaved Task draft"
        let layout = workspace.multiplexer.layout
        let focus = workspace.multiplexer.focusedPaneId

        _ = try await fixture.run(["task", action, id])
        let state = action == "complete" ? "completed" : "canceled"
        try await eventually { model.task(id: id)?.task.task.state == state }
        #expect(model.selection == .task(id: id))
        #expect(selectedPlanMembers(model).isEmpty) // Built-in plans belong to repositories and Waves, not Tasks.
        let terminalTask = try #require(model.task(id: id)?.task)
        let header = TaskWorkflowHeader(model: model, task: terminalTask, work: model.taskWork[id])
        let label = try header.inspect().find(viewWithAccessibilityIdentifier: "task-lifecycle-status").text().string()
        #expect(label == (action == "complete" ? "Completed" : "Canceled"))
        #expect(FileManager.default.fileExists(atPath: checkout.worktree))
        #expect(workspace.multiplexer.layout == layout)
        #expect(workspace.multiplexer.focusedPaneId == focus)
        #expect(workspace.presentation.selectedSessionId == session.id)
        #if canImport(GhosttyKit)
        #expect(registry.surfaces.view(for: surfaceID) === surface)
        #endif
        #expect(files.document("notes.txt") === draft)
        #expect(draft.editor.string == "unsaved Task draft")

        model.select(.wave(id: wave.wave.id))
        #expect(!selectedPlanMembers(model).contains { $0.work.id == id })
        // A later subscription reply updates history without restoring the old selection.
        _ = try await fixture.run(["task", "edit", id, "--title", "Updated after navigation"])
        try await eventually { model.task(id: id)?.task.task.name == "Updated after navigation" }
        #expect(model.selection == .wave(id: wave.wave.id))
        #expect(!selectedPlanMembers(model).contains { $0.work.id == id })
        #expect(registry.retained(workspace.key) === workspace)
        model.goBack()
        #expect(model.selection == .task(id: id))
        #expect(selectedPlanMembers(model).isEmpty)
        let returned = TaskWorkflowHeader(model: model, task: try #require(model.task(id: id)?.task),
                                          work: model.taskWork[id])
        #expect(try returned.inspect().find(viewWithAccessibilityIdentifier: "task-lifecycle-status")
            .text().string() == label)
        #expect(registry.selectedWork == workspace.key)
        #expect(workspace.multiplexer.layout == layout)
        #expect(workspace.multiplexer.focusedPaneId == focus)
        #if canImport(GhosttyKit)
        #expect(registry.surfaces.view(for: surfaceID) === surface)
        #endif
        #expect(files.document("notes.txt") === draft)
        #expect(draft.editor.string == "unsaved Task draft")
        keeping.cancel()
        await keeping.value
    }

    @Test("Standalone membership moves reach the retained workspace through the CLI stream")
    func standaloneMovesKeepWorkspace() async throws {
        let fixture = try WorkCLIFixture()
        defer { try? FileManager.default.removeItem(at: fixture.root) }
        try await fixture.prepare()
        let created = try await fixture.json(["task", "create", "--no-wave", "--title", "Before filing", "--json"])
        let id = try #require(created["id"] as? String)
        _ = try await fixture.run(["checkout", id, "--json"])
        let query = RegistryQuery(watchWork: {
            try LocalWorkObservation.start(process: fixture.process(["monitor", "work", "--watch", "--json"]), configurationChanged: { false })
        }) { args, _ in try await fixture.run(args) }
        let model = WorkModel(query: query, repoPath: fixture.repo.path)
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        try await eventually { model.visibleWork.standaloneTasks.count == 1 }
        let machine = try await fixture.json(["machine", "id", "--json"])
        let machineId = try #require(machine["id"] as? String)
        let registry = SessionsWorkspaceRegistry(localMachineId: machineId)
        model.confirmMachine(machineId)
        let repository = try await query.repositoryIdentity(cwd: fixture.repo.path)
        model.bindWorkspaces(registry, repository: repository.id)
        model.select(.task(id: id))
        let key = try #require(registry.selectedWork)
        let workspace = try #require(registry.retained(key))
        let location = try #require(workspace.location)
        let draft = workspace.files(issue: id, cwd: location.worktree).document("notes.txt")
        draft.editor.string = "Unsent direction"
        let layout = workspace.multiplexer.layout
        _ = try await fixture.run(["wave", "ensure", "product", "--json"])
        let before = try #require(model.task(id: id)?.task)
        await model.moveTask(before, toWave: "product")
        #expect(model.navigation.taskRunDrafts[id]?.error == nil,
                "\(model.peerPlanning.errorMessage ?? "") · \(model.workActivity.errorMessage ?? "")")
        try await eventually { model.task(id: id)?.task.task.waveId != nil }
        #expect(model.visibleWork.standaloneTasks.isEmpty)
        await model.moveTask(try #require(model.task(id: id)?.task), toWave: nil)
        #expect(model.navigation.taskRunDrafts[id]?.error == nil,
                "\(model.peerPlanning.errorMessage ?? "") · \(model.workActivity.errorMessage ?? "")")
        try await eventually { model.visibleWork.standaloneTasks.count == 1 }
        #expect(model.selection == .task(id: id))
        #expect(registry.selectedWork == key)
        #expect(registry.retained(key) === workspace)
        #expect(workspace.multiplexer.layout == layout)
        #expect(draft.editor.string == "Unsent direction")
        keeping.cancel()
        await keeping.value
    }

    private func selectedPlanMembers(_ model: WorkModel) -> [ViewMember] {
        model.selectionPlans.value?.first { $0.owner.reference == model.selection }?.members ?? []
    }

    private func eventually(_ condition: @MainActor () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(20)
        while !condition(), ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(20)) }
        try #require(condition(), "CLI mutation did not reach the open Work model")
    }
}

private struct WorkCLIFixture: Sendable {
    let root: URL
    let repo: URL
    let binary: URL

    init() throws {
        root = FileManager.default.temporaryDirectory.appendingPathComponent("work-cli-\(UUID().uuidString)")
            .resolvingSymlinksInPath()
        repo = root.appendingPathComponent("repo")
        binary = ProcessInfo.processInfo.environment["LOOPFLOW_TEST_LF"].map(URL.init(fileURLWithPath:))
            ?? URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
                .deletingLastPathComponent().appendingPathComponent("target/debug/lf")
        try #require(FileManager.default.isExecutableFile(atPath: binary.path), "Build the source CLI with cargo build -p loopflow --bin lf")
        try FileManager.default.createDirectory(at: repo, withIntermediateDirectories: true)
    }

    func prepare() async throws {
        let flows = repo.appendingPathComponent(".lf/flows")
        try FileManager.default.createDirectory(at: flows, withIntermediateDirectories: true)
        try Data("- cmd: flow show proof\n".utf8).write(to: flows.appendingPathComponent("proof.yaml"))
        try Data("nodes:\n  review: demo\nedges:\n  - {from: start, to: review, flow: proof}\n".utf8)
            .write(to: root.appendingPathComponent("proof.yaml"))
        for args in [["init", "-b", "main"], ["config", "user.name", "Fixture Person"],
                     ["config", "user.email", "fixture@example.invalid"], ["add", "."], ["commit", "-m", "Fixture"]] {
            _ = try await run(args, executable: "/usr/bin/git")
        }
        let remote = root.appendingPathComponent("planning.git").path
        _ = try await run(["init", "--bare", remote], executable: "/usr/bin/git")
        _ = try await run(["remote", "add", "origin", remote], executable: "/usr/bin/git")
    }

    func process(_ args: [String], executable: String? = nil) -> Process {
        let process = Process()
        process.executableURL = executable.map(URL.init(fileURLWithPath:)) ?? binary
        process.arguments = args
        process.currentDirectoryURL = repo
        // Nothing from the launching agent, installed Machine, provider or account is inherited.
        process.environment = ["PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": root.path,
                               "LF_HOME": root.path, "LF_BIN": binary.path, "LF_USER_NAME": "Fixture Person",
                               "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
                               "GIT_ALLOW_PROTOCOL": "file", "GIT_TERMINAL_PROMPT": "0"]
        process.standardInput = FileHandle.nullDevice
        return process
    }

    @MainActor
    func json(_ args: [String]) async throws -> [String: Any] {
        try #require(JSONSerialization.jsonObject(with: Data(try await run(args).utf8)) as? [String: Any])
    }

    func run(_ args: [String], executable: String? = nil) async throws -> String {
        try await Task.detached {
            let output = root.appendingPathComponent("output-\(UUID().uuidString)")
            let errors = output.appendingPathExtension("stderr")
            try Data().write(to: output)
            try Data().write(to: errors)
            let handle = try FileHandle(forWritingTo: output)
            let diagnostics = try FileHandle(forWritingTo: errors)
            defer {
                try? handle.close(); try? diagnostics.close()
                try? FileManager.default.removeItem(at: output)
                try? FileManager.default.removeItem(at: errors)
            }
            let child = process(args, executable: executable)
            child.standardOutput = handle
            child.standardError = diagnostics
            try child.run()
            let deadline = ContinuousClock.now + .seconds(30)
            while child.isRunning && ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
            if child.isRunning {
                child.terminate()
                try await Task.sleep(for: .milliseconds(100))
                if child.isRunning { kill(child.processIdentifier, SIGKILL) }
                child.waitUntilExit()
                throw RegistryQueryError("CLI fixture timed out: \(args)")
            }
            guard child.terminationStatus == 0 else {
                throw RegistryQueryError("CLI fixture \(args): \(try String(contentsOf: errors, encoding: .utf8))")
            }
            return try String(contentsOf: output, encoding: .utf8)
        }.value
    }
}
#endif
