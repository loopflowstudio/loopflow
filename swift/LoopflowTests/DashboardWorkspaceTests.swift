#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Dashboard workspace integration", .serialized)
@MainActor
struct DashboardWorkspaceTests {
    @Test func dashboardsReuseWorkAndKeepRepositoryNavigation() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let plan = try JSONDecoder().decode(WorkViewPlan.self, from: Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/view_plan.json")))
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
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let fixture = try String(contentsOf: root.appendingPathComponent("tests/fixtures/dto/view_plan.json"), encoding: .utf8)
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

    @Test func hiddenWorkRetainsPlacementWhileVisibleSplitsChange() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let a = WorkWorkspaceKey(repository: "repo", kind: .task, id: "a")
        let b = WorkWorkspaceKey(repository: "repo", kind: .task, id: "b")
        let c = WorkWorkspaceKey(repository: "repo", kind: .task, id: "c")
        registry.open(a, title: "A", location: nil)
        registry.open(b, title: "B", location: nil, besideFocused: true)
        registry.resize(between: a, and: b, ratio: 0.7)
        registry.hide(a)
        registry.open(c, title: "C", location: nil, besideFocused: true)
        registry.resize(between: b, and: c, ratio: 0.6)
        registry.open(a, title: "A", location: nil)
        #expect(registry.layout == .split(.vertical, .workspace(a),
            .split(.vertical, .workspace(b), .workspace(c), 0.6), 0.7))
    }

    @Test func liveViewUsesTheSamePlanWithoutAcquiringWork() async throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let bytes = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/view_plan.json"))
        let plan = try JSONDecoder().decode(WorkViewPlan.self, from: bytes)
        let query = RegistryQuery { args, _ in
            if args == ["desktop", "view", "query:picked", "--plan", "--json"] { return String(decoding: bytes, as: UTF8.self) }
            if args.starts(with: ["desktop", "layout"]) { return "null" }
            throw RegistryQueryError("Unexpected execution")
        }
        let model = WorkModel(query: query, repoPath: "/repo")
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        model.bindWorkspaces(registry, repository: plan.repository)
        let link = try #require(URL(string: "loopflow://open?repo=/repo&view=query:picked"))
        await model.receiveWorkLink(link)
        let state = model.inspectDesktop(repository: plan.repository, window: UUID(), workspaces: registry)
        #expect(state.selectionKind == "dashboard")
        #expect(state.selectionId == "picked")
        #expect(state.view == plan)
        #expect(state.opening?.status == .usable)
        #expect(registry.openWork.count == 1)
        #expect(registry.inspect().allSatisfy { $0.layout.surface == nil })
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
        workspace.multiplexer.newShell(command: ["new-tab"], location: source.location)
        let added = try #require(workspace.multiplexer.focusedPane.content.selectedShell)
        await workspace.savePaneIntent()
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
