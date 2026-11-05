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
    @Test("Load initializes working Task and repository conversations without a Start click", arguments: [false, true])
    func workingTaskConversationOpensAutomatically(prepared: Bool) async throws {
        let fixture = try WorkCLIFixture()
        defer { try? FileManager.default.removeItem(at: fixture.root) }
        try await fixture.prepare()
        let created = try await fixture.json(["task", "create", "--no-wave", "--title", "Conversation recovery", "--json"])
        let id = try #require(created["id"] as? String)
        _ = try await fixture.run(["checkout", id, "--json"])
        let query = fixture.lf.query
        let initial = prepared ? try await query.prepareTaskSession(issue: id, cwd: fixture.repo.path) : nil
        let snapshot = try await query.taskDestination(issue: id, repo: fixture.repo.path)
        let task = try #require(snapshot.tasks.items.first)
        let location = try #require(task.reference.workspace?.identity)
        let repository = try await query.repositoryIdentity(cwd: fixture.repo.path)
        let registry = SessionsWorkspaceRegistry(localMachineId: location.machineId)
        defer { registry.prepareForQuit() }
        let model = WorkModel(query: query, repoPath: fixture.repo.path)
        model.bindWorkspaces(registry, repository: repository.id)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        await model.refreshSessions()
        // A second window may discover the same missing conversation before
        // the first window has finished preparing it.
        let otherRegistry = SessionsWorkspaceRegistry(localMachineId: location.machineId)
        defer { otherRegistry.prepareForQuit() }
        let otherModel = WorkModel(query: query, repoPath: fixture.repo.path)
        otherModel.bindWorkspaces(otherRegistry, repository: repository.id)
        otherModel.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        await otherModel.refreshSessions()
        let otherStore = otherRegistry.sessionStore(repository: repository.id, repoPath: fixture.repo.path, query: query)
        let workspace = try #require(registry.retained(model.workspaceKey(for: .task(id: id), repository: repository.id)))
        let store = registry.sessionStore(repository: repository.id, repoPath: fixture.repo.path, query: query)
        try await eventually { workspace.multiplexer.sessionPane?.sessionId != nil && store.sessions.first { $0.record.taskPrimary }?.state == .prepared }
        #expect(model.selection == nil)
        // The repository's own conversation opens on the same load, in its scope checkout.
        let repositoryWorkspace = try #require(registry.retained(model.workspaceKey(for: nil, repository: repository.id)))
        try await eventually { store.sessions.first { $0.record.primaryScope == "repository" }?.state == .prepared }
        #expect(repositoryWorkspace.conversationError == nil)
        #expect(repositoryWorkspace.multiplexer.sessionPane?.sessionId == store.sessions.first { $0.record.primaryScope == "repository" }?.id)
        // Selecting and refreshing reuse the Session admitted by fresh-load prefetch.
        model.select(.task(id: id))
        await model.refreshSessions()
        model.openWarmConversations()
        let session = try #require(store.sessions.first { $0.record.taskPrimary })
        #expect(session.state == .prepared)
        #expect(session.record.workspace?.identity == location)
        #expect(session.launchArguments.contains("--start-if-free"))
        #expect(workspace.conversationStart == nil)
        #expect(await query.startOffer(refused: .reveal, issue: id, cwd: fixture.repo.path) == nil)
        let recorded = try await query.sessionPage(cwd: fixture.repo.path).entries
        #expect(recorded.filter(\.taskPrimary).map(\.id) == [session.id])
        #expect(recorded.filter { $0.primaryScope == "repository" }.count == 1)
        #expect(recorded.count == 2)
        if let initial { #expect(initial.id == session.id) }
        #expect(recorded.allSatisfy { $0.agentProcessId == nil }) // Prepared, not launched: no Graph or operator ran.
        try await eventually { otherStore.sessions.first { $0.record.taskPrimary }?.state == .prepared }
        #expect(otherStore.sessions.first { $0.record.taskPrimary }?.id == session.id)
    }

    @Test(arguments: ["complete", "abandon"])
    func taskLifecycleUpdatesOpenSidebar(action: String) async throws {
        let fixture = try WorkCLIFixture()
        defer { try? FileManager.default.removeItem(at: fixture.root) }
        try await fixture.prepare()
        let seed = try await fixture.json(["task", "create", "--wave", "proof", "--title", "Backlog", "--json"])
        let seedID = try #require(seed["id"] as? String)
        let status = try await fixture.json(["desktop", "task", seedID, "--json"])
        let waveName = try #require((status["execution"] as? [String: Any])?["wave"] as? String)
        _ = try await fixture.run(["--wave", waveName, "set", "graph", "proof"])
        let query = fixture.lf.query
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
        let originalPanes = original.multiplexer.layout.allPanes.map(\.id)
        let originalFiles = original.files(issue: seedID, cwd: fixture.repo.path, query: query)
        let originalDraft = originalFiles.document("notes.txt")
        originalDraft.editor.string = "retain original draft"

        // The reader and selected workspace are already open before either CLI mutation.
        let created = try await fixture.json(["task", "create", "--wave", waveName, "--title", "Arrives through CLI", "--json"])
        let id = try #require(created["id"] as? String)
        try await eventually { model.task(id: id) != nil }
        // An unstarted Task waits in the board's Backlog, where it offers Start; the strip lists Working Tasks.
        try await eventually {
            WaveStandup(tasks: model.visibleTasks, closed: [], doneDays: 0, now: .now, github: [:], sessions: [:])
                .backlog.contains { $0.id == id }
        }
        #expect(!selectedPlanMembers(model).contains { $0.work.id == id })
        // `-b` would run `demo` headless; plain `task run` stops before it, needing no provider.
        _ = try await fixture.run(["task", "run", id])
        // Membership and checkout details arrive in separate observation frames.
        try await eventually {
            selectedPlanMembers(model).filter { $0.work.id == id }.count == 1
                && model.task(id: id)?.task.reference.workspace?.identity != nil
        }
        #expect(model.selection == .wave(id: wave.wave.id))
        #expect(registry.retained(original.key) === original)
        // The Wave's own conversation fills its session pane on load; the panes survive.
        #expect(original.multiplexer.layout.allPanes.map(\.id) == originalPanes)
        #expect(originalFiles.document("notes.txt") === originalDraft)
        #expect(originalDraft.editor.string == "retain original draft")

        model.select(.task(id: id))
        let workspace = try #require(registry.selectedWork.flatMap(registry.retained))
        let task = try #require(model.task(id: id)?.task)
        let checkout = try #require(task.reference.workspace?.identity)
        // Automatic opening prepares this Session; nothing mounts it, so no provider launches.
        try await eventually { workspace.multiplexer.sessionPane?.sessionId != nil }
        let sessionID = try #require(workspace.multiplexer.sessionPane?.sessionId)
        #if canImport(GhosttyKit)
        let surfaceID = TerminalIdentity.session(sessionID, machineId: machineID)
        let surface = registry.surfaces.view(for: surfaceID)
        #endif
        workspace.multiplexer.showBrowser(.files)
        let files = workspace.files(issue: task.task.identifier, cwd: checkout.worktree, query: query)
        let draft = files.document("notes.txt")
        draft.editor.string = "unsaved Task draft"
        let layout = workspace.multiplexer.layout
        let focus = workspace.multiplexer.focusedPaneId

        _ = try await fixture.run(["task", action, id])
        let state = action == "complete" ? "completed" : "canceled"
        // Planning and sidebar membership arrive as independent stream frames.
        try await eventually {
            guard let plan = model.selectionPlans.value?.first(where: {
                $0.dashboardID == nil && $0.selection.expression == .wave(wave.wave.id)
            }) else { return false }
            return model.task(id: id)?.task.task.state == state
                && !plan.contents.contains { $0.work.id == id }
        }
        #expect(model.selection == .task(id: id))
        #expect(selectedPlanMembers(model).isEmpty) // Built-in plans belong to repositories and Waves, not Tasks.
        let terminalTask = try #require(model.task(id: id)?.task)
        let header = TaskWorkflowHeader(model: model, task: terminalTask, work: model.taskWork[id])
        let label = try header.inspect().find(viewWithAccessibilityIdentifier: "task-lifecycle-status").text().string()
        #expect(label == (action == "complete" ? "Completed" : "Canceled"))
        #expect(FileManager.default.fileExists(atPath: checkout.worktree))
        #expect(workspace.multiplexer.layout == layout)
        #expect(workspace.multiplexer.focusedPaneId == focus)
        #expect(workspace.presentation.selectedSessionId == sessionID)
        #if canImport(GhosttyKit)
        #expect(registry.surfaces.view(for: surfaceID) === surface)
        #endif
        #expect(files.document("notes.txt") === draft)
        #expect(draft.editor.string == "unsaved Task draft")

        model.select(.wave(id: wave.wave.id))
        #expect(!selectedPlanMembers(model).contains { $0.work.id == id })
        // A closed Task left the plan; an edit made while it is not shown
        // reaches it when it is shown again, without restoring the old selection.
        _ = try await fixture.run(["task", "edit", id, "--title", "Updated after navigation"])
        #expect(model.selection == .wave(id: wave.wave.id))
        #expect(!selectedPlanMembers(model).contains { $0.work.id == id })
        #expect(registry.retained(workspace.key) === workspace)
        model.goBack()
        #expect(model.selection == .task(id: id))
        try await eventually { model.task(id: id)?.task.task.name == "Updated after navigation" }
        #expect(selectedPlanMembers(model).isEmpty)
        let returned = TaskWorkflowHeader(model: model, task: try #require(model.task(id: id)?.task),
                                          work: model.taskWork[id])
        #expect(try returned.inspect().find(viewWithAccessibilityIdentifier: "task-lifecycle-status")
            .text().string() == label)
        #expect(registry.selectedWork == workspace.key)
        #expect(workspace.multiplexer.layout == layout)
        #expect(workspace.multiplexer.focusedPaneId == focus)
        #if canImport(GhosttyKit)
        // Completed departure releases the native viewer; returning keeps panes and drafts.
        #expect(registry.surfaces.view(for: surfaceID) !== surface)
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
        let query = fixture.lf.query
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
        // Moving must preserve the conversation the working Task already opened.
        try await eventually { workspace.multiplexer.sessionPane?.sessionId != nil }
        let layout = workspace.multiplexer.layout
        _ = try await fixture.run(["wave", "add", "product", "--json"])
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

    @Test func canonical_ids_desktop_cli_transport_accepts_any_casing() async throws {
        let fixture = try WorkCLIFixture()
        defer { try? FileManager.default.removeItem(at: fixture.root) }
        try await fixture.prepare()
        let created = try await fixture.json(["task", "create", "--wave", "proof", "--title", "Mixed case", "--json"])
        let id = try #require(created["id"] as? String)
        let current = try await fixture.run(["task", "location", id, "--json"])
        let reading = try #require(try JSONDecoder().decode([TaskLocationObservation].self, from: Data(current.utf8)).first)
        let query = fixture.lf.query

        // Desktop's CLI transport sends every identity uppercased; Rust owns ingress.
        let observed = try await query.taskLocation(task: id.uppercased(),
                                                    repository: reading.repositoryID.uppercased(),
                                                    machine: reading.machineID.uppercased())
        #expect(observed.taskID == id)
        #expect(observed.repositoryID == reading.repositoryID)
        #expect(observed.machineID == reading.machineID)
        #expect(observed.request == observed.request.uppercased(), "opaque request token returns byte-exact")
        let status = try await fixture.json(["task", "status", id.uppercased(), "--json"])
        #expect((status["task"] as? [String: Any])?["id"] as? String == id)
    }

    private func selectedPlanMembers(_ model: WorkModel) -> [WorkViewPlan.Content] {
        model.selectionPlans.value?.first { plan in
            guard plan.dashboardID == nil else { return false }
            switch plan.selection.expression {
            case .repository: return model.selection == nil
            case .wave(let id): return model.selection == .wave(id: id)
            default: return false
            }
        }?.contents ?? []
    }

    private func eventually(_ line: Int = #line, _ condition: @MainActor () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(20)
        while !condition(), ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(20)) }
        try #require(condition(), "CLI mutation did not reach the open Work model (line \(line))")
    }
}

private struct WorkCLIFixture: Sendable {
    let root: URL
    let repo: URL
    let lf: BuiltLf

    init() throws {
        root = FileManager.default.temporaryDirectory.appendingPathComponent("work-cli-\(UUID().uuidString)")
            .resolvingSymlinksInPath()
        repo = root.appendingPathComponent("repo")
        lf = try BuiltLf(home: root, cwd: repo)
        try FileManager.default.createDirectory(at: repo, withIntermediateDirectories: true)
    }

    func prepare() async throws {
        let graphs = repo.appendingPathComponent(".lf/graphs")
        try FileManager.default.createDirectory(at: graphs, withIntermediateDirectories: true)
        try Data("- cmd: graph show proof\n- interactive: {name: demo, id: review}\n".utf8)
            .write(to: graphs.appendingPathComponent("proof.yaml"))
        for args in [["init", "-b", "main"], ["config", "user.name", "Fixture Person"],
                     ["config", "user.email", "fixture@example.invalid"], ["add", "."], ["commit", "-m", "Fixture"]] {
            _ = try await run(args, executable: "/usr/bin/git")
        }
        let remote = root.appendingPathComponent("planning.git").path
        _ = try await run(["init", "--bare", remote], executable: "/usr/bin/git")
        _ = try await run(["remote", "add", "origin", remote], executable: "/usr/bin/git")
    }

    @MainActor
    func json(_ args: [String]) async throws -> [String: Any] {
        try #require(JSONSerialization.jsonObject(with: Data(try await run(args).utf8)) as? [String: Any])
    }

    func run(_ args: [String], executable: String? = nil) async throws -> String {
        try await lf.run(args, executable: executable)
    }
}
#endif
