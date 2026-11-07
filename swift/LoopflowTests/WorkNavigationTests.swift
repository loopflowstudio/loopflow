#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Unified Work and Session navigation")
@MainActor
struct WorkNavigationTests {
    @Test("Every navigator entry restores its caller after a previous accepted navigation")
    func navigatorFocusResetsForSidebarEntry() {
        let navigation = WorkNavigation()
        navigation.palette = .search
        navigation.restorePaletteFocus = false
        navigation.palette = nil
        #expect(!navigation.restorePaletteFocus)
        navigation.palette = .sidebarSearch
        #expect(navigation.restorePaletteFocus)
        navigation.palette = nil
        #expect(navigation.restorePaletteFocus)
    }

    @Test("Sidebar search includes its filtered Tasks and Waves without changing the open Work")
    func sidebarSearchScope() throws {
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(roadmapJSON().utf8))
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Search is passive") }, repoPath: "/src/loopflow")
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        model.select(.task(id: "issue-now"))
        let selection = model.selection
        let order = model.workingTaskIDs
        let expected = Set(model.sidebarTaskIDs.map { WorkDestination.task($0) }
            + model.visibleRoadmaps.map { WorkDestination.wave($0.wave.id) })
        #expect(!expected.isEmpty)
        #expect(Set(model.searchDestinations("", sidebarOnly: true).map(\.id)) == expected)
        #expect(model.searchDestinations("", sidebarOnly: true).allSatisfy { $0.id != .repository })
        model.navigation.sidebarFilter = .available
        let filtered = model.searchDestinations("", sidebarOnly: true)
        #expect(filtered.allSatisfy { row in
            if case .task(let id) = row.id { return model.sidebarTaskIDs.contains(id) }
            if case .wave = row.id { return true }
            return false
        })
        #expect(model.searchDestinations("", sidebarOnly: false).contains { $0.id == .repository })
        #expect(model.selection == selection)
        #expect(model.workingTaskIDs == order)
    }

    @Test("Working Tasks are the repository plan's members, not visits; a Wave scopes the sidebar until Back")
    func workingTasksFollowTheRepositoryPlan() throws {
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(roadmapJSON().utf8))
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Viewing planning must not execute") }, repoPath: "/src/loopflow")
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        let wave = try #require(snapshot.waves.first?.wave.id)
        func task(_ id: String) -> WorkViewPlan.Content {
            fixtureViewContent(work: .init(repository: "repo", kind: .task, id: id), title: id)
        }
        model.select(.task(id: "issue-review"))
        #expect(model.workingTaskIDs.isEmpty)
        model.applySelectionPlans([
            fixtureViewPlan(owner: .init(repository: "repo", kind: .repository, id: "repo"), members: [task("issue-now")]),
            // Rust lists a Wave's Working Tasks only, as at the repository.
            fixtureViewPlan(owner: .init(repository: "repo", kind: .wave, id: wave), members: [task("issue-now")]),
        ], reason: "")
        #expect(model.workingTaskIDs == ["issue-now"])
        #expect(model.sidebarTaskIDs == ["issue-now"])

        model.select(.wave(id: wave))
        #expect(model.navigation.sidebarWaveID == wave)
        #expect(model.sidebarTaskIDs == ["issue-now"])
        #expect(model.sidebarWaves.isEmpty)
        model.select(.task(id: "issue-review"))
        #expect(model.sidebarTaskIDs == ["issue-now"])
        model.select(nil)
        #expect(model.selection == nil)
        #expect(model.sidebarTaskIDs == ["issue-now"])
    }

    @Test("Breadcrumb navigation and sidebar toggling retain panes and width")
    func breadcrumbAndSidebar() async throws {
        let model = try model()
        await model.refresh()
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        model.bindWorkspaces(registry, repository: "repo")
        let key = try #require(registry.selectedWork)
        let root = try #require(registry.retained(key))
        let layout = root.multiplexer.layout
        let crumbs = WorkBreadcrumbs(model: model)
        model.navigation.resizeSidebar(460, availableWidth: 1200)
        model.select(.wave(id: "wave-1"))
        try crumbs.inspect().find(button: "loopflow").tap()
        #expect(model.selection == nil)
        model.navigation.palette = .search
        model.navigation.toggleNavigator()
        #expect(model.navigation.palette == nil)
        #expect(!model.navigation.showsNavigator)
        model.navigation.toggleNavigator()
        #expect(model.navigation.showsNavigator)
        #expect(model.navigation.sidebarWidth == 460)
        #expect(root.multiplexer.layout == layout)
    }

    @Test("Each breadcrumb's › lists what is inside it and searches that scope; ⌘K keeps Find Work")
    func jumpBar() async throws {
        let model = try model()
        await model.refresh()
        model.select(.task(id: "issue-review"))
        let crumbs = WorkBreadcrumbs(model: model)
        for scope in ["repository", "wave-wave-1"] {
            _ = try crumbs.inspect().find(viewWithAccessibilityIdentifier: "work-jump-" + scope)
        }
        // The jump bar is navigation; Auto-Ship lives in the Workflow bar below it.
        #expect(throws: (any Error).self) { try crumbs.inspect().find(viewWithAccessibilityIdentifier: "task-ship") }
        #expect(throws: (any Error).self) { try crumbs.inspect().find(text: "This Mac") }
        #expect(throws: (any Error).self) { try crumbs.inspect().find(viewWithAccessibilityLabel: "Navigate Tasks, Waves and repositories") }
        #expect(model.jumpRows(.repository, filter: "").map(\.id) == [.wave("wave-1")])
        #expect(model.jumpRows(.repository, filter: "W2-131").map(\.id) == [.task("issue-review")])
        let wave = try #require(model.wave(id: "wave-1"))
        #expect(Set(model.jumpRows(.wave("wave-1"), filter: "").map(\.id)) == Set(wave.tasks.items.map { .task($0.id) }))
        #expect(model.jumpRows(.wave("wave-1"), filter: "W2-144").map(\.id) == [.task("issue-now")])
        model.navigation.palette = .search
        model.jump(to: .wave("wave-1"), source: "test")
        #expect(model.selection == .wave(id: "wave-1"))
        #expect(model.navigation.palette == nil)
    }

    @Test("The heading reads the Task's lifecycle, never Linear's planning state")
    func headingAgreesWithWorkingTasks() throws {
        let tasks = try roadmap().tasks.items
        func state(_ id: String) throws -> String { TaskHeading(task: try #require(tasks.first { $0.id == id })).state }
        #expect(tasks.first { $0.id == "issue-now" }?.task.state == "unstarted")
        #expect(try state("issue-now") == "Shipping")
        #expect(try state("issue-review") == "Unstarted")
        #expect(try state("issue-later") == "Done")
    }

    @Test("Working Tasks opens as a Dashboard; a sidebar Task keeps it in the trail, as its tile does")
    func workingTasksDashboardKeepsSidebarTaskOnTheBoard() throws {
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Viewing must not execute") }, repoPath: "/src/loopflow")
        model.applyFixture(roadmap: .available(try roadmap()), waves: .available([]), workActivity: .loading, repos: [])
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        model.bindWorkspaces(registry, repository: "repo")
        let tasks = ["issue-review", "issue-now"].map {
            fixtureViewContent(work: .init(repository: "repo", kind: .task, id: $0), title: $0)
        }
        model.applySelectionPlans([fixtureViewPlan(owner: .init(repository: "repo", kind: .repository, id: "repo"), members: tasks)], reason: "")
        model.select(.task(id: "issue-review"))
        model.navigation.selectedSessionId = "a-session-of-issue-review"
        model.selectWorkingTasksDashboard()
        #expect(model.navigation.showsSelection)
        #expect(model.navigation.selectedSessionId == nil)
        #expect(model.breadcrumb == nil)
        #expect(model.workingTaskMembers.map(\.work.id) == ["issue-review", "issue-now"])
        model.select(.task(id: "issue-now"))
        #expect(model.navigation.destination == .entered(from: .workingTasks, work: .task(id: "issue-now")))
        #expect(model.breadcrumb?.task?.task.id == "issue-now")
        model.goBack()
        #expect(model.navigation.destination == .workingTasks)
    }

    @Test("A Dashboard scopes the sidebar to its visible Tasks in arrangement order; ⌘N, ⌃⇥ and rows keep the board")
    func dashboardScopeListsMembersAndKeepsTheBoard() throws {
        let model = try dashboardModel()
        #expect(model.sidebarTaskIDs == ["issue-review", "issue-now"])
        model.selectDashboard("review")
        #expect(model.navigation.sidebarScope == .dashboard("review"))
        #expect(model.sidebarTaskIDs == ["issue-now", "issue-later", "issue-review"])
        #expect(model.sidebarWaves.map(\.wave.id) == ["wave-1"])
        let board = WorkNavigation.Board.saved("review")
        model.selectSidebarTask(slot: 1)
        #expect(model.navigation.destination == .entered(from: board, work: .task(id: "issue-later")))
        model.cycleSidebarTask(1)
        #expect(model.navigation.destination == .entered(from: board, work: .task(id: "issue-review")))
        model.cycleSidebarTask(1)
        #expect(model.navigation.destination == .entered(from: board, work: .task(id: "issue-now")))
        #expect(model.sidebarTaskIDs == ["issue-now", "issue-later", "issue-review"])
        model.select(.task(id: "issue-available"))
        #expect(model.navigation.destination == .work(.task(id: "issue-available")))
        #expect(model.navigation.sidebarScope == .repository)
        model.goBack()
        #expect(model.navigation.destination == .entered(from: board, work: .task(id: "issue-now")))
        model.select(nil)
        #expect(model.navigation.sidebarScope == .repository)
        #expect(model.sidebarTaskIDs == model.workingTaskIDs)
        // The repository page is a Wave view, so its Tasks enter through it too.
        model.selectSidebarTask(slot: 1)
        #expect(model.navigation.destination == .entered(from: .repository, work: .task(id: "issue-now")))
        #expect(model.navigation.wavePages[.repository] == 1)
        #expect(model.desktopDestination(repository: "repo")
            == .entered(from: .repository, work: .init(repository: "repo", kind: .task, id: "issue-now")))
    }

    @Test("Inside a Wave view, the sidebar and ⌘N count its started Tasks and enter through the Wave on the Task's page")
    func waveScopeSlotsCountTheWave() throws {
        let model = try dashboardModel()
        model.select(.wave(id: "wave-1"))
        #expect(model.sidebarTaskIDs == ["issue-now"])
        #expect(model.boardTiles(.wave("wave-1")).map(\.work.id) == ["issue-now"])
        model.selectSidebarTask(slot: 0)
        #expect(model.navigation.destination == .entered(from: .wave("wave-1"), work: .task(id: "issue-now")))
        #expect(model.navigation.wavePages[.wave("wave-1")] == 0)
        #expect(model.navigation.sidebarScope == .wave("wave-1"))
        model.selectSidebarTask(slot: 1)
        #expect(model.selection == .task(id: "issue-now"))
        model.goBack()
        #expect(model.navigation.destination == .work(.wave(id: "wave-1")))
        model.selectDashboard("review")
        #expect(model.navigation.sidebarWaveID == nil)
    }

    @Test("A Wave row counts its working and autonomous Tasks, not backlog or done")
    func waveRowCountsStartedTasks() throws {
        let model = try dashboardModel(claimed: true)
        // wave-1: shipping issue-now counts; unstarted, never-started and completed Tasks don't.
        #expect(model.startedTaskCount(.wave("wave-1")) == 1)
        let navigator = WorkNavigator(model: model, onOpenSession: { _ in })
        let row = try navigator.inspect().find(viewWithAccessibilityIdentifier: "work-wave-wave-1")
        #expect(try row.find(text: "1").string() == "1")
    }

    @Test("Shipping Tasks sit under an Autonomous heading below the Working Tasks, in light and dark",
          arguments: [ColorScheme.light, .dark])
    func autonomousSectionListsShippingTasks(scheme: ColorScheme) throws {
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Viewing must not execute") }, repoPath: "/src/loopflow")
        model.applyFixture(roadmap: .available(try claimedRoadmap()), waves: .available([]), workActivity: .loading, repos: [])
        // Rust's Working Tasks leave shipping issue-now out; its Autonomous section holds it.
        model.applySelectionPlans([fixtureViewPlan(owner: .init(repository: "repo", kind: .repository, id: "repo"),
            members: [fixtureViewContent(work: .init(repository: "repo", kind: .task, id: "issue-review"), title: "issue-review")])],
            reason: "")
        #expect(model.sidebarTaskIDs == ["issue-review"])
        #expect(model.sidebarAutonomousTaskIDs == ["issue-now"])
        let navigator = WorkNavigator(model: model, onOpenSession: { _ in }).environment(\.colorScheme, scheme)
        let heading = try navigator.inspect().find(viewWithAccessibilityIdentifier: "work-autonomous-heading")
        _ = try heading.find(text: "Autonomous")
        model.navigation.sidebarFilter = .available
        #expect(model.sidebarAutonomousTaskIDs.isEmpty)
    }

    @Test("A dropped Task row takes its place before the order is written")
    func droppedRowMovesBeforeTheWrite() async throws {
        let model = WorkModel(query: RegistryQuery { args, _ in
            if args.contains("move") { try await Task.sleep(for: .seconds(30)) }
            throw RegistryQueryError("unused")
        }, repoPath: "/src/loopflow")
        model.applyFixture(roadmap: .available(try roadmap()), waves: .available([]), workActivity: .loading, repos: [])
        func task(_ id: String) -> WorkViewPlan.Content {
            fixtureViewContent(work: .init(repository: "repo", kind: .task, id: id), title: id)
        }
        model.applySelectionPlans([fixtureViewPlan(owner: .init(repository: "repo", kind: .repository, id: "repo"),
                                                   members: [task("issue-review"), task("issue-now")])], reason: "")
        #expect(model.workingTaskIDs == ["issue-review", "issue-now"])
        let write = model.reorderWorkingTask("issue-now", to: "issue-review")
        #expect(model.workingTaskIDs == ["issue-now", "issue-review"])
        write.cancel()
        await write.value
    }

    @Test("Abandoning the open Task drops its row and opens the next one before the command returns; a refusal puts it back")
    func abandonLeavesTheSidebarAtOnce() async throws {
        let model = WorkModel(query: RegistryQuery { args, _ in
            if args.contains("abandon") {
                try await Task.sleep(for: .milliseconds(50))
                throw RegistryQueryError("Task has unpushed commits")
            }
            throw RegistryQueryError("unused")
        }, repoPath: "/src/loopflow")
        model.applyFixture(roadmap: .available(try roadmap()), waves: .available([]), workActivity: .loading, repos: [])
        func task(_ id: String) -> WorkViewPlan.Content {
            fixtureViewContent(work: .init(repository: "repo", kind: .task, id: id), title: id)
        }
        model.applySelectionPlans([fixtureViewPlan(owner: .init(repository: "repo", kind: .repository, id: "repo"),
                                                   members: [task("issue-review"), task("issue-now")])], reason: "")
        model.select(.task(id: "issue-review"))
        let abandoned = try #require(model.task(id: "issue-review")?.task)
        let abandon = Task { await model.abandonTask(abandoned) }
        for _ in 0..<20 where !model.abandoningTaskIDs.contains("issue-review") { await Task.yield() }
        #expect(model.sidebarTaskIDs == ["issue-now"])
        #expect(model.selection == .task(id: "issue-now"))
        await abandon.value
        #expect(model.sidebarTaskIDs == ["issue-review", "issue-now"])
        #expect(model.dashboardError == "Task has unpushed commits")
    }

    @Test("A row dragged over another lands there on release and leaves no drag behind")
    func draggedRowLandsOnRelease() {
        let drag = TaskOrderDrag.State()
        drag.frames = ["a": CGRect(x: 0, y: 0, width: 200, height: 60),
                       "b": CGRect(x: 0, y: 65, width: 200, height: 60),
                       "c": CGRect(x: 0, y: 130, width: 200, height: 60)]
        drag.move("a", by: 10, at: 40, order: ["a", "b", "c"])
        #expect(drag.target == nil)
        drag.move("a", by: 120, at: 150, order: ["a", "b", "c"])
        #expect(drag.landing(on: "c") == .bottom)
        drag.move("a", by: 80, at: 90, order: ["a", "b", "c"])
        #expect(drag.landing(on: "b") == .bottom)
        let landed = drag.end()
        #expect(landed?.0 == "a" && landed?.1 == "b")
        #expect(drag.source == nil && drag.target == nil && drag.offset == 0)
    }

    @Test("The sidebar header names the Dashboard under the repository, and ⌘1 marks its first member")
    func dashboardSidebarHeader() throws {
        let model = try dashboardModel()
        model.selectDashboard("review")
        let navigator = WorkNavigator(model: model, onOpenSession: { _ in })
        _ = try navigator.inspect().find(text: "REVIEW")
        _ = try navigator.inspect().find(viewWithAccessibilityIdentifier: "work-wave-repository")
        let first = try navigator.inspect().find(TaskRowMetadata.self) { try $0.actualView().slotHint == "⌘1" }
        #expect(try first.actualView().task.id == "issue-now")
    }

    @Test("Zooming a Working Tasks tile keeps Working Tasks in the trail, and Back returns to it")
    func workingTasksZoomPassesThroughDashboard() throws {
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Viewing must not execute") }, repoPath: "/src/loopflow")
        model.applyFixture(roadmap: .available(try roadmap()), waves: .available([]), workActivity: .loading, repos: [])
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        model.bindWorkspaces(registry, repository: "repo")
        let tasks = ["issue-review", "issue-now"].map {
            fixtureViewContent(work: .init(repository: "repo", kind: .task, id: $0), title: $0)
        }
        model.applySelectionPlans([fixtureViewPlan(owner: .init(repository: "repo", kind: .repository, id: "repo"), members: tasks)], reason: "")
        model.selectWorkingTasksDashboard()
        let member = try #require(model.workingTaskMembers.first { $0.work.id == "issue-now" })
        model.enterTile(member.work, origin: nil)
        #expect(model.navigation.trailBoard == .workingTasks)
        #expect(model.breadcrumb?.task?.task.id == "issue-now")
        model.goBack()
        #expect(model.navigation.destination == .workingTasks)
    }

    @Test("An unstarted Task's Session slot offers Start; showing it starts nothing")
    func unstartedTaskOffersStart() async throws {
        let model = try model()
        await model.refresh()
        let task = try #require(model.task(id: "issue-now")?.task)
        #expect(task.awaitsStart)
        #expect(try #require(model.task(id: "issue-review")?.task).awaitsStart == false)
        #expect(try #require(model.task(id: "issue-later")?.task).awaitsStart == false)
        let offer = TaskStartOffer(model: model, task: task)
        let start = try offer.inspect().find(viewWithAccessibilityIdentifier: "task-start-issue-now").button()
        #expect(!start.isDisabled())
        #expect(throws: Never.self) { try offer.inspect().find(text: task.task.description) }
        #expect(model.navigation.taskRunDrafts[task.id] == nil)
    }

    @Test("Selecting Work closes Find Work and preserves the sidebar preference", arguments: [false, true])
    func selectionClosesSearch(pinned: Bool) async throws {
        let model = try model()
        await model.refresh()
        model.navigation.showsNavigator = pinned
        for destination: WorkReference? in [.wave(id: "wave-1"), nil] {
            model.navigation.palette = .search
            model.select(destination)
            #expect(model.navigation.palette == nil)
            #expect(model.navigation.showsNavigator == pinned)
        }
    }

    @Test("Navigation shortcuts leave Escape and extra-modifier provider shortcuts alone")
    func navigationShortcutOwnership() {
        #expect(WorkNavigationShortcut(characters: "k", modifiers: .command) == .search)
        #expect(WorkNavigationShortcut(characters: "\\", modifiers: [.command, .capsLock]) == .toggleSidebar)
        for modifiers: NSEvent.ModifierFlags in [[], [.control], [.command, .shift], [.command, .option]] {
            #expect(WorkNavigationShortcut(characters: "k", modifiers: modifiers) == nil)
            #expect(WorkNavigationShortcut(characters: "s", modifiers: modifiers) == nil)
        }
        #expect(WorkNavigationShortcut(characters: "\u{1b}", modifiers: []) == nil)
        #expect(WorkNavigationShortcut(characters: "c", modifiers: .command) == nil)
        #expect(WorkNavigationShortcut(characters: "s", modifiers: [.control, .command]) == nil)
        func action(_ characters: String, _ modifiers: NSEvent.ModifierFlags) -> String? {
            DesktopAction.binding(characters: characters, modifiers: modifiers).map { "\($0.0.name) \($0.1)" }
        }
        #expect(action("1", .command) == "goto [\"index\": \"1\"]")
        #expect(action("9", [.command, .capsLock]) == "goto [\"index\": \"9\"]")
        #expect(action("0", .command) == nil)
        #expect(action("1", [.command, .option]) == nil)
        #expect(action("\t", .control) == "next [:]")
        #expect(action("\u{19}", [.control, .shift]) == "previous [:]")
        #expect(action("\t", []) == nil)
    }

    @Test("Repository view reconstruction keeps the Session store's resolved location")
    func repositoryViewKeepsResolvedLocation() throws {
        let repo = "/workspace-alias-\(UUID().uuidString)"
        WaveOrigin.remember(origin: repo)
        let alias = repo + "/"
        let query = RegistryQuery { _, _ in throw RegistryQueryError("No reads expected") }
        let model = WorkModel(query: query, repoPath: alias)
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let key = model.workspaceKey(for: nil, repository: "repo")
        for _ in 0..<3 {
            let view = SessionsContentView(model: model, repoPath: alias, workspaces: registry,
                                          machineId: fixtureMachineId, repository: "repo", query: query)
            let root = try #require(registry.retained(key))
            let store = root.sessionStore(repoPath: alias, query: query)
            #expect(root.location?.worktree == store.repoPath)
            _ = view.body
            #expect(root.location?.worktree == store.repoPath)
        }
    }

    @Test("Repository conversation reuses its exact pane without selecting another Work")
    func retainedRepositoryConversation() async throws {
        let record = try session("repo-primary", work: nil)
        let json = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let gate = PlanningGate()
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        let key = WorkWorkspaceKey(repository: "repo", kind: .repository, id: "repo")
        let entry = registry.open(key, title: "Repository", location: record.workspace?.identity)
        let store = entry.sessionStore(repoPath: "/src/loopflow", query: RegistryQuery { args, _ in
            // Gate the repository conversation's preparation; showing it then attaches at once.
            if args.first == "--repo" { await gate.wait() }
            return json
        })
        let first = Task { await entry.openPrimaryConversation(store: store, registry: registry) }
        await gate.started()
        let otherKey = WorkWorkspaceKey(repository: "repo", kind: .task, id: "other")
        let other = registry.open(otherKey, title: "Other", location: record.workspace?.identity)
        let otherLayout = other.multiplexer.layout
        await entry.openPrimaryConversation(store: store, registry: registry)
        await gate.release()
        await first.value
        let pane = try #require(entry.multiplexer.pane(forSessionId: record.id))
        #expect(registry.selectedWork == otherKey)
        #expect(other.multiplexer.layout == otherLayout)
        #expect(entry.presentation.selectedSessionId == record.id)
        let second = Task { await entry.openPrimaryConversation(store: store, registry: registry) }
        await gate.started()
        await gate.release()
        await second.value
        #expect(entry.multiplexer.pane(forSessionId: record.id)?.id == pane.id)
        #expect(entry.conversationError == nil)
        let model = WorkModel(query: store.query, repoPath: "/src/loopflow")
        let view = WorkNavigator(model: model, onOpenSession: { _ in })
        #expect(throws: (any Error).self) { try view.inspect().find(viewWithAccessibilityIdentifier: "work-create-task") }
        #expect(throws: (any Error).self) { try view.inspect().find(viewWithAccessibilityIdentifier: "work-headless-sessions") }
    }

    @Test("Work tabs retain their own conversation and details choice across aliases and close/reopen")
    func workTabsRetainSelection() throws {
        let snapshot = try roadmap()
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Navigation must not launch or read") }, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        model.bindWorkspaces(registry, repository: "repo")
        let root = try #require(registry.selectedWork)
        model.select(.task(id: task.id))
        let taskKey = try #require(registry.selectedWork)
        let workspace = try #require(registry.retained(taskKey))
        model.navigation.content = .terminals
        model.navigation.selectedSessionId = "retained-conversation"
        workspace.multiplexer.reveal(sessionId: "retained-conversation")
        let pane = workspace.multiplexer.focusedPane
        model.select(.wave(id: wave.wave.id))
        #expect(model.navigation.selectedSessionId == nil)
        model.select(.task(id: task.runtime?.workId ?? task.id))
        #expect(registry.selectedWork == taskKey)
        #expect(registry.retained(taskKey) === workspace)
        #expect(model.navigation.selectedSessionId == "retained-conversation")
        #expect(model.navigation.content == .terminals)
        #expect(workspace.multiplexer.focusedPane == pane)
        model.select(nil)
        #expect(registry.selectedWork == root)
        model.select(.task(id: task.id))
        #expect(model.navigation.selectedSessionId == "retained-conversation")
        #expect(registry.openWork.filter { $0 == taskKey }.count == 1)
    }

    @Test("Checkout association groups independent Sessions despite different attribution")
    func checkoutOwnsGroupingWithoutGrantingGraphMembership() throws {
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(roadmapJSON().utf8))
        let original = try session("independent", work: .task(id: "another-task"))
        var json = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(original)) as? [String: Any])
        json["workspace"] = ["machine_id": "local", "worktree": "/src/loopflow.review",
                             "task_id": "ts_review00000000000000000000000000"]
        let associated = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: json))
        let projection = WorkProjection(roadmaps: roadmap.waves, inventory: roadmap.tasks.items, sessions: [associated])
        let task = try #require(projection.waves.flatMap(\.tasks).first { !$0.sessions.isEmpty })
        #expect(task.task.runtime?.workId == associated.workspace?.taskId)
        #expect(associated.work == .task(id: "another-task"))
        #expect(associated.graphMembership == .independent)
        #expect(projection.unmatchedSessions.isEmpty)
        #expect(projection.subject(for: associated.id) == task.id.work)
    }

    @Test("Checkout-only Sessions use Rust membership in Task navigation")
    func checkoutMembershipKeepsUnboundConversationReachable() throws {
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(roadmapJSON().utf8))
        let bindings: [WorkReference?] = [nil, .wave(id: roadmap.waves[0].wave.id)]
        for work in bindings {
            let original = try session("manual", work: work)
            var json = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(original)) as? [String: Any])
            json["task_ids"] = ["ts_review00000000000000000000000000"]
            let record = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: json))
            let projection = WorkProjection(roadmaps: roadmap.waves, inventory: roadmap.tasks.items, sessions: [record])
            #expect(record.work == work)
            #expect(projection.subject(for: "manual") == .task(id: "issue-review"))
            #expect(projection.unmatchedSessions.isEmpty)
            #expect(projection.waves.allSatisfy { $0.sessions.isEmpty })
            #expect(projection.breadcrumb(selection: nil, sessionId: "manual")?.task?.task.id == "issue-review")
        }
    }

    @Test("Repo and Wave Sessions cannot join a Task through checkout or stored associations")
    func explicitScopesStaySeparate() throws {
        let snapshot = try roadmap()
        let original = try session("scoped", work: .task(id: "ts_review00000000000000000000000000"))
        for scope in ["repository", "wave"] {
            var value = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(original)) as? [String: Any])
            value["primary_scope"] = scope
            let record = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: value))
            let projection = WorkProjection(roadmaps: snapshot.waves, inventory: snapshot.tasks.items, sessions: [record])
            #expect(projection.waves.flatMap(\.tasks).allSatisfy { $0.sessions.isEmpty })
            #expect(projection.unmatchedSessions.map(\.id) == ["scoped"])
            #expect(projection.subject(for: "scoped") == nil)
            #expect(projection.breadcrumb(selection: nil, sessionId: "scoped")?.taskWork == nil)
        }
    }

    @Test("Unscoped Sessions remain in history without navigation entries")
    func orphansStayOutOfTheTree() async throws {
        let attached = try session("human", work: .task(id: "ts_review00000000000000000000000000"))
        let orphan = try session("demo", work: nil)
        let stray = try session("stray", work: .task(id: "absent"))
        let projection = WorkProjection(roadmaps: try roadmap().waves, inventory: try roadmap().tasks.items, sessions: [attached, orphan, stray])

        #expect(projection.subject(for: "stray") == .task(id: "absent"))
        #expect(projection.breadcrumb(selection: nil, sessionId: "stray")?.waveWork == nil)

        let source = try ReadingSource(
            roadmap: roadmapJSON(),
            sessions: String(decoding: try JSONEncoder().encode([attached, orphan, stray]), as: UTF8.self))
        let model = WorkModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refresh()
        var opened: [String] = []
        let view = WorkNavigator(model: model, onOpenSession: { opened.append($0.id) })
        #expect(throws: (any Error).self) { try view.inspect().find(viewWithAccessibilityIdentifier: "work-orphans-open") }
        #expect(throws: (any Error).self) { try view.inspect().find(viewWithAccessibilityIdentifier: "session-row-demo") }
        #expect(throws: (any Error).self) { try view.inspect().find(viewWithAccessibilityIdentifier: "debug-orphan-session-demo") }
        #expect(throws: (any Error).self) { try view.inspect().find(viewWithAccessibilityIdentifier: "debug-session-stray") }
        #expect(opened.isEmpty)
        #expect(Set(model.projection.unmatchedSessions.map(\.id)) == ["demo", "stray"])
        #expect(throws: (any Error).self) { try view.inspect().find(text: "Repository or unavailable ancestry") }

        // Session inventory still reconciles without providing navigation.
        await source.replaceSessions(String(decoding: try JSONEncoder().encode([attached]), as: UTF8.self))
        await model.refreshSessions()
        #expect(model.projection.unmatchedSessions.isEmpty)
        #expect(throws: (any Error).self) { try view.inspect().find(viewWithAccessibilityIdentifier: "work-orphans-open") }
        model.select(.wave(id: "wave-1"))
    }

    @Test("Autonomous, upcoming and human work share stable ranked planning rows")
    func workDoesNotRequireSessions() throws {
        let roadmap = try roadmap()
        let empty = WorkProjection(roadmaps: roadmap.waves, inventory: roadmap.tasks.items, sessions: [])
        let interactive = try session("human", work: .task(id: "ts_review00000000000000000000000000"))
        let joined = WorkProjection(roadmaps: roadmap.waves, inventory: roadmap.tasks.items, sessions: [interactive])
        let before = empty.waves[0].tasks
        let after = joined.waves[0].tasks

        #expect(before.map(\.id) == after.map(\.id))
        #expect(after.map(\.task.id) == ["issue-later", "issue-review", "issue-now", "issue-available"])
        #expect(after[2].task.condition.state == .clear)
        #expect(after[3].task.runtime == nil)
        #expect(after[1].sessions.map(\.id) == ["human"])
        #expect(after[3].sessions.isEmpty)
        #expect(joined.subject(for: "human") == .task(id: "issue-review"))
    }

    @Test("Typed durable joins preserve top-level, multiple, completed and unmatched Sessions")
    func everyHumanBoundaryRemainsReachable() throws {
        let records = try [
            session("repo", work: nil),
            session("wave", work: .wave(id: "wave-1")),
            session("project", work: .project(id: "ps_11111111111111111111111111111111")),
            session("completed", work: .task(id: "ts_later000000000000000000000000000")),
            session("another", work: .task(id: "ts_later000000000000000000000000000")),
            session("wrong-kind", work: .project(id: "ts_review00000000000000000000000000")),
            session("planning-id-is-not-work", work: .task(id: "issue-review")),
        ]
        let projection = WorkProjection(roadmaps: try roadmap().waves, inventory: try roadmap().tasks.items, sessions: records)
        #expect(projection.waves[0].sessions.map(\.id) == ["wave", "project"])
        let completed = try #require(projection.waves[0].tasks.first)
        #expect(completed.task.task.isSuccessful)
        #expect(completed.sessions.map(\.id) == ["completed", "another"])
        #expect(projection.unmatchedSessions.map(\.id) == ["repo", "wrong-kind", "planning-id-is-not-work"])
        let missing = WorkProjection(roadmaps: [], inventory: [], sessions: records)
        #expect(missing.unmatchedSessions == records)
    }

    @Test("Presentation and repo changes preserve selection, viewport and terminal splits")
    func navigationRetainsWorkspace() async throws {
        let model = try model()
        await model.refresh()
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let workspace = fixtureWork(registry, fixtureWorkspace("/src/loopflow"))
        workspace.multiplexer.load(sessionId: "human")
        let sessionPane = workspace.multiplexer.focusedPaneId
        _ = workspace.multiplexer.newShell()
        workspace.multiplexer.newShell(location: workspace.location)
        let layout = workspace.multiplexer.layout
        let focused = workspace.multiplexer.focusedPaneId
        model.select(.task(id: "issue-review"))
        let navigation = model.navigation
        navigation.selectedSessionId = "human"
        #expect(navigation.selection == .task(id: "issue-review"))
        navigation.sidebarScrollOffset = 160
        navigation.showsNavigator = false
        navigation.content = .terminals
        model.setRepoPath("/src/context")
        model.select(.wave(id: "wave-2"))
        model.setRepoPath("/src/loopflow")

        #expect(model.selection == .task(id: "issue-review"))
        #expect(model.navigation === navigation)
        #expect(model.navigation.content == .terminals)
        #expect(model.navigation.selectedSessionId == "human")
        #expect(model.navigation.sidebarScrollOffset == 160)
        #expect(!model.navigation.showsNavigator)
        #expect(fixtureWork(registry, fixtureWorkspace("/src/loopflow")) === workspace)
        #expect(workspace.multiplexer.layout == layout)
        #expect(workspace.multiplexer.focusedPaneId == focused)
        workspace.multiplexer.load(sessionId: "human")
        workspace.multiplexer.load(sessionId: "human")
        #expect(workspace.multiplexer.focusedPaneId == sessionPane)
        #expect(workspace.multiplexer.layout == layout)
    }

    @Test("Failed reads keep last-good work and exact Sessions with a visible error")
    func unavailableIsNotEmpty() async throws {
        let source = try ReadingSource(roadmap: roadmapJSON(), sessions: sessionsJSON())
        let model = WorkModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refresh()
        let keys = model.projection.waves[0].tasks.map(\.id)
        model.select(.task(id: "issue-review"))
        await source.fail()
        await model.refresh()
        #expect(model.roadmap.errorMessage == "offline")
        #expect(model.sessions.errorMessage == "offline")
        #expect(model.projection.waves[0].tasks.map(\.id) == keys)
        #expect(model.projection.subject(for: "human") == model.selection)
        let view = WorkNavigator(model: model, onOpenSession: { _ in })
        #expect(throws: Never.self) { try view.inspect().find(text: "Couldn't update: offline") }
    }

    @Test("Returning to a repository retains its last-good Sessions when refresh fails")
    func lastGoodSessionsSurviveRepositorySwitch() async throws {
        let source = try ReadingSource(roadmap: roadmapJSON(), sessions: sessionsJSON())
        let model = WorkModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refresh()
        model.setRepoPath("/src/context")
        #expect(model.sessions.value == nil)
        await source.fail()
        model.setRepoPath("/src/loopflow")
        #expect(model.sessions.value?.map(\.id) == ["human"])
        await model.refreshSessions()
        #expect(model.sessions.errorMessage == "offline")
        #expect(model.sessions.value?.map(\.id) == ["human"])
        model.setRepoPath("/src/context")
        model.setRepoPath("/src/loopflow")
        #expect(model.sessions.value?.map(\.id) == ["human"])
        #expect(model.sessions.errorMessage == "offline")
    }

    @Test("Incomplete planning preserves the repository's selected Task", arguments: [false, true])
    func unavailablePlanningPreservesRepositorySelection(truncated: Bool) async throws {
        let source = try ReadingSource(roadmap: roadmapJSON(), sessions: sessionsJSON())
        let model = WorkModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refresh()
        model.select(.task(id: "issue-review"))
        var snapshot = try #require(JSONSerialization.jsonObject(with: Data(roadmapJSON().utf8)) as? [String: Any])
        var waves = try #require(snapshot["waves"] as? [[String: Any]])
        snapshot["tasks"] = truncated
            ? ["state": "ok", "items": [], "truncated": true]
            : ["state": "unavailable", "reason": "planning offline"]
        waves[0]["unavailable_tasks"] = []
        snapshot["waves"] = waves
        await source.replaceRoadmap(String(decoding: try JSONSerialization.data(withJSONObject: snapshot), as: UTF8.self))
        await model.refresh()
        #expect(model.selection == .task(id: "issue-review"))
        await model.applyPortfolio([], initialRepoPath: nil)
        #expect(model.selection == .task(id: "issue-review"))
        model.setRepoPath("/src/context")
        model.setRepoPath("/src/loopflow")
        #expect(model.selection == .task(id: "issue-review"))
        #expect(model.navigation.content == .details)
        #expect(model.projection.unmatchedSessions.isEmpty)
        #expect(model.roadmap.errorMessage != nil)
        #expect(model.projection.subject(for: "human") == .task(id: "issue-review"))
    }


    @Test("Task briefs and the Wave standup board share the current snapshot")
    func inspectorShowsPlanning() async throws {
        let model = try model()
        await model.refresh()
        model.select(.task(id: "issue-review"))
        let task = try #require(model.task(id: "issue-review"))
        let view = WorkSurfaceView(model: model, onNewSession: { _ in })
        #expect(throws: Never.self) { try view.inspect().find(text: task.task.task.name) }
        #expect(throws: Never.self) { try view.inspect().find(text: task.task.task.description) }
        #expect(throws: Never.self) { try view.inspect().find(button: "New session") }
        model.select(.wave(id: task.wave!.wave.id))
        let board = WorkNavigation.Board.wave(task.wave!.wave.id)
        let standup = model.standup(task.wave!.tasks.items, doneDays: WaveStandupDefaults.doneDays)
        for section in WaveBoardSection.allCases {
            model.navigation.boardSections[board] = section
            for row in standup.rows(section) {
                _ = try view.inspect().find(viewWithAccessibilityIdentifier: "wave-row-\(row.id)")
            }
        }
        // Completed work without a date is outside Done's window; Project history keeps it.
        #expect(throws: (any Error).self) { try view.inspect().find(viewWithAccessibilityIdentifier: "wave-row-issue-later") }
        #expect(throws: (any Error).self) { try view.inspect().find(TaskHistoryControls.self) }
    }

    @Test("A delayed New session failure stays with its Task across Task and repository navigation")
    func newSessionFailureKeepsItsTask() async throws {
        let model = try model()
        await model.refresh()
        model.select(.task(id: "issue-review"))
        let owner = model.navigation
        let gate = PlanningGate()
        let view = WorkSurfaceView(model: model, onNewSession: { _ in
            await gate.wait()
            throw RegistryQueryError("Preparation failed for the original Task")
        })
        try view.inspect().find(button: "New session").tap()
        await gate.started()
        #expect(try view.inspect().find(button: "New session").isDisabled())
        model.select(.task(id: "issue-available"))
        #expect(try !view.inspect().find(button: "New session").isDisabled())
        model.setRepoPath("/src/context")
        let other = model.navigation
        await gate.release()
        let deadline = Date().addingTimeInterval(3)
        while owner.startingTaskSessions.contains("issue-review"), Date() < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(owner.startingTaskSessions.isEmpty)
        #expect(other.taskSessionErrors.isEmpty)
        model.setRepoPath("/src/loopflow")
        model.select(.task(id: "issue-available"))
        #expect(throws: (any Error).self) {
            try view.inspect().find(text: "Preparation failed for the original Task")
        }
        model.select(.task(id: "issue-review"))
        #expect(throws: Never.self) {
            try view.inspect().find(text: "Preparation failed for the original Task")
        }
        #expect(try !view.inspect().find(button: "New session").isDisabled())
        // A retry clears only this Task's prior failure and settles normally.
        let retry = WorkSurfaceView(model: model, onNewSession: { _ in })
        try retry.inspect().find(button: "New session").tap()
        #expect(throws: (any Error).self) {
            try retry.inspect().find(text: "Preparation failed for the original Task")
        }
        while owner.startingTaskSessions.contains("issue-review"), Date() < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(owner.taskSessionErrors.isEmpty)
        #expect(owner.startingTaskSessions.isEmpty)
    }

    @Test("Task inspection retains Available links without a Session inventory")
    func inspectorKeepsExactAvailableConversation() async throws {
        var records = try JSONSerialization.jsonObject(with: Data(sessionsJSON().utf8)) as! [[String: Any]]
        records[0]["availability"] = "available"
        let source = try ReadingSource(roadmap: roadmapJSON(), sessions: String(decoding: JSONSerialization.data(withJSONObject: records), as: UTF8.self))
        let model = WorkModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refresh()
        model.select(.task(id: "issue-review"))
        var opened: String?
        let view = WorkSurfaceView(model: model, onOpenSession: { opened = $0.id })
        try view.inspect().find(viewWithAccessibilityIdentifier: "task-available-human").button().tap()
        #expect(opened == "human")
        #expect(throws: (any Error).self) { try view.inspect().find(viewWithAccessibilityIdentifier: "task-sessions-working") }
        #expect(throws: (any Error).self) { try view.inspect().find(text: "No open Sessions.") }
        await source.fail()
        await model.refresh()
        #expect(throws: Never.self) { try view.inspect().find(viewWithAccessibilityIdentifier: "task-available-human") }
    }

    @Test("Work outside the plan remains explained in Wave details")
    func unavailableTaskLivesInWaveDetails() async throws {
        let model = try model()
        await model.refresh()
        let wave = try #require(model.projection.waves.first)
        let task = try #require(wave.roadmap.unavailableTasks.first)
        let text = "\(task.taskIdentifier): \(task.reason)"
        let navigator = WorkNavigator(model: model, onOpenSession: { _ in })
        #expect(throws: (any Error).self) { try navigator.inspect().find(text: text) }
        model.select(wave.id.work)
        // One quiet line names the Task; Details discloses reason and recovery.
        let details = WorkSurfaceView(model: model)
        #expect(throws: Never.self) { try details.inspect().find(text: "\(task.taskIdentifier) is outside the current plan.") }
        #expect(throws: (any Error).self) { try details.inspect().find(text: text) }
        try details.inspect().find(viewWithAccessibilityIdentifier: "wave-notice-details-\(task.taskId)").button().tap()
        #expect(throws: Never.self) { try details.inspect().find(text: text) }
        #expect(throws: Never.self) { try details.inspect().find(text: task.recovery) }
    }

    @Test("Slow planning does not gate Session reading or navigation")
    func sessionsArriveBeforePlanning() async throws {
        let snapshot = try roadmapJSON()
        let records = try sessionsJSON()
        let gate = PlanningGate()
        let model = WorkModel(query: RegistryQuery { args, _ in
            switch args.first {
            case "desktop" where args.dropFirst().first == "plan": await gate.wait(); return snapshot
            case "desktop" where args.dropFirst().first == "sessions": return #"{"entries":\#(records),"next":null}"#
            case "wave" where args.dropFirst().first == "list": return "[]"
            default: return #"{"truncated":false,"items":[]}"#
            }
        }, repoPath: "/src/loopflow")
        let refresh = Task { await model.refresh() }
        await gate.started()
        await model.refreshSessions()
        #expect(model.roadmap.isLoading)
        #expect(model.projection.unmatchedSessions.map(\.id) == ["human"])
        await gate.release()
        await refresh.value
        #expect(model.projection.subject(for: "human") == .task(id: "issue-review"))
        #expect(model.projection.unmatchedSessions.isEmpty)
    }

    @Test("Bound Sessions retain ancestry and panes across planning loss and return",
          arguments: ["absent-task", "no-current-project", "no-waves"])
    func boundSessionRetainsAncestry(planning: String) async throws {
        let work = WorkReference.task(id: "ts_review00000000000000000000000000")
        let records = try [session("first", work: work, waveId: "wave-1"),
                           session("second", work: work, waveId: "wave-1")]
        let original = try roadmapJSON()
        var snapshot = try #require(JSONSerialization.jsonObject(with: Data(original.utf8)) as? [String: Any])
        var waves = try #require(snapshot["waves"] as? [[String: Any]])
        snapshot["tasks"] = ["state": "ok", "items": [], "truncated": false]
        waves[0]["unavailable_tasks"] = []
        if planning == "no-current-project" {
            waves[0]["projects"] = ["state": "ok", "items": [], "truncated": false]
        }
        snapshot["waves"] = planning == "no-waves" ? [] : waves
        let missing = String(decoding: try JSONSerialization.data(withJSONObject: snapshot), as: UTF8.self)
        let source = ReadingSource(roadmap: original, sessions: String(decoding: try JSONEncoder().encode(records), as: UTF8.self))
        let model = WorkModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refresh()
        model.select(model.projection.subject(for: "first"))
        model.navigation.selectedSessionId = "first"
        model.navigation.content = .terminals
        model.beginSessionRename(records[0])
        model.navigation.renaming?.text = "Retained draft"
        let selection = model.selection
        let registry = SessionsWorkspaceRegistry()
        let workspace = fixtureWork(registry, WorkspaceIdentity(machineId: "local", worktree: "/src/loopflow"))
        workspace.multiplexer.load(sessionId: "first")
        let pane = workspace.multiplexer.focusedPaneId
        _ = workspace.multiplexer.newShell()
        workspace.multiplexer.load(sessionId: "second")
        workspace.multiplexer.load(sessionId: "first")
        let layout = workspace.multiplexer.layout

        await source.replaceRoadmap(missing)
        await model.refresh()
        let crumb = try #require(model.projection.breadcrumb(selection: model.selection, sessionId: "first"))
        #expect(crumb.task == nil)
        #expect(crumb.taskWork == work)
        #expect(crumb.waveWork == .wave(id: "wave-1"))
        #expect(crumb.siblings.map(\.id) == ["first", "second"])
        #expect(model.projection.subject(for: "first") == work)
        #expect(model.workspaceTitle(WorkWorkspaceKey(repository: "repo", kind: .task, id: work.id)).isEmpty == false)
        #expect(Set(model.visibleWork.unmatchedSessions.map(\.id)) == ["first", "second"])
        let navigator = WorkNavigator(model: model, onOpenSession: { _ in })
        #expect(throws: (any Error).self) { try navigator.inspect().find(viewWithAccessibilityIdentifier: "debug-session-first") }
        #expect(throws: (any Error).self) { try navigator.inspect().find(viewWithAccessibilityIdentifier: "debug-session-second") }


        // Fresh discovery cannot depend on cached planning names or selection.
        let cold = WorkModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await cold.refresh()
        cold.select(cold.projection.subject(for: "first"))
        cold.navigation.selectedSessionId = "first"
        #expect(cold.selection == work)
        await source.replaceRoadmap(original)
        await model.refresh()
        await cold.refresh()
        #expect(cold.selection == work)
        #expect(cold.task(id: work.id)?.task.id == "issue-review")
        #expect(model.selection == selection)
        #expect(model.navigation.selectedSessionId == "first")
        #expect(model.navigation.content == .terminals)
        #expect(model.navigation.renaming?.text == "Retained draft")
        #expect(model.projection.breadcrumb(selection: model.selection, sessionId: "first")?.task?.task.id == "issue-review")
        #expect(workspace.multiplexer.layout == layout)
        #expect(workspace.multiplexer.focusedPaneId == pane)
    }

    @Test("Workflow loading, unavailable and absent readings remain distinct")
    func workflowReadingStates() async throws {
        let model = try model()
        await model.refresh()
        let found = try #require(model.task(id: "issue-review"))
        let empty = try JSONDecoder().decode(TaskOverview.self, from: Data(#"{"sessions":[],"graph_processes":[],"processes":[],"graph":null}"#.utf8))
        let loading = TaskWorkflowHeader(model: model, task: found.task, work: .loading)
        #expect(try loading.inspect().find(text: "Reading workflow…").string() == "Reading workflow…")
        #expect(throws: (any Error).self) { try loading.inspect().find(text: "No workflow selected") }
        let missing = TaskWorkflowHeader(model: model, task: found.task, work: .unavailable(lastGood: nil, reason: "Offline"))
        #expect(try missing.inspect().find(text: "Workflow could not be read: Offline").string() == "Workflow could not be read: Offline")
        let absent = TaskWorkflowHeader(model: model, task: found.task, work: .available(empty))
        let selection = found.task.graphName.map { "\($0) · \(found.task.graphSource ?? "")" } ?? "No workflow selected"
        #expect(try absent.inspect().find(text: selection).string() == selection)
        let stale = TaskWorkflowHeader(model: model, task: found.task, work: .unavailable(lastGood: empty, reason: "Offline"))
        #expect(try stale.inspect().find(text: "Workflow could not be read: Offline").string() == "Workflow could not be read: Offline")
        #expect(throws: (any Error).self) { try stale.inspect().find(text: "No workflow selected") }
        #expect(throws: (any Error).self) { try stale.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-start") }
    }

    @Test("Session rename is scoped to its draft without a second breadcrumb")
    func sessionActionSheetKeepsTarget() async throws {
        let model = try model()
        await model.refresh()
        let record = try session("secondary", work: .task(id: "ts_review00000000000000000000000000"))
        model.select(.wave(id: "wave-1"))
        model.beginSessionRename(record)
        #expect(model.selection == .wave(id: "wave-1"))
        let sheet = SessionActionSheet(model: model)
        #expect(model.navigation.renaming?.sessionId == record.id)
        #expect(model.navigation.renaming?.text == record.title)
        _ = try sheet.inspect().find(viewWithAccessibilityIdentifier: "session-rename-field")
        #expect(throws: (any Error).self) { try sheet.inspect().find(viewWithAccessibilityIdentifier: "breadcrumb-session") }
    }

    @Test("Unbound Sessions do not acquire ancestry from display text or checkout")
    func unboundSessionHasNoAncestry() throws {
        let record = try session("unbound", work: nil)
        var json = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(record)) as? [String: Any])
        json["work_path"] = "wave-1/issue-review"
        json["title"] = "Task ts_review00000000000000000000000000"
        let unbound = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: json))
        let projection = WorkProjection(roadmaps: try roadmap().waves, inventory: try roadmap().tasks.items, sessions: [unbound])
        let crumb = try #require(projection.breadcrumb(selection: nil, sessionId: unbound.id))
        #expect(projection.subject(for: unbound.id) == nil)
        #expect(crumb.waveWork == nil)
        #expect(crumb.taskWork == nil)
        #expect(crumb.siblings == [unbound])
    }

    @Test("Attention opens the exact retained conversation and Back returns to interrupted Work", arguments: [false, true])
    func attentionNavigation(alreadyRetained: Bool) async throws {
        let record = try session("needs-answer", work: .task(id: "ts_review00000000000000000000000000"))
        var wire = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(record)) as? [String: Any])
        wire["availability"] = "available"
        let source = try ReadingSource(roadmap: navigatorRoadmapJSON(),
            sessions: String(decoding: JSONSerialization.data(withJSONObject: [wire]), as: UTF8.self))
        let query = RegistryQuery { args, _ in try await source.read(args) }
        let model = WorkModel(query: query, repoPath: "/src/loopflow")
        await model.refresh()
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        model.bindWorkspaces(registry, repository: "repo")
        let rootKey = try #require(registry.selectedWork)
        let root = try #require(registry.retained(rootKey))
        let store = root.sessionStore(repoPath: "/src/loopflow", query: query)
        if alreadyRetained { registry.revealSession(record, in: root) }
        let existingPane = root.multiplexer.pane(forSessionId: record.id)
        model.select(.wave(id: "wave-2"))
        model.openSession(record, store: store)
        #expect(model.selection == (alreadyRetained ? nil : .task(id: "issue-review")))
        #expect(model.navigation.selectedSessionId == record.id)
        if alreadyRetained {
            #expect(root.multiplexer.pane(forSessionId: record.id)?.id == existingPane?.id)
        }
        model.goBack()
        #expect(model.selection == .wave(id: "wave-2"))
    }

    @Test("In-place navigation, resizing, collapse and history preserve Work and drafts")
    func navigatorContextReturn() async throws {
        let source = try ReadingSource(roadmap: navigatorRoadmapJSON(), sessions: sessionsJSON())
        let model = WorkModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refresh()
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        model.bindWorkspaces(registry, repository: "repo")
        model.select(.task(id: "issue-review"))
        let key = try #require(registry.selectedWork)
        let retained = try #require(registry.retained(key))
        let files = retained.files(issue: "W2-131", cwd: "/src/loopflow")
        let draft = files.document("notes.md")
        draft.editor.string = "keep this thought"
        retained.multiplexer.newShell(location: WorkspaceIdentity(machineId: "local", worktree: "/src/loopflow"))
        let pane = retained.multiplexer.focusedPane
        model.navigation.sidebarScrollOffset = 168
        model.navigation.resizeSidebar(460, availableWidth: 1200)
        model.select(nil)
        model.select(.wave(id: "wave-2"))
        #expect(model.selection == .wave(id: "wave-2"))
        model.goBack()
        #expect(model.selection == nil)
        model.goBack()
        #expect(model.selection == .task(id: "issue-review"))
        model.goForward()
        #expect(model.selection == nil)
        model.goBack()
        model.navigation.showsNavigator = true
        model.navigation.toggleNavigator()
        #expect(!model.navigation.showsNavigator)
        model.navigation.showsNavigator = true
        #expect(model.navigation.sidebarWidth == 460)
        #expect(model.navigation.sidebarScrollOffset == 168)
        #expect(registry.retained(key) === retained)
        #expect(retained.multiplexer.focusedPane == pane)
        #expect(files.document("notes.md") === draft)
        #expect(draft.editor.string == "keep this thought")
        model.select(.wave(id: "wave-1"))
        #expect(model.selection == .wave(id: "wave-1"))
        #expect(model.navigation.forwardStack.isEmpty)
    }

    @Test("Navigation selects one destination and retains each Task's panes")
    func singleDestinationRetainsWorkspaces() async throws {
        let model = try model()
        await model.refresh()
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        model.bindWorkspaces(registry, repository: "repo")
        let root = try #require(registry.selectedWork)
        model.select(.task(id: "issue-review"))
        let taskKey = try #require(registry.selectedWork)
        let entry = try #require(registry.retained(taskKey))
        _ = entry.multiplexer.newShell()
        let innerLayout = entry.multiplexer.layout
        #expect(registry.openWork == [taskKey])
        model.select(nil)
        #expect(registry.openWork == [root])
        model.select(.wave(id: "wave-1"))
        #expect(registry.openWork.count == 1)
        #expect(registry.selectedWork?.kind == .wave)
        model.goBack()
        #expect(registry.openWork == [root])
        model.goBack()
        #expect(registry.openWork == [taskKey])
        #expect(registry.retained(taskKey) === entry)
        #expect(entry.multiplexer.layout == innerLayout)
    }

    @Test("Standalone Tasks keep their workspace and draft across Wave moves")
    func wavelessTaskKeepsWorkspaceAcrossMove() async throws {
        var wire = try #require(JSONSerialization.jsonObject(with: Data(roadmapJSON().utf8)) as? [String: Any])
        var inventory = try #require(wire["tasks"] as? [String: Any])
        var task = try #require((inventory["items"] as? [[String: Any]])?.first)
        var summary = try #require(task["task"] as? [String: Any])
        let taskId = try #require(summary["id"] as? String)
        let projectId = summary["project_id"]
        let waveId = summary["wave_id"]
        summary["project_id"] = NSNull()
        summary["wave_id"] = NSNull()
        task["task"] = summary
        inventory["items"] = [task]
        wire["tasks"] = inventory
        let initial = String(decoding: try JSONSerialization.data(withJSONObject: wire), as: UTF8.self)
        let source = ReadingSource(roadmap: initial, sessions: try sessionsJSON())
        let model = WorkModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refresh()
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        model.bindWorkspaces(registry, repository: "repo")
        model.select(.task(id: taskId))
        let key = try #require(registry.selectedWork)
        let workspace = try #require(registry.retained(key))
        let draft = workspace.files(issue: "W2-131", cwd: "/src/loopflow").document("notes.md")
        draft.editor.string = "Keep this thought"
        workspace.multiplexer.newShell(location: workspace.location)
        let layout = workspace.multiplexer.layout
        #expect(model.visibleWork.standaloneTasks.map(\.task.id).contains(taskId))
        for attached in [true, false] {
            summary["project_id"] = attached ? projectId : NSNull()
            summary["wave_id"] = attached ? waveId : NSNull()
            task["task"] = summary
            inventory["items"] = [task]
            wire["tasks"] = inventory
            await source.replaceRoadmap(String(decoding: try JSONSerialization.data(withJSONObject: wire), as: UTF8.self))
            await model.refresh()
            #expect(model.selection == .task(id: taskId))
            #expect(registry.selectedWork == key)
            #expect(registry.retained(key) === workspace)
            #expect(workspace.multiplexer.layout == layout)
            #expect(draft.editor.string == "Keep this thought")
            #expect(model.visibleWork.standaloneTasks.map(\.task.id).contains(taskId) == !attached)
            #expect(model.task(id: taskId)?.task.task.waveId == (attached ? waveId as? String : nil))
        }
    }

    private func navigatorRoadmapJSON() throws -> String {
        var value = try #require(JSONSerialization.jsonObject(with: Data(roadmapJSON().utf8)) as? [String: Any])
        var waves = try #require(value["waves"] as? [[String: Any]])
        var second = try #require(waves[1]["wave"] as? [String: Any])
        second["repo"] = "/src/loopflow"
        waves[1]["wave"] = second
        value["waves"] = waves
        return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
    }

    @Test("The Machine control reads the current Work's Machine from planning; unstarted Tasks and the repository have none")
    func machineControl() async throws {
        let model = try model()
        await model.refresh()
        model.select(.task(id: "issue-review"))
        _ = try WorkBreadcrumbs(model: model).inspect().find(viewWithAccessibilityIdentifier: "work-machine")
        #expect(model.machineTarget == .task(issue: "W2-131"))
        #expect(model.machine(of: .task(issue: "W2-131")).label == "mini")
        model.select(.task(id: "issue-now"))
        #expect(model.machineTarget == nil)
        model.select(.wave(id: "wave-1"))
        #expect(model.machineTarget == .wave(id: "wave-1"))
        #expect(model.machine(of: .wave(id: "wave-1")).id == "home_00000000000000000000000000000001")
        model.select(nil)
        #expect(model.machineTarget == nil)
    }

    private func model() throws -> WorkModel {
        let source = try ReadingSource(roadmap: roadmapJSON(), sessions: sessionsJSON())
        return WorkModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
    }
    /// Working Tasks [review, now]; wave-1 [now]; Dashboard "Review" [now, later, review] and wave-1.
    private func dashboardModel(claimed: Bool = false) throws -> WorkModel {
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Viewing must not execute") }, repoPath: "/src/loopflow")
        model.applyFixture(roadmap: .available(try claimed ? claimedRoadmap() : roadmap()), waves: .available([]), workActivity: .loading, repos: [])
        model.bindWorkspaces(SessionsWorkspaceRegistry(localMachineId: fixtureMachineId), repository: "repo")
        func content(_ kind: WorkWorkspaceKey.Kind, _ id: String) -> WorkViewPlan.Content {
            fixtureViewContent(work: .init(repository: "repo", kind: kind, id: id), title: id)
        }
        let review = [content(.task, "issue-now"), content(.task, "issue-later"), content(.wave, "wave-1"),
                      content(.task, "issue-review")]
        let members = review.map { WorkMemberRef(source: ["root"], work: $0.work) }
        let hidden = WorkMemberRef(source: ["root"], work: .init(repository: "repo", kind: .task, id: "issue-available"))
        let dashboard = WorkViewPlan(name: "Review", pinned: true, dashboardID: "review", repository: "repo",
            selection: WorkSelection(id: "root", expression: .filter(.state(.open))), savedRevision: 1,
            resolution: WorkSelectionResolution(attention: [], members: members + [hidden], unavailable: []),
            arrangement: WorkArrangement(order: members + [hidden], hidden: [hidden], mode: .touched),
            projection: WorkArrangementProjection(visible: members, suppressed: []), slots: DashboardSlotLayout(root: .slot(0)),
            contents: review)
        model.applySelectionPlans([
            fixtureViewPlan(owner: .init(repository: "repo", kind: .repository, id: "repo"),
                            members: [content(.task, "issue-review"), content(.task, "issue-now")]),
            fixtureViewPlan(owner: .init(repository: "repo", kind: .wave, id: "wave-1"),
                            members: [content(.task, "issue-now")]),
            dashboard,
        ], reason: "")
        return model
    }
    private func roadmap() throws -> RoadmapSnapshot {
        try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(roadmapJSON().utf8))
    }
    /// The fixture with shipping issue-now claimed, as `lf ship` leaves it, and so in Rust's
    /// Autonomous section; unowned Tasks are backlog.
    private func claimedRoadmap() throws -> RoadmapSnapshot {
        var planning = try #require(JSONSerialization.jsonObject(with: Data(roadmapJSON().utf8)) as? [String: Any])
        var inventory = try #require(planning["tasks"] as? [String: Any])
        var tasks = try #require(inventory["items"] as? [[String: Any]])
        let index = try #require(tasks.firstIndex { ($0["task"] as? [String: Any])?["id"] as? String == "issue-now" })
        var runtime = try #require(tasks[index]["runtime"] as? [String: Any])
        runtime["owner"] = ["person": "Jack Heart", "person_key": "jack@loopflow.studio",
                            "machine": ["id": "home_7a3f0c2e9b1d4e5f8a6b0c1d2e3f4a5b", "label": "mini"]]
        tasks[index]["runtime"] = runtime
        tasks[index]["section"] = "autonomous"
        inventory["items"] = tasks
        planning["tasks"] = inventory
        return try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: planning))
    }
    private func roadmapJSON() throws -> String {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        return try String(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json"), encoding: .utf8)
    }
    private func sessionsJSON() throws -> String {
        let records = [try session("human", work: .task(id: "ts_review00000000000000000000000000"))]
        return String(decoding: try JSONEncoder().encode(records), as: UTF8.self)
    }
    private func session(_ id: String, work: WorkReference?, state: SessionState = .active, waveId: String? = nil) throws -> SessionRecord {
        let workData = try JSONEncoder().encode(work)
        let workJSON = String(decoding: workData, as: UTF8.self)
        let taskJSON = String(decoding: try JSONEncoder().encode(work?.kind == .task ? work?.id : nil), as: UTF8.self)
        let taskIds = work.flatMap { $0.kind == .task ? [$0.id] : nil } ?? []
        let taskIdsJSON = String(decoding: try JSONEncoder().encode(taskIds), as: UTF8.self)
        let waveJSON = String(decoding: try JSONEncoder().encode(waveId ?? (id == "project" ? "wave-1" : nil)), as: UTF8.self)
        return try JSONDecoder().decode(SessionRecord.self, from: Data("""
        {"id":"\(id)", "run_id": "\(id)", "interactive": true,"work":\(workJSON),"title":"\(id)",
         "workspace":{"machine_id":"local","worktree":"/src/loopflow","task_id":\(taskJSON),"unavailable":null},
         "detail":"codex","cwd":"/src/loopflow","state":"\(state.rawValue)",
         "wave_id":\(waveJSON),"work_path":null,"actions":\(sessionActionFixtureJSON(state: state.rawValue)),
         "ready_summary":null,"title_source":"generated","task_primary":false, "graph_membership":{"kind":"independent"},"task_ids": \(taskIdsJSON), "agent_process_id": "44444444-4444-4444-8444-444444444444", "terminal_ids":[],"open_argv":["lf","session","resume","\(id)"]}
        """.utf8))
    }
}

private actor PlanningGate {
    private var response: CheckedContinuation<Void, Never>?
    private var requested: CheckedContinuation<Void, Never>?
    func wait() async {
        await withCheckedContinuation { continuation in
            response = continuation
            requested?.resume()
            requested = nil
        }
    }
    func started() async {
        if response != nil { return }
        await withCheckedContinuation { requested = $0 }
    }
    func release() { response?.resume(); response = nil }
}

private actor ReadingSource {
    var roadmap: String
    var sessions: String
    var failed = false
    init(roadmap: String, sessions: String) { self.roadmap = roadmap; self.sessions = sessions }
    func fail() { failed = true }
    func replaceRoadmap(_ value: String) { roadmap = value }
    func replaceSessions(_ value: String) { sessions = value }
    func read(_ args: [String]) throws -> String {
        if failed { throw RegistryQueryError("offline") }
        switch args.first {
        case "desktop" where args.dropFirst().first == "plan": return roadmap
        case "desktop" where args.dropFirst().first == "sessions": return #"{"entries":\#(sessions),"next":null}"#
        case "wave" where args.dropFirst().first == "list": return "[]"
        case "history": return #"{"truncated":false,"items":[]}"#
        default: throw RegistryQueryError("unexpected command")
        }
    }
}
#endif
