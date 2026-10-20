#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Dashboard workspace integration", .serialized)
@MainActor
struct DashboardWorkspaceTests {
    @Test func dashboardsReuseWorkAndKeepRepositoryNavigation() throws {
        let plan = try JSONDecoder().decode(WorkViewPlan.self, from: loadFixtureData("view_plan.json"))
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Offline") }, repoPath: "/repo")
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        model.bindWorkspaces(registry, repository: plan.repository)
        model.applySelectionPlans([plan], reason: "")
        let id = try #require(plan.dashboardID)
        model.selectDashboard(id)
        let member = try #require(plan.projection.visible?.firstWork)
        let workspace = try #require(registry.retained(member))
        let files = workspace.files(issue: member.id, cwd: "/task")
        files.document("draft.md").editor.string = "Retained draft"
        model.select(member.reference)
        #expect(registry.retained(member) === workspace)
        model.goBack()
        #expect(model.navigation.dashboardID == id)
        #expect(files.document("draft.md").editor.string == "Retained draft")
        model.select(nil)
        #expect(model.navigation.dashboardID == nil)
        #expect(registry.selectedWork?.kind == .repository)
        #expect(registry.retained(member) === workspace)
        #expect(model.paletteRows.contains { $0.id == .dashboard(id) })
    }

    @Test func refreshedPlansDoNotReplaceOrUnhideRetainedPrimaryPanes() throws {
        let fixture = String(decoding: try loadFixtureData("view_plan.json"), as: UTF8.self)
        let bytes = Data(fixture.replacingOccurrences(of: "\"primary_session\": null", with: "\"primary_session\": \"primary\"").utf8)
        let plan = try JSONDecoder().decode(WorkViewPlan.self, from: bytes)
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Offline") }, repoPath: "/repo")
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        model.bindWorkspaces(registry, repository: plan.repository)
        model.applySelectionPlans([plan], reason: "")
        model.selectDashboard(try #require(plan.dashboardID))
        let key = try #require(plan.contents.first?.work)
        let work = try #require(registry.retained(key))
        let pane = try #require(work.multiplexer.pane(forSessionId: "primary"))
        work.multiplexer.setCollapsed(paneId: pane.id, collapsed: true)
        model.applySelectionPlans([plan], reason: "")
        #expect(work.multiplexer.pane(forSessionId: "primary") == pane)
        #expect(work.multiplexer.collapsedPaneIds.contains(pane.id))
        #expect(work.multiplexer.layout.allPanes.count == 1)
    }

    @Test func dashboardFocusPreservesQueryGeometryAndRetainedWork() throws {
        let a = WorkWorkspaceKey(repository: "repo", kind: .task, id: "a")
        let b = WorkWorkspaceKey(repository: "repo", kind: .task, id: "b")
        let members = [a, b].map { WorkMemberRef(source: ["root"], work: $0) }
        let tree = WorkArrangementTree.split(axis: .vertical, ratio: 0.7,
            first: .member(members[0]), second: .member(members[1]))
        let plan = WorkViewPlan(name: "Review", pinned: false, dashboardID: "review", repository: "repo",
            observedAt: 0, selection: WorkSelection(id: "root", expression: .filter(.state(.open))), savedRevision: 1,
            resolution: WorkSelectionResolution(attention: [], members: members, unavailable: []),
            arrangement: WorkArrangement(tree: tree, hidden: [], mode: .touched),
            projection: WorkArrangementProjection(visible: tree, suppressed: []),
            contents: [a, b].map { fixtureViewContent(work: $0, title: $0.id) })
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("No execution allowed") }, repoPath: "/repo")
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        model.bindWorkspaces(registry, repository: "repo")
        model.applySelectionPlans([plan], reason: "")
        model.selectDashboard("review")
        let workspace = try #require(registry.retained(b))
        let pane = workspace.multiplexer.focusedPane
        let window = UUID()
        let target = DesktopPaneTarget(repository: "repo", window: window.uuidString, work: b,
            pane: pane.id, incarnation: pane.incarnation, tab: nil)
        _ = try registry.controlPane(.init(target: target, action: .focus), model: model)
        let state = model.inspectDesktop(repository: "repo", window: window, workspaces: registry)
        #expect(state.selectionKind == "dashboard")
        #expect(state.view == plan)
        #expect(registry.selectedWork == b)
        #expect(state.layout == .split(axis: .vertical, ratio: 0.7,
            first: .workspace(work: a), second: .workspace(work: b)))
        #expect(try JSONDecoder().decode(DesktopWindowInspection.self, from: JSONEncoder().encode(state)) == state)
        #expect(registry.inspect().allSatisfy { $0.layout.surface == nil })
        model.select(a.reference)
        #expect(registry.openWork == [a])
        model.goBack()
        #expect(registry.openWork == [a, b])
        #expect(registry.retained(b) === workspace)
        #expect(workspace.multiplexer.focusedPane == pane)
    }

    @Test(arguments: [true, false])
    func liveQueriesUseDashboardsWithoutAcquiringWork(saved: Bool) async throws {
        let fixture = String(decoding: try loadFixtureData("view_plan.json"), as: UTF8.self)
        let bytes = Data((saved ? fixture : fixture
            .replacingOccurrences(of: "\"dashboard_id\": \"picked\"", with: "\"dashboard_id\": null")
            .replacingOccurrences(of: "\"saved_revision\": 3", with: "\"saved_revision\": null")).utf8)
        let expression = saved ? "query:picked" : "state:backlog or task:LOO-497"
        let plan = try JSONDecoder().decode(WorkViewPlan.self, from: bytes)
        let query = RegistryQuery { args, _ in
            if args == ["desktop", "view", expression, "--plan", "--json"] { return String(decoding: bytes, as: UTF8.self) }
            if args.starts(with: ["desktop", "layout"]) { return "null" }
            throw RegistryQueryError("Unexpected execution")
        }
        let model = WorkModel(query: query, repoPath: "/repo")
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        model.bindWorkspaces(registry, repository: plan.repository)
        model.navigation.palette = .search
        let encoded = try #require(expression.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed))
        let link = try #require(URL(string: "loopflow://open?repo=/repo&view=\(encoded)"))
        await model.receiveWorkLink(.view(expression: expression, address: link.absoluteString))
        let state = model.inspectDesktop(repository: plan.repository, window: UUID(), workspaces: registry)
        #expect(state.selectionKind == "dashboard")
        #expect(state.selectionId == (saved ? "picked" : nil))
        #expect(model.navigation.showsSelection)
        #expect(model.navigation.palette == nil)
        #expect(state.view == plan)
        #expect(state.opening?.status == .usable)
        #expect(registry.openWork.count == 1)
        #expect(registry.inspect().allSatisfy { $0.layout.surface == nil })
        model.select(nil)
        #expect(!model.navigation.showsSelection)
        #expect(registry.openWork.count == 1)
        #expect(registry.selectedWork?.kind == .repository)
        model.navigation.palette = .search
        model.goBack()
        #expect(model.navigation.showsSelection)
        #expect(model.navigation.palette == nil)
        #expect(model.selectedViewPlan == plan)
        model.goForward()
        #expect(registry.selectedWork?.kind == .repository)
        model.goBack()
        #expect(model.selectedViewPlan == plan)
    }

    @Test func delayedRestoreRetainsNewTabsAndDraftsAndStaleSaveDoesNotReplaceThem() async throws {
        let source = fixtureWork(fixtureWorkspace("/task"))
        let location = try #require(source.location)
        source.multiplexer.load(sessionId: "primary", location: location)
        source.multiplexer.newShell(command: ["never-launch"], location: location)
        let oldShell = try #require(source.multiplexer.focusedPane.content.selectedShell)
        source.multiplexer.newShell(command: ["never-launch"], location: location)
        source.toggleFiles()
        let originalFiles = source.files(issue: source.key.id, cwd: "/task")
        originalFiles.selection = "old.txt"
        originalFiles.selection = "second.txt"
        let saved = source.portableLayout()
        let server = IntentReading(saved)
        let query = RegistryQuery(runWithInput: { _, _, input in try await server.write(input) }) { args, _ in
            if args.starts(with: ["desktop", "layout"]) { return await server.read() }
            if args.contains("file") {
                return #"{"recoveries":[],"issue_identifier":"TASK","task_id":"task","path":"new.txt","content":"base","state":"text","revision":"base","read_only_reason":null,"size_bytes":4}"#
            }
            throw RegistryQueryError("No execution allowed")
        }
        let workspace = fixtureWork(location)
        workspace.startPersistence(query: query, repoPath: "/repo")
        await server.waitForRead()
        workspace.multiplexer.newShell(command: ["new-shell"], location: location)
        let fresh = try #require(workspace.multiplexer.focusedPane.content.selectedShell)
        workspace.toggleFiles()
        let files = workspace.files(issue: workspace.key.id, cwd: "/task")
        files.selection = "new.txt"
        let document = files.document("new.txt")
        let draft = WorkFileDraft(location: .init(machineID: location.machineId, cwd: location.worktree),
            path: "new.txt", baseRevision: "base", baseContent: "base", text: "new draft")
        document.restore(draft, task: workspace.key.id)
        try await server.release(advance: true)
        let durable = await server.saved
        try await eventually { workspace.multiplexer.shell(oldShell.id) != nil && !workspace.isRestoringIntent }
        #expect(workspace.multiplexer.shell(fresh.id) == fresh)
        #expect(workspace.multiplexer.shell(oldShell.id)?.command == nil)
        #expect(files.selection == "new.txt")
        #expect(Set(files.tabs.map(\.path)) == ["new.txt", "old.txt", "second.txt"])
        #expect(files.document("new.txt") === document)
        #expect(document.text == "new draft")
        let before = workspace.portableLayout()
        await workspace.savePaneIntent()
        try await eventually { workspace.persistenceError != nil }
        #expect(workspace.persistenceError == "Revision conflict")
        #expect(workspace.portableLayout() == before)
        #expect(await server.saved == durable)
    }

    @Test func coldRestorationSavesLaterEditsWithoutLaunchingShells() async throws {
        let source = fixtureWork(fixtureWorkspace("/task"))
        source.multiplexer.newShell(command: ["must-not-replay"], location: source.location)
        let original = try #require(source.multiplexer.focusedPane.content.selectedShell)
        let server = IntentReading(source.portableLayout().revised(1))
        let query = RegistryQuery(runWithInput: { _, _, input in try await server.write(input) }) { args, _ in
            if args.starts(with: ["desktop", "layout"]) { return await server.read() }
            throw RegistryQueryError("Unexpected execution")
        }
        let workspace = fixtureWork(try #require(source.location))
        workspace.startPersistence(query: query, repoPath: "/repo")
        await server.waitForRead()
        try await server.release()
        try await eventually { !workspace.isRestoringIntent }
        #expect(workspace.multiplexer.shell(original.id)?.command == nil)
        let reload = Task { await workspace.loadPaneIntent() }
        await server.waitForRead()
        try await server.release()
        await reload.value
        #expect(await server.saved.revision == 1)
        workspace.multiplexer.newShell(command: ["new-tab"], location: source.location)
        let added = try #require(workspace.multiplexer.focusedPane.content.selectedShell)
        try await eventually { workspace.portableLayout().revision == 2 }
        let durable = await server.saved
        #expect(durable.revision == 2)
        #expect(durable == workspace.portableLayout())
        #expect(workspace.multiplexer.shell(added.id) == added)
        #expect(workspace.persistenceError == nil)
    }

    private func eventually(_ condition: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(5)
        while !condition(), ContinuousClock.now < deadline { await Task.yield() }
        try #require(condition())
    }
}

private actor IntentReading {
    var saved: WorkPaneLayout
    var continuation: CheckedContinuation<String, Never>?
    init(_ saved: WorkPaneLayout) { self.saved = saved }
    func read() async -> String { await withCheckedContinuation { continuation = $0 } }
    func waitForRead() async {
        while continuation == nil { await Task.yield() }
    }
    func write(_ input: String) throws -> String {
        let proposed = try JSONDecoder().decode(WorkPaneLayout.self, from: Data(input.utf8))
        guard proposed.revision == saved.revision else { throw RegistryQueryError("Revision conflict") }
        saved = proposed.revised(saved.revision + 1)
        return String(decoding: try JSONEncoder().encode(saved), as: UTF8.self)
    }
    func release(advance: Bool = false) throws {
        let data = try JSONEncoder().encode(saved)
        if advance { saved = saved.revised(saved.revision + 1) }
        continuation?.resume(returning: String(decoding: data, as: UTF8.self))
        continuation = nil
    }
}
private extension WorkArrangementTree {
    var firstWork: WorkWorkspaceKey {
        switch self { case .member(let member): member.work; case .split(_, _, let first, _): first.firstWork }
    }
}
private extension WorkPaneLayout {
    func revised(_ revision: Int64) -> WorkPaneLayout {
        WorkPaneLayout(work: work, revision: revision, tree: tree, focused: focused, zoomed: zoomed, hidden: hidden, drafts: drafts)
    }
}

#endif
