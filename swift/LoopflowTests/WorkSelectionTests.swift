#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Query-driven Work sidebar")
@MainActor
struct WorkSelectionTests {
    private let repo = "/fixture"
    private let repository = "repo"
    private var wave: WorkWorkspaceKey { .init(repository: repository, kind: .wave, id: "wave") }
    private var root: WorkWorkspaceKey { .init(repository: repository, kind: .repository, id: repository) }
    private func member(_ id: String) -> ViewMember {
        .init(work: .init(repository: repository, kind: .task, id: id), title: id)
    }

    @Test("Query arrivals, departures and failures never change retained working positions")
    func sidebarUpdatesPreserveWorkspace() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: "machine")
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Sidebar reading cannot execute") }
        let model = WorkModel(query: query, repoPath: repo)
        model.bindWorkspaces(registry, repository: repository)
        let a = member("a"), b = member("b")
        model.applySelectionPlans([ViewPlan(owner: root, members: [a, b])], reason: "")
        model.select(a.work.reference)
        model.select(b.work.reference)
        model.select(nil)
        let workspace = try #require(registry.retained(root))
        let layout = registry.layout
        let keys = Set(registry.keys)
        let sidebar = WorkNavigator(model: model, onOpenSession: { _ in })
        for members in [[b, a], [b], [], [member("arrival"), a]] {
            model.applySelectionPlans([ViewPlan(owner: root, members: members)], reason: "")
            #expect(model.workingTaskIDs == ["a", "b"])
        }
        model.applySelectionPlans(nil, reason: "Store unavailable")
        _ = try sidebar.inspect().find(text: "Store unavailable")
        _ = try sidebar.inspect().find(viewWithAccessibilityIdentifier: "work-task-a")
        _ = try sidebar.inspect().find(viewWithAccessibilityIdentifier: "work-task-b")
        #expect(model.selection == nil)
        #expect(registry.layout == layout)
        #expect(Set(registry.keys) == keys)
        #expect(workspace.multiplexer.focusedPane.content == .empty)
    }

    @Test("Sidebar navigation enters and returns without losing retained panes and drafts")
    func renderAndReturn() throws {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Passive rendering cannot execute") }
        let registry = SessionsWorkspaceRegistry(localMachineId: "machine")
        let model = WorkModel(query: query, repoPath: repo)
        let view = SessionsContentView(model: model, repoPath: repo, workspaces: registry,
            machineId: "machine", repository: repository, query: query)
        let a = member("a"), b = member("b")
        model.applySelectionPlans([ViewPlan(owner: wave, members: [a, b]), ViewPlan(owner: root, members: [a, b])], reason: "")
        let workspace = registry.workspace(for: a.work, title: "a", location: nil)
        let location = WorkspaceIdentity(machineId: "machine", worktree: "/fixture/a")
        workspace.multiplexer.load(sessionId: "session-a", location: location)
        let pane = workspace.multiplexer.focusedPane
        let original = try renameFixtureRecord("session-a", title: "A primary", work: .task(id: "a"))
        var wire = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(original)) as? [String: Any])
        wire["task_primary"] = true
        wire["state"] = "unknown"
        wire["workspace"] = ["machine_id": "machine", "worktree": location.worktree, "task_id": "a", "unavailable": NSNull()]
        let primary = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: wire))
        registry.sessionStore(repository: repository, repoPath: repo, query: query).reconcile([primary])
        let files = workspace.files(issue: "a", cwd: location.worktree, query: query)
        let draft = files.document("notes.md")
        draft.editor.string = "unsaved draft"
        model.select(a.work.reference)
        model.select(wave.reference)
        _ = try view.body.inspect().find(viewWithAccessibilityIdentifier: "work-workspace-wave")
        let sidebar = WorkNavigator(model: model, onOpenSession: { _ in })
        _ = try sidebar.inspect().find(viewWithAccessibilityIdentifier: "work-task-a")
        model.select(a.work.reference)
        #expect(registry.retained(a.work) === workspace)
        #expect(workspace.multiplexer.focusedPane == pane)
        model.goBack()
        #expect(model.selection == wave.reference)
        #expect(files.document("notes.md") === draft)
        #expect(draft.editor.string == "unsaved draft")
        #expect(workspace.multiplexer.focusedPane == pane)
        #expect(registry.surfaces.surfaceIncarnation(for: .session("session-a", machineId: "machine")) == nil)
    }

    @Test("Working slots stay stable through filters, Wave navigation and missing planning")
    func stableWorkingSlots() throws {
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Navigation cannot execute") }, repoPath: repo)
        let registry = SessionsWorkspaceRegistry(localMachineId: "machine")
        model.bindWorkspaces(registry, repository: repository)
        let a = member("a"), b = member("b")
        model.applySelectionPlans([ViewPlan(owner: root, members: [a, b])], reason: "")
        model.select(a.work.reference)
        model.select(b.work.reference)
        model.select(a.work.reference)
        let retained = try #require(registry.retained(a.work))
        let pane = retained.multiplexer.focusedPane
        model.navigation.sidebarFilter = .running
        #expect(model.sidebarTaskIDs.isEmpty)
        model.selectWorkingTask(slot: 1)
        #expect(model.selection == b.work.reference)
        #expect(model.navigation.sidebarFilter == .running)
        model.selectWorkingTask(slot: 8)
        #expect(model.selection == b.work.reference)
        model.cycleWorkingTask(1)
        #expect(model.selection == a.work.reference)
        model.cycleWorkingTask(-1)
        #expect(model.selection == b.work.reference)
        model.select(wave.reference)
        #expect(model.workingTaskIDs == ["a", "b"])
        model.navigation.sidebarFilter = .working
        let sidebar = WorkNavigator(model: model, onOpenSession: { _ in })
        _ = try sidebar.inspect().find(viewWithAccessibilityIdentifier: "work-task-a")
        _ = try sidebar.inspect().find(viewWithAccessibilityIdentifier: "work-task-b")
        #expect(retained.multiplexer.focusedPane == pane)
    }

    @Test("Explicit opening resolves a sidebar Task and explains unavailable planning")
    func openingWithoutPlanning() async throws {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Planning unavailable") }
        let model = WorkModel(query: query, repoPath: repo)
        let registry = SessionsWorkspaceRegistry(localMachineId: "machine")
        model.bindWorkspaces(registry, repository: repository)
        let a = member("a")
        model.applySelectionPlans([ViewPlan(owner: root, members: [a])], reason: "")
        model.select(a.work.reference)
        await model.openTaskConversation(a.work.id)
        #expect(model.taskOpening?.status == .failed)
        #expect(model.taskLinkReading.errorMessage == "Planning unavailable")
        #expect(model.showsTaskLink)
        #expect(model.selection == a.work.reference)
    }

    @Test("Work observation applies selection frames and failed readings preserve known membership")
    func streamedPlans() async throws {
        let (frames, continuation) = AsyncThrowingStream<WorkFrame, any Error>.makeStream()
        let observation = WorkObservation(frames: frames, request: { _ in }, cancel: { continuation.finish() })
        let query = RegistryQuery(watchWork: { observation }) { _, _ in throw RegistryQueryError("Unexpected execution") }
        let model = WorkModel(query: query, repoPath: repo)
        let registry = SessionsWorkspaceRegistry()
        model.bindWorkspaces(registry, repository: repository)
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel(); continuation.finish() }
        let plans = [ViewPlan(owner: root, members: [member("a")])]
        continuation.yield(WorkFrame(sequence: 1, answers: 100, home: "/fixture-home", revisions: nil,
            unavailable: nil, content: .selections(.init(repo: repo, plans: plans)), wire: nil))
        try await eventually { model.selectionPlans.value == plans }
        continuation.yield(WorkFrame(sequence: 2, answers: 100, home: "/fixture-home", revisions: nil,
            unavailable: "read failed", content: .selections(nil), wire: nil))
        try await eventually { model.selectionPlans.errorMessage == "read failed" }
        #expect(model.selectionPlans.value == plans)
        #expect(registry.openWork == [root])
        #expect(registry.retained(member("a").work) == nil)
    }

    @Test("A Task first opened by runtime identity keeps one slot when planning arrives")
    func latePlanningAliases() throws {
        let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(contentsOf: fixture))
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Navigation cannot execute") }, repoPath: "/src/loopflow")
        let registry = SessionsWorkspaceRegistry(localMachineId: "machine")
        model.bindWorkspaces(registry, repository: repository)
        let id = "ts_review00000000000000000000000000"
        model.applySelectionPlans([ViewPlan(owner: root, members: [member(id)])], reason: "")
        model.select(.task(id: id))
        #expect(model.workingTaskIDs == [id])
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        model.select(.task(id: "issue-review"))
        model.select(.task(id: id))
        #expect(model.workingTaskIDs == ["issue-review"])
    }

    @Test("Find Work spans repositories and routes another repo without changing this window")
    func crossRepositoryPalette() async throws {
        let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(contentsOf: fixture))
        let remoteWave = try #require(roadmap.waves.first)
        let remoteTask = try #require(remoteWave.tasks.items.first)
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Routing must not launch here") }, repoPath: repo)
        model.applyFixture(roadmap: .available(roadmap), waves: .available([]), workActivity: .loading, repos: [])
        var destinations: [(String, LinkedWork?)] = []
        model.openRepository = { destinations.append(($0, $1)) }
        #expect(model.searchDestinations(remoteTask.task.identifier).contains { $0.id == .task(remoteTask.id) })
        await model.openPaletteTask(remoteTask.id)
        model.openPaletteWave(remoteWave.wave.id)
        #expect(destinations.count == 2)
        #expect(destinations.allSatisfy { $0.0 == remoteWave.wave.repo })
        #expect(destinations.first?.1 == .task(TaskLink(issue: remoteTask.task.identifier, repo: remoteWave.wave.repo)))
        #expect(destinations.last?.1 == .wave(id: remoteWave.wave.id, address: nil))
        #expect(model.repoPath == repo)
        #expect(model.selection == nil)
    }

    @Test("A delivered Wave link selects that Wave in its repository window")
    func deliveredWaveLinkSelectsWave() async throws {
        let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(contentsOf: fixture))
        let wave = try #require(roadmap.waves.first)
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Selection must not read") }, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(roadmap), waves: .available([]), workActivity: .loading, repos: [])
        await model.receiveWorkLink(.wave(id: wave.wave.id, address: "loopflow://loopflow/wave/\(wave.wave.id)"))
        #expect(model.selection == .wave(id: wave.wave.id))
    }

    @Test("Navigator selection previews planned Tasks without preparing a conversation")
    func plannedTaskNavigationIsPassive() async throws {
        let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(contentsOf: fixture))
        let task = try #require(roadmap.tasks.items.first { $0.runtime?.started != true })
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Navigation cannot execute") },
                              repoPath: task.task.repo)
        model.applyFixture(roadmap: .available(roadmap), waves: .available([]), workActivity: .loading, repos: [])
        let registry = SessionsWorkspaceRegistry(localMachineId: "machine")
        model.bindWorkspaces(registry, repository: repository)
        await model.openPaletteTask(task.id)
        #expect(model.selection == .task(id: task.id))
        #expect(model.workingTaskIDs.isEmpty)
        #expect(model.taskOpening == nil)
        let workspace = try #require(registry.selectedWork.flatMap(registry.retained))
        #expect(workspace.multiplexer.focusedPane.content == .empty)
        #expect(workspace.presentation.content == .details)
    }

    private func eventually(_ condition: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(2)
        while !condition(), ContinuousClock.now < deadline { await Task.yield() }
        try #require(condition())
    }
}
#endif
