#if os(macOS)
import AppKit
import Foundation
import Network
import Testing
import WebKit
#if canImport(GhosttyKit)
import GhosttyKit
#endif
@testable import Loopflow
@testable import LoopflowMac

extension DesktopActionRequest {
    init(_ action: String, at target: DesktopPaneTarget, args: [String: String] = [:]) {
        self.init(repository: target.repository, window: target.window, pane: target, action: action, args: args)
    }
}

@Suite("Exact Desktop pane arrangement", .serialized)
@MainActor
struct DesktopPaneControlTests {
    private let repository = "selected-plan"
    private let identity = WorkspaceIdentity(machineId: "machine-a", worktree: "/same/path")

    private var workKey: WorkWorkspaceKey { WorkWorkspaceKey(repository: repository, kind: .task, id: "task") }

    @Test("State uses the resolved repository plan identity, never its opening path")
    func stateUsesDurableRepositoryIdentity() async throws {
        let plan = "repo_11111111111111111111111111111111"
        let router = WorkLinkRouter()
        let query = RegistryQuery { _, _ in
            "{\"id\":\"\(plan)\",\"locators\":[\"\(plan)\"]}"
        }
        let path = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().path
        let scene = try await router.openRepository(path: path, link: nil, query: query, openWindow: { _ in })
        #expect(scene.id == plan)
        #expect(scene.path != plan)
        let registry = SessionsWorkspaceRegistry()
        let key = WorkWorkspaceKey(repository: scene.id, kind: .repository, id: scene.id)
        registry.open(key, title: "Repository", location: nil)
        router.register(UUID(), repository: scene.id, focus: {}, inspect: { repository, window in
            DesktopWindowInspection(repository: repository, window: window.uuidString, path: scene.path,
                destination: .work(WorkWorkspaceKey(repository: repository, kind: .repository, id: repository)), history: DesktopHistory(back: 0, forward: 0), reading: "current", reason: nil,
                task: nil, session: nil, list: [], actions: [], workspaces: registry.inspect(),
                openWork: registry.openWork, view: nil, viewParts: [], opening: nil)
        }, perform: { _ in throw RegistryQueryError("No mutation expected") },
        readText: { _ in throw RegistryQueryError("No terminal expected") }, receive: { _ in })
        let window = try #require(router.inspect().windows.first)
        #expect(window.repository == plan)
        #expect(window.openWork == [key])
        #expect(window.workspaces.first?.work.repository == plan)
        #expect(window.path != window.repository)
    }

    @Test("A Task's first conversation brings Files into view once; closing hides it and toggling returns it")
    func taskStartsWithRetainedCompanion() throws {
        let workspace = SessionsWorkspace(key: workKey, title: "Task", location: identity, localMachineId: identity.machineId)
        let store = workspace.multiplexer
        store.load(sessionId: "main", location: identity)
        let main = store.focusedPane
        workspace.prepareTaskPanes()
        let browser = try #require(store.browserPane)
        #expect(browser.content == .browser(taskId: "task", tabs: [.files], selected: "files", location: identity))
        #expect(store.isVisible(browser))
        #expect(workspace.browserPage == .files)
        #expect(store.focusedPane == main)
        store.updateRatio(between: main.id, and: browser.id, ratio: 0.6)
        workspace.showBrowser(.files, from: main)
        #expect(store.focusedPaneId == browser.id)
        #expect(store.layout.allPanes.count == 3)
        #expect(store.layout.pane(for: main.id) == main)
        let retained = store.layout
        workspace.prepareTaskPanes()
        #expect(store.layout == retained)
        store.close(browser.id)
        #expect(store.collapsedPaneIds.contains(browser.id))
        workspace.toggleBrowser()
        #expect(store.isVisible(try #require(store.browserPane)))
        #expect(store.browserPane?.content == browser.content)
    }

    @Test("Companion switches retain the shell and file draft; the next close retires a closed shell")
    func companionSwitchRetainsViews() throws {
        let workspace = SessionsWorkspace(key: workKey, title: "Task", location: identity, localMachineId: identity.machineId)
        let store = workspace.multiplexer
        store.load(sessionId: "main", location: identity)
        let main = store.focusedPane
        let shell = try #require(store.newShell(command: ["retained-shell"], location: identity))
        #if canImport(GhosttyKit)
        let terminal = try #require(shell.terminal)
        let view = workspace.surfaces.view(for: terminal)
        #endif
        let browser = try #require(workspace.showBrowser(.files, from: shell))
        let files = try workspace.files(for: browser.content, task: nil, query: RegistryQuery { _, _ in "{}" })
        files.selection = "draft.txt"
        files.document("draft.txt").editor.string = "retained draft"
        #expect(store.focusedPaneId == browser.id)
        #expect(store.layout.allPanes.count == 3)
        #if canImport(GhosttyKit)
        #expect(workspace.surfaces.view(for: terminal) === view)
        #endif
        store.setFocusedPane(shell.id)
        #expect(store.layout.pane(for: shell.id)?.content.selectedShell?.command == ["retained-shell"])
        workspace.showBrowser(.files, from: store.focusedPane)
        #expect(files.selection == "draft.txt")
        #expect(files.document("draft.txt").editor.string == "retained draft")
        #expect(store.layout.pane(for: main.id) == main)
        workspace.closePane(shell.id)
        #expect(store.shellPane?.content.selectedShell == nil)
        #expect(store.collapsedPaneIds.contains(shell.id))
        #if canImport(GhosttyKit)
        #expect(workspace.surfaces.view(for: terminal) === view)
        workspace.closePane(browser.id)
        #expect(workspace.surfaces.view(for: terminal) !== view)
        #endif
    }

    private func register(_ router: WorkLinkRouter, window: UUID, registry: SessionsWorkspaceRegistry, model: WorkModel = WorkModel(query: RegistryQuery { _, _ in
        throw RegistryQueryError("Pane controls must not launch Work")
    })) {
        router.register(window, repository: repository, focus: {},
            inspect: { repository, incarnation in
                DesktopWindowInspection(repository: repository, window: incarnation.uuidString, path: "/repo",
                    destination: .work(WorkWorkspaceKey(repository: "repo", kind: .repository, id: "repo")), history: DesktopHistory(back: 0, forward: 0), reading: "unavailable", reason: "Offline",
                    task: nil, session: nil, list: [], actions: [],
                    workspaces: registry.inspect(), openWork: registry.openWork, view: nil, viewParts: [], opening: nil)
            }, perform: { try DesktopAction.perform($0, model: model, workspaces: registry) }, readText: { try registry.readText($0) }) { _ in }
    }

    @Test("Navigation selects as the window's controls do; receipts match the next reading")
    func navigationRoundTripsThroughInspection() throws {
        let a = WorkWorkspaceKey(repository: repository, kind: .task, id: "a")
        let b = WorkWorkspaceKey(repository: repository, kind: .task, id: "b")
        let members = [a, b].map { WorkMemberRef(source: ["root"], work: $0) }
        func plan(dashboard: String?, expression: WorkSelection.Expression, also: [WorkWorkspaceKey] = []) -> WorkViewPlan {
            WorkViewPlan(name: dashboard, pinned: false, dashboardID: dashboard, repository: repository,
                selection: WorkSelection(id: "root", expression: expression), savedRevision: dashboard == nil ? nil : 1,
                resolution: WorkSelectionResolution(attention: [], members: members, unavailable: []),
                arrangement: WorkArrangement(order: members, hidden: [], mode: .touched),
                projection: WorkArrangementProjection(visible: members, suppressed: []), slots: DashboardSlotLayout(root: .slot(0)),
                contents: ([a, b] + also).map { fixtureViewContent(work: $0, title: $0.id) })
        }
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("No execution allowed") }, repoPath: "/repo")
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId)
        model.bindWorkspaces(registry, repository: repository)
        let wave = "33333333-3333-3333-3333-333333333333"
        let waveKey = WorkWorkspaceKey(repository: repository, kind: .wave, id: wave)
        model.applySelectionPlans([plan(dashboard: "review", expression: .filter(.state(.open))),
                                   plan(dashboard: nil, expression: .repository, also: [waveKey]),
                                   plan(dashboard: nil, expression: .wave(wave))], reason: "")
        let router = WorkLinkRouter()
        let window = UUID()
        router.register(window, repository: repository, focus: {},
            inspect: { model.inspectDesktop(repository: $0, window: $1, workspaces: registry) },
            perform: { try DesktopAction.perform($0, model: model, workspaces: registry) },
            readText: { _ in throw RegistryQueryError("No terminal expected") }) { _ in }
        func perform(_ action: String, _ args: [String: String] = [:]) throws -> DesktopActionReceipt {
            let receipt = try router.perform(DesktopActionRequest(repository: repository, window: window.uuidString,
                pane: nil, action: action, args: args))
            let state = try #require(router.inspect().windows.first)
            #expect(receipt.destination == state.destination)
            #expect(receipt.history == state.history)
            return receipt
        }
        model.selectDashboard("review")
        #expect(throws: DesktopTargetError.self) { try perform("enter", ["tile": "absent"]) }
        #expect(model.navigation.destination == .dashboard("review"))
        #expect(try perform("enter", ["tile": "b"]).destination == .entered(from: .dashboard("review"), work: b))
        let back = try perform("back")
        #expect(back.destination == .dashboard(id: "review"))
        #expect(back.history.forward == 1)
        #expect(try perform("forward").destination == .entered(from: .dashboard("review"), work: b))
        #expect(try perform("working-tasks").destination == .workingTasks)
        #expect(try perform("enter", ["tile": "a"]).destination == .entered(from: .workingTasks, work: a))
        // A sidebar Task on the open board keeps the board in the path, as its tile does.
        #expect(try perform("goto", ["index": "2"]).destination == .entered(from: .workingTasks, work: b))
        #expect(throws: DesktopTargetError.self) { try perform("goto", ["index": "3"]) }
        #expect(throws: DesktopArgumentError.self) { try perform("goto", ["index": "two"]) }
        #expect(throws: DesktopArgumentError.self) { try perform("goto") }
        #expect(throws: DesktopArgumentError.self) { try perform("goto", ["index": "1", "extra": "x"]) }
        #expect(throws: DesktopArgumentError.self) { try perform("navigate.sideways") }
        #expect(try perform("next").destination == .entered(from: .workingTasks, work: a))
        #expect(try perform("previous").destination == .entered(from: .workingTasks, work: b))
        // A Wave view's Tasks are tiles too; back returns to the Wave.
        model.select(.wave(id: wave))
        #expect(try perform("enter", ["tile": "b"]).destination == .entered(from: .wave(wave), work: b))
        #expect(try perform("back").destination == .work(waveKey))
        #expect(throws: DesktopTargetError.self) { try perform("enter", ["tile": "absent"]) }
        let state = try #require(router.inspect().windows.first)
        #expect(state.list.map(\.position) == [1, 2])
        #expect(state.list.map(\.work) == [a, b])
        #expect(state.actions.map(\.name) == DesktopAction.all.map(\.name))
        #expect(throws: DesktopTargetError.self) {
            try router.perform(DesktopActionRequest(repository: repository, window: UUID().uuidString, pane: nil, action: "back", args: [:]))
        }
    }

    @Test func actionRegistryMatchesTheFixture() throws {
        let data = try loadFixtureData("desktop_action.json")
        let wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let actions = try JSONDecoder().decode([DesktopActionDescriptor].self,
            from: JSONSerialization.data(withJSONObject: try #require(wire["actions"])))
        #expect(DesktopAction.all.map(\.descriptor) == actions)
    }

    /// Two identical windows: one acts by key, the other by Apple event.
    private func actionFixture() -> (WorkModel, SessionsWorkspaceRegistry, WorkLinkRouter, UUID) {
        let keys = ["a", "b"].map { WorkWorkspaceKey(repository: repository, kind: .task, id: $0) }
        let members = keys.map { WorkMemberRef(source: ["root"], work: $0) }
        let plan = WorkViewPlan(name: nil, pinned: false, dashboardID: nil, repository: repository,
            selection: WorkSelection(id: "root", expression: .repository), savedRevision: nil,
            resolution: WorkSelectionResolution(attention: [], members: members, unavailable: []),
            arrangement: WorkArrangement(order: members, hidden: [], mode: .touched),
            projection: WorkArrangementProjection(visible: members, suppressed: []), slots: DashboardSlotLayout(root: .slot(0)),
            contents: keys.map { fixtureViewContent(work: $0, title: $0.id) })
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("No execution allowed") }, repoPath: "/repo")
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId)
        model.bindWorkspaces(registry, repository: repository)
        model.applySelectionPlans([plan], reason: "")
        model.select(.task(id: "a"))
        model.select(.task(id: "b"))
        let workspace = registry.open(keys[1], title: "b", location: identity)
        workspace.multiplexer.load(sessionId: "conversation", location: identity)
        workspace.multiplexer.newShell(command: ["fixture-shell"], location: identity)
        workspace.multiplexer.setFocusedPane(workspace.multiplexer.sessionPane!.id)
        let router = WorkLinkRouter(), window = UUID()
        router.register(window, repository: repository, focus: {},
            inspect: { model.inspectDesktop(repository: $0, window: $1, workspaces: registry) },
            perform: { try DesktopAction.perform($0, model: model, workspaces: registry) },
            readText: { _ in throw RegistryQueryError("No terminal expected") }) { _ in }
        return (model, registry, router, window)
    }

    private struct ActionResult: Equatable {
        let destination: DesktopDestination
        let history: DesktopHistory
        let focused: String?, zoomed: String?, hidden: [String], order: [String]
        let shells: Int, browser: String?
        let failure: String?
    }

    private func result(_ model: WorkModel, _ registry: SessionsWorkspaceRegistry, failure: Error?) -> ActionResult {
        let store = registry.retained(WorkWorkspaceKey(repository: repository, kind: .task, id: "b"))?.multiplexer
        var shells = 0
        if case .shell(let tabs, _)? = store?.shellPane?.content { shells = tabs.count }
        return ActionResult(destination: model.desktopDestination(repository: repository), history: model.desktopHistory,
            focused: store?.focusedPaneId, zoomed: store?.zoomedPaneId, hidden: store.map { $0.collapsedPaneIds.sorted() } ?? [],
            order: store?.layout.allPanes.map(\.id) ?? [], shells: shells, browser: store?.browserPane?.content.selectedTabID,
            failure: failure.map { String(describing: type(of: $0)) })
    }

    @Test("Each action changes the model as its key does")
    func everyKeyBindingMatchesItsAppleEvent() throws {
        let bindings = DesktopAction.all.flatMap { action in action.keys.map { (action, $0) } }
        #expect(bindings.count >= 20)
        for (action, binding) in bindings {
            let (found, args) = try #require(DesktopAction.binding(characters: {
                if case .character(let character) = binding.key { return String(character) }
                return binding.key == .tab ? "\t" : nil
            }(), keyCode: binding.key == .return ? 36 : 0, specialKey: {
                switch binding.key {
                case .left: .leftArrow
                case .right: .rightArrow
                case .up: .upArrow
                case .down: .downArrow
                default: nil
                }
            }(), modifiers: binding.modifiers))
            #expect(found.name == action.name, "\(binding.display)")
            #expect(args == binding.args)

            let (keyModel, keyRegistry, _, _) = actionFixture()
            let workspace = try #require(keyRegistry.retained(WorkWorkspaceKey(repository: repository, kind: .task, id: "b")))
            let context = DesktopActionContext(model: keyModel, workspaces: keyRegistry, workspace: workspace,
                pane: workspace.multiplexer.focusedPane, args: args)
            guard action.available(context) else { continue }
            var keyFailure: Error?
            do { try action.perform(context) } catch { keyFailure = error }

            let (eventModel, eventRegistry, router, window) = actionFixture()
            let store = try #require(eventRegistry.retained(WorkWorkspaceKey(repository: repository, kind: .task, id: "b"))).multiplexer
            let pane = store.focusedPane
            let target = action.scope == .pane ? DesktopPaneTarget(repository: repository, window: window.uuidString,
                work: WorkWorkspaceKey(repository: repository, kind: .task, id: "b"), pane: pane.id,
                incarnation: pane.incarnation, tab: nil) : nil
            var eventFailure: Error?
            do {
                let receipt = try router.perform(DesktopActionRequest(repository: repository, window: window.uuidString,
                    pane: target, action: action.name, args: args))
                #expect(receipt.destination == eventModel.desktopDestination(repository: repository))
            } catch { eventFailure = error }
            #expect(result(keyModel, keyRegistry, failure: keyFailure) == result(eventModel, eventRegistry, failure: eventFailure),
                "\(action.name) \(binding.display)")
        }
    }

    private func target(_ pane: PaneState, window: UUID, machine: String = "machine-a") -> DesktopPaneTarget {
        DesktopPaneTarget(repository: repository, window: window.uuidString, work: WorkWorkspaceKey(repository: repository, kind: .task, id: machine == identity.machineId ? "task" : "other"), pane: pane.id, incarnation: pane.content.selectedShell?.incarnation ?? pane.incarnation, tab: pane.content.selectedShell?.id)
    }

    @Test("Session targets use captured location, never the enclosing workspace")
    func sessionLocationOwnsTargetingAndSplit() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId)
        let workspace = registry.open(workKey, title: "Task", location: identity)
        let peer = WorkspaceIdentity(machineId: "peer", worktree: identity.worktree)
        workspace.multiplexer.load(sessionId: "conversation", location: peer)
        let pane = workspace.multiplexer.focusedPane
        #expect(pane.terminal == .session("conversation", machineId: "peer"))
        let before = workspace.multiplexer.layout
        registry.reconcileMembership([])
        #expect(workspace.multiplexer.layout == before)
        let added = try #require(try workspace.newShell())
        #expect(added.content.location == identity)
        #expect(workspace.multiplexer.pane(forSessionId: "conversation") == pane)
        let router = WorkLinkRouter(), window = UUID()
        register(router, window: window, registry: registry)
        let stale = target(pane, window: window)
        _ = try router.perform(DesktopActionRequest("hide", at: stale))
        workspace.multiplexer.updateSessionLocation("conversation", location: identity)
        #expect(throws: DesktopTargetError.self) {
            try router.perform(DesktopActionRequest("hide", at: stale))
        }
        let split = try #require(try workspace.newShell())
        #expect(split.content.location == identity)
        workspace.multiplexer.updateSessionLocation("conversation", location: nil)
        #expect((try #require(workspace.multiplexer.pane(forSessionId: "conversation"))).terminal == nil)
    }

    @Test func literalTextDoesNotInterpretEscapesOrAcceptControlKeys() throws {
        for text in ["", "héλ🙂 e\u{301}", #""$(literal)"; \n"#] {
            try validateDesktopLiteralText(text)
        }
        for text in ["line\nsubmit", "\r", "\t", "\0", "\u{1b}[A", "\u{7f}", "\u{85}"] {
            #expect(throws: RegistryQueryError.self) { try validateDesktopLiteralText(text) }
        }
    }

    @Test func inputRequiresAnExistingTerminalAndNeverAllocatesOrFollowsFocus() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId)
        let router = WorkLinkRouter(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        store.showBrowser(.files)
        let companion = target(store.focusedPane, window: window)
        store.newShell(command: ["retained-command"], location: identity)
        let shell = target(store.focusedPane, window: window)
        store.load(sessionId: "focused-elsewhere", location: identity)
        store.setCollapsed(paneId: shell.pane, collapsed: true)
        register(router, window: window, registry: registry)
        let before = store.layout, focus = store.focusedPaneId
        for destination in [companion, shell] {
            for (action, args) in [("send", ["text": "draft", "surface": "missing"]),
                                   ("send-key", ["key": "enter", "surface": "missing"])] {
                #expect(throws: RegistryQueryError.self) {
                    try router.perform(DesktopActionRequest(action, at: destination, args: args))
                }
            }
        }
        #expect(store.layout == before)
        #expect(store.focusedPaneId == focus)
        #expect(store.collapsedPaneIds == [shell.pane])
        #expect(store.layout.pane(for: shell.pane)?.content.selectedShell?.command == ["retained-command"])
        #expect(registry.surfaces.programStatus(for: .shell(shell.pane, machineId: identity.machineId)) == nil)
    }

    @Test("Shell observation and exit use the pane's location, not the workspace path")
    func shellOwnsExecutionLocation() throws {
        let workspace = fixtureWork(fixtureWorkspace("/repository"))
        let location = WorkspaceIdentity(machineId: "peer", worktree: "/remote/task")
        let shell = try #require(workspace.multiplexer.newShell(command: ["peer-shell"], location: location))
        let terminal = TerminalIdentity.shell(try #require(shell.content.selectedShell).id, machineId: "peer")
        #expect(shell.terminal == terminal)
        NotificationCenter.default.post(name: .ghosttySurfaceClosed,
            object: TerminalIdentity.shell(try #require(shell.content.selectedShell).id, machineId: fixtureMachineId))
        #expect(workspace.multiplexer.layout.pane(for: shell.id) == shell)
        NotificationCenter.default.post(name: .ghosttySurfaceClosed, object: terminal)
        #expect(workspace.multiplexer.focusedPane.kind == .session)
        #expect(workspace.multiplexer.collapsedPaneIds.contains(shell.id))
        let unknown = try #require(workspace.multiplexer.newShell())
        #expect(unknown.terminal == nil)
    }

    @Test(arguments: [DesktopTextRegion.screen, .scrollback, .selection])
    func passiveReadKeepsMissingSurfaceDistinctAndDoesNotFollowFocus(region: DesktopTextRegion) throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId)
        let router = WorkLinkRouter(), window = UUID()
        let workspace = registry.open(workKey, title: "Task", location: identity), store = workspace.multiplexer
        store.newShell(command: ["retained-command"], location: identity)
        let shell = store.focusedPane
        let request = DesktopTextRequest(target: target(shell, window: window), surface: "old-surface",
                                         region: region, maxBytes: 16)
        store.load(sessionId: "other-session", location: identity)
        store.setCollapsed(paneId: shell.id, collapsed: true)
        let before = store.layout, focus = store.focusedPane
        register(router, window: window, registry: registry)
        let reading = try router.readText(request)
        #expect(reading.request == request)
        #expect(reading.result == .unavailable(reason: .missingSurface))
        #expect(reading.hidden)
        store.setCollapsed(paneId: shell.id, collapsed: false)
        #expect(try !router.readText(request).hidden)
        store.setZoom(focus.id, enabled: true)
        #expect(try router.readText(request).hidden)
        let inspection = try #require(registry.inspect().first)
        #expect(inspection.layout.children.first?.surface == nil)
        #expect(store.layout == before)
        #expect(store.focusedPane == focus)
        #expect(store.layout.pane(for: shell.id)?.content.selectedShell?.command == ["retained-command"])
        #expect(registry.surfaces.programStatus(for: try #require(shell.terminal)) == nil)
        #expect(registry.keys == [workKey])
    }

    @Test func readRejectsStaleWindowContentAndByteLimitsWithoutChangingWorkspace() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId)
        let router = WorkLinkRouter(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        let empty = store.focusedPane
        let request = DesktopTextRequest(target: target(empty, window: window), surface: "surface", region: .screen, maxBytes: 16)
        register(router, window: window, registry: registry)
        #expect(try router.readText(request).result == .unavailable(reason: .notTerminal))
        for bound in [0, -1, 1_048_577] {
            #expect(throws: RegistryQueryError.self) {
                try router.readText(.init(target: request.target, surface: request.surface, region: .screen, maxBytes: bound))
            }
        }
        store.load(sessionId: "replacement", location: identity)
        let before = store.layout
        #expect(throws: DesktopTargetError.self) { try router.readText(request) }
        let current = DesktopTextRequest(target: target(store.focusedPane, window: window), surface: "surface", region: .screen, maxBytes: 16)
        register(router, window: UUID(), registry: registry)
        #expect(throws: DesktopTargetError.self) { try router.readText(current) }
        #expect(store.layout == before)
    }

    #if canImport(GhosttyKit)
    @Test(.requiresDisplay) func exactInputCombinesDraftAtCursorWithoutFollowingFocusOrSubmitting() async throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let result = directory.appendingPathComponent("submitted")
        func quote(_ value: String) -> String { "'" + value.replacingOccurrences(of: "'", with: "'\\''") + "'" }
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId)
        let router = WorkLinkRouter(), windowID = UUID()
        let workspace = registry.open(workKey, title: "Task", location: identity), store = workspace.multiplexer
        store.newShell(location: identity)
        let pane = store.focusedPane
        store.load(sessionId: "other", location: identity)
        let otherPane = store.focusedPane
        let terminal = try #require(pane.terminal)
        let other = try #require(otherPane.terminal)
        let view = registry.surfaces.view(for: terminal), otherView = registry.surfaces.view(for: other)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 800, height: 300),
                              styleMask: [.titled], backing: .buffered, defer: false)
        let root = NSView(frame: window.contentLayoutRect)
        window.contentView = root
        for (index, view) in [view, otherView].enumerated() {
            view.frame = CGRect(x: CGFloat(index * 400), y: 0, width: 400, height: 300)
            root.addSubview(view)
            view.workingDirectory = directory.path
            view.command = "/usr/bin/env -i HOME=\(quote(directory.path)) PATH=/usr/bin:/bin TERM=xterm-256color PS1=fixture-ready /bin/zsh -df"
            view.createSurface(manager: manager)
            _ = try #require(view.surface)
        }
        defer {
            registry.surfaces.release(terminal)
            registry.surfaces.release(other)
            window.contentView = nil
        }
        let token = try #require(registry.surfaces.surfaceIncarnation(for: terminal))
        let destination = target(pane, window: windowID)
        register(router, window: windowID, registry: registry)
        func text(_ pane: PaneState) throws -> String {
            let terminal = try #require(pane.terminal)
            let surface = try #require(registry.surfaces.surfaceIncarnation(for: terminal))
            let result = try router.readText(.init(target: target(pane, window: windowID),
                surface: surface, region: .screen, maxBytes: 65536)).result
            guard case .available(let text, _) = result else {
                throw RegistryQueryError("Fixture terminal output is unavailable")
            }
            return text
        }
        let deadline = ContinuousClock.now + .seconds(5)
        while try !text(pane).contains("fixture-ready")
            || !text(otherPane).contains("fixture-ready") {
            try #require(ContinuousClock.now < deadline)
            try await Task.sleep(for: .milliseconds(20))
        }
        func send(text: String) throws {
            _ = try router.perform(DesktopActionRequest("send", at: destination, args: ["text": text, "surface": token]))
        }
        func send(key: DesktopKey) throws {
            _ = try router.perform(DesktopActionRequest("send-key", at: destination, args: ["key": key.rawValue, "surface": token]))
        }
        try send(text: "printf '%s' 'drft'")
        for _ in 0..<3 { try send(key: .left) }
        #expect(window.makeFirstResponder(otherView))
        store.setCollapsed(paneId: pane.id, collapsed: true)
        let before = store.layout
        view.setMarkedText("unfinished", selectedRange: NSRange(location: 10, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        #expect(throws: RegistryQueryError.self) { try send(text: "a") }
        #expect(throws: RegistryQueryError.self) { try send(key: .enter) }
        #expect(view.hasMarkedText())
        view.unmarkText()
        try send(text: "a")
        try send(key: .end)
        try send(text: " > " + quote(result.path))
        let draftDeadline = ContinuousClock.now + .seconds(3)
        while try !text(pane).contains("submitted") {
            try #require(ContinuousClock.now < draftDeadline)
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(!FileManager.default.fileExists(atPath: result.path))
        try send(key: .enter)
        let submitDeadline = ContinuousClock.now + .seconds(3)
        while (try? String(contentsOf: result, encoding: .utf8)) != "draft" {
            try #require(ContinuousClock.now < submitDeadline)
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(try !text(otherPane).contains("printf"))
        #expect(store.layout == before)
        #expect(store.focusedPaneId == otherPane.id)
        #expect(window.firstResponder === otherView)
        #expect(registry.surfaces.surfaceIncarnation(for: terminal) == token)
    }

    @Test(.requiresDisplay) func boundedReadPreservesRegionsSelectionAndExitedSurfaceUntilReplacement() throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId)
        let router = WorkLinkRouter(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        store.newShell(location: identity)
        let pane = store.focusedPane
        let terminal = try #require(pane.terminal)
        let view = registry.surfaces.view(for: terminal)
        view.setFrameSize(CGSize(width: 400, height: 300))
        view.workingDirectory = NSTemporaryDirectory()
        // Leave the Unicode prefix in scrollback, away from the viewport.
        view.command = "/bin/sh -c 'printf aé界😀z; i=0; while [ $i -lt 100 ]; do printf \"\\r\\nline\"; i=$((i+1)); done; printf \"\\r\\nretained\"'"
        view.createSurface(manager: manager)
        defer { registry.surfaces.release(terminal) }
        let surface = try #require(view.surface)
        let token = try #require(registry.surfaces.surfaceIncarnation(for: terminal))
        register(router, window: window, registry: registry)
        store.newShell(location: identity)
        store.setCollapsed(paneId: pane.id, collapsed: true)
        let before = store.layout, focus = store.focusedPaneId
        // Observe the exited-but-retained surface before queued lifecycle cleanup.
        let deadline = Date().addingTimeInterval(3)
        while !ghostty_surface_process_exited(surface), Date() < deadline { Thread.sleep(forTimeInterval: 0.01) }
        try #require(ghostty_surface_process_exited(surface))
        func read(_ region: DesktopTextRegion, maxBytes: Int) throws -> DesktopTextResult {
            let reading = try router.readText(.init(target: target(pane, window: window), surface: token,
                                                   region: region, maxBytes: maxBytes))
            #expect(reading.hidden)
            return reading.result
        }
        #expect(try read(.selection, maxBytes: 1) == .available(text: "", truncated: false))
        let prefix = "aé界😀z"
        for limit in 1...prefix.utf8.count {
            var bytes = Array(prefix.utf8.prefix(limit))
            while String(bytes: bytes, encoding: .utf8) == nil { bytes.removeLast() }
            #expect(try read(.scrollback, maxBytes: limit) == .available(
                text: String(decoding: bytes, as: UTF8.self), truncated: true))
        }
        let result = try read(.screen, maxBytes: 65536)
        guard case .available(let screen, let truncated) = result else {
            throw RegistryQueryError("Fixture viewport is unavailable")
        }
        #expect(screen.contains("retained"))
        #expect(!screen.contains(prefix))
        #expect(!truncated)
        let action = "select_all"
        try #require(action.withCString { ghostty_surface_binding_action(surface, $0, UInt(action.utf8.count)) })
        let selected = try read(.selection, maxBytes: 65536)
        #expect(selected == (try read(.scrollback, maxBytes: 65536)))
        _ = try read(.screen, maxBytes: 1)
        _ = try read(.scrollback, maxBytes: 1)
        #expect(try read(.selection, maxBytes: 65536) == selected)
        #expect(store.layout == before)
        #expect(store.focusedPaneId == focus)
        #expect(registry.surfaces.surfaceIncarnation(for: terminal) == token)
        #expect(view.surface == surface)
        let request = DesktopTextRequest(target: target(pane, window: window), surface: token,
                                         region: .screen, maxBytes: 65536)
        for (action, args) in [("send", ["text": "must not reopen", "surface": token]), ("send-key", ["key": "enter", "surface": token])] {
            #expect(throws: RegistryQueryError.self) { try router.perform(DesktopActionRequest(action, at: request.target, args: args)) }
        }
        #expect(view.surface == surface)
        #expect(registry.surfaces.surfaceIncarnation(for: terminal) == token)
        registry.surfaces.release(terminal)
        #expect(try router.readText(request).result == .unavailable(reason: .missingSurface))
        let replacement = registry.surfaces.view(for: terminal)
        replacement.setFrameSize(CGSize(width: 400, height: 300))
        replacement.workingDirectory = NSTemporaryDirectory()
        replacement.command = "/bin/cat"
        replacement.createSurface(manager: manager)
        _ = try #require(replacement.surface)
        #expect(throws: RegistryQueryError.self) { try router.readText(request) }
        for (action, args) in [("send", ["text": "stale", "surface": token]), ("send-key", ["key": "enter", "surface": token])] {
            #expect(throws: RegistryQueryError.self) { try router.perform(DesktopActionRequest(action, at: request.target, args: args)) }
        }
        #expect(replacement.surface != nil)
    }

    @Test func sameTerminalIdOnAnotherMachineKeepsItsOwnViewAndLifetime() {
        let pool = GhosttySurfacePool()
        let local = TerminalIdentity.session("same-session", machineId: "local")
        let peer = TerminalIdentity.session("same-session", machineId: "peer")
        #expect(pool.surfaceIncarnation(for: local) == nil)
        #expect(pool.programStatus(for: local) == nil)
        let first = pool.view(for: local)
        let other = pool.view(for: peer)
        #expect(first !== other)
        #expect(first.programStatus.incarnation != other.programStatus.incarnation)
        // A retained view without a native surface must not advertise one.
        #expect(pool.surfaceIncarnation(for: local) == nil)
        pool.release(local)
        #expect(pool.programStatus(for: local) == nil)
        #expect(pool.programStatus(for: peer) === other.programStatus)
        let replacement = pool.view(for: local)
        #expect(replacement.programStatus.incarnation != first.programStatus.incarnation)
        pool.discard(first)
        #expect(pool.programStatus(for: local) === replacement.programStatus)
        pool.release(local)
        pool.release(peer)
    }
    #endif

    @Test func companionsRetainDraftsAndDoNotFollowFocusOrReplaceContent() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from:
            Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let model = WorkModel(query: RegistryQuery { _, _ in
            throw RegistryQueryError("Companion creation must not read or launch Work")
        }, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        model.navigation.preparedTaskWorktrees[task.id] = identity
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId), router = WorkLinkRouter(), window = UUID()
        let workKey = WorkWorkspaceKey(repository: repository, kind: .task, id: task.runtime?.workId ?? task.id)
        let workspace = registry.open(workKey, title: "Task", location: identity), store = workspace.multiplexer
        store.load(sessionId: "retained-session", location: identity)
        let session = store.focusedPane
        let source = DesktopPaneTarget(repository: repository, window: window.uuidString, work: workKey, pane: session.id, incarnation: session.incarnation, tab: nil)
        store.newShell(command: ["retained-shell-command"], location: identity)
        let selected = store.focusedPane
        store.setZoom(selected.id, enabled: true)
        register(router, window: window, registry: registry, model: model)
        let document = workspace.files(issue: task.id, cwd: identity.worktree).document("note.txt")
        document.editor.string = "unfinished draft"
        document.editor.setSelectedRange(NSRange(location: 2, length: 4))
        for action in ["files", "graph-log"] {
            let created = try router.perform(DesktopActionRequest(action, at: source))
            let reused = try router.perform(DesktopActionRequest(action, at: source))
            #expect(created.pane == reused.pane)
            #expect(created.pane != source.pane)
            #expect(store.layout.pane(for: try #require(created.pane)) != nil)
            #expect(created.workspace?.work == workKey)
        }
        #expect(store.layout.allPanes.count == 3)
        let files = try #require(store.browserPane)
        guard case .browser(_, let tabs, _, _, _) = files.content else { Issue.record("Missing browser"); return }
        #expect(tabs == [.files, .graphLog])
        #expect(files.content.location == identity)
        store.setCollapsed(paneId: files.id, collapsed: true)
        _ = try router.perform(DesktopActionRequest("files", at: source))
        #expect(!store.collapsedPaneIds.contains(files.id))
        #expect(store.browserPane?.incarnation == files.incarnation)
        #expect(store.browserPane?.content.selectedBrowserTab == .files)
        let shellReceipt = try router.perform(DesktopActionRequest("run", at: source))
        #expect(store.shell(shellReceipt.tab ?? "")?.location == identity)
        #expect(shellReceipt.pane == selected.id)
        #expect(store.layout.allPanes.count == 3)
        #expect(store.layout.pane(for: session.id) == session)
        #expect(store.focusedPaneId == selected.id)
        #expect(store.zoomedPaneId == selected.id)
        #expect(store.shell(try #require(selected.content.selectedShell).id)?.command == ["retained-shell-command"])
        #expect(document.editor.string == "unfinished draft")
        #expect(document.editor.selectedRange() == NSRange(location: 2, length: 4))
        #expect(model.selection == nil)
        #expect(throws: DesktopArgumentError.self) {
            try router.perform(DesktopActionRequest("browse", at: source, args: ["url": "file:///etc/passwd"]))
        }
        let page = try router.perform(DesktopActionRequest("browse", at: source, args: ["url": "http://localhost:5173"]))
        #expect(store.browserPane?.content.selectedBrowserTab == .url(id: try #require(page.tab), url: try #require(URL(string: "http://localhost:5173"))))
        #expect(store.focusedPaneId == selected.id)
    }

    @Test("Web page Back, Forward and Reload act on the browser tab's retained page")
    func webPageActionsDriveTheRetainedPage() async throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId), router = WorkLinkRouter(), window = UUID()
        let workspace = registry.open(workKey, title: "Task", location: identity), store = workspace.multiplexer
        store.load(sessionId: "main", location: identity)
        let source = target(store.focusedPane, window: window)
        register(router, window: window, registry: registry)
        _ = try router.perform(DesktopActionRequest("files", at: source))
        let files = try #require(store.browserPane)
        #expect(throws: DesktopTargetError.self) { try router.perform(DesktopActionRequest("reload", at: target(files, window: window))) }

        let server = try await LoopbackPages()
        defer { server.listener.cancel() }
        let first = server.url("one"), second = server.url("two")
        let opened = try router.perform(DesktopActionRequest("browse", at: source, args: ["url": first.absoluteString]))
        let browser = try #require(store.browserPane)
        let tab = try #require(opened.tab)
        let page = workspace.pages.view(for: tab, url: first)
        #expect(workspace.pages.view(for: tab, url: first) === page)
        let at = target(browser, window: window)
        try await settle(page) { $0.url == first && $0.title?.isEmpty == false }
        let loaded = page.title
        page.load(URLRequest(url: second))
        try await settle(page) { $0.url == second && $0.canGoBack }
        #expect(throws: DesktopTargetError.self) { try router.perform(DesktopActionRequest("page-forward", at: at)) }

        _ = try router.perform(DesktopActionRequest("page-back", at: at))
        try await settle(page) { $0.url == first && $0.canGoForward }
        _ = try router.perform(DesktopActionRequest("reload", at: at))
        try await settle(page) { $0.url == first && $0.title?.isEmpty == false && $0.title != loaded }
        _ = try router.perform(DesktopActionRequest("page-forward", at: at))
        try await settle(page) { $0.url == second }

        workspace.closePane(browser.id)
        #expect(workspace.pages.view(for: tab, url: second) === page)
        workspace.closePane(browser.id) // Closing Files replaces the undoable page.
        #expect(workspace.pages.view(for: tab, url: second) !== page)
    }

    /// Wait for WebKit to finish the navigation the condition describes.
    private func settle(_ page: WKWebView, _ condition: (WKWebView) -> Bool) async throws {
        for _ in 0..<200 where page.isLoading || !condition(page) { try await Task.sleep(for: .milliseconds(25)) }
        #expect(!page.isLoading && condition(page), "\(String(describing: page.url)) \(page.title ?? "")")
    }

    @Test func remoteShellRefusalPreservesTheWorkspace() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: "other"), router = WorkLinkRouter(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        let before = store.layout
        register(router, window: window, registry: registry)
        #expect(throws: RegistryQueryError.self) {
            try router.perform(DesktopActionRequest("run", at: target(store.focusedPane, window: window)))
        }
        #expect(store.layout == before)
        #expect(store.shellPane?.content.selectedShell == nil)
    }

    @Test func hideRestoreRetainsContentAndIgnoresLaterFocus() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), window = UUID()
        let workspace = registry.open(workKey, title: "Task", location: identity), store = workspace.multiplexer
        store.newShell(command: ["fixture-shell"], location: identity)
        let shell = store.focusedPane
        store.load(sessionId: "retained-draft", location: identity)
        let session = store.focusedPane
        let layout = store.layout
        register(router, window: window, registry: registry)
        let request = target(shell, window: window)

        for _ in 0..<2 { // Repeating after a lost reply is idempotent.
            let report = try router.perform(DesktopActionRequest("hide", at: request))
            #expect(report.pane == shell.id)
            #expect(router.inspect().windows[0].workspaces[0].hiddenPanes == ["browser", shell.id])
            #expect(store.focusedPane == session)
        }
        let report = try router.perform(DesktopActionRequest("show", at: request))
        #expect(report.pane == shell.id)
        #expect(router.inspect().windows[0].workspaces[0].hiddenPanes == ["browser"])
        #expect(store.layout == layout)
        #expect(store.focusedPane == session)
        #expect(store.layout.pane(for: shell.id)?.content.selectedShell?.command == ["fixture-shell"])
        #expect(registry.open(workKey, title: "Task", location: identity) === workspace)
        #expect(workspace.surfaces === registry.surfaces)
        #expect(!store.canUndoClose)
    }

    @Test func arrangementRetainsDocumentsSurfacesSelectionAndContentOccurrences() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId), window = UUID()
        let workspace = registry.open(workKey, title: "Task", location: identity), store = workspace.multiplexer
        store.newShell(command: ["fixture-shell"], location: identity)
        let shell = store.focusedPane
        store.load(sessionId: "retained-session", location: identity)
        let session = store.focusedPane
        store.showBrowser(.files)
        let filesPane = store.focusedPane
        let document = workspace.files(issue: "TASK", cwd: identity.worktree).document("note.txt")
        document.editor.string = "unfinished draft"
        document.editor.setSelectedRange(NSRange(location: 3, length: 5))
        #if GHOSTTY_ENABLED
        let shellView = registry.surfaces.view(for: try #require(shell.terminal))
        let sessionView = registry.surfaces.view(for: .session("retained-session", machineId: identity.machineId))
        #endif
        register(router, window: window, registry: registry)
        let shellTarget = target(shell, window: window), sessionTarget = target(session, window: window)
        // Later focus must not redirect any of these requests.
        store.setFocusedPane(filesPane.id)
        _ = try router.perform(DesktopActionRequest("move", at: shellTarget, args: ["destination": sessionTarget.pane, "axis": "vertical"]))
        #expect(store.layout.allPanes.map(\.id) == [session.id, shell.id, filesPane.id])
        _ = try router.perform(DesktopActionRequest("resize", at: shellTarget, args: ["toward": sessionTarget.pane, "ratio": String(0.7)]))
        let reading = DesktopLayoutInspection(store.layout, surface: { _ in nil }, commands: { _ in nil })
        let movedSplit = reading.children[0]
        #expect(movedSplit.axis == "vertical")
        #expect(abs(try #require(movedSplit.ratio) - 0.3) < 0.0001)
        for _ in 0..<2 {
            _ = try router.perform(DesktopActionRequest("zoom", at: shellTarget, args: ["on": String(true)]))
            #expect(store.zoomedPaneId == shell.id)
            #expect(store.focusedPane == filesPane)
        }
        _ = try router.perform(DesktopActionRequest("zoom", at: sessionTarget, args: ["on": String(false)]))
        #expect(store.zoomedPaneId == shell.id) // Do not unzoom a different target.
        _ = try router.perform(DesktopActionRequest("zoom", at: shellTarget, args: ["on": String(false)]))
        #expect(store.zoomedPaneId == nil)
        #expect(store.focusedPane == filesPane)
        #expect(store.layout.pane(for: shell.id) == shell)
        #expect(store.layout.pane(for: session.id) == session)
        #expect(store.layout.pane(for: shell.id)?.content.selectedShell?.command == ["fixture-shell"])
        #expect(document.editor.string == "unfinished draft")
        #expect(document.editor.selectedRange() == NSRange(location: 3, length: 5))
        #expect(registry.open(workKey, title: "Task", location: identity) === workspace)
        #expect(workspace.surfaces === registry.surfaces)
        #if GHOSTTY_ENABLED
        #expect(registry.surfaces.view(for: try #require(shell.terminal)) === shellView)
        #expect(registry.surfaces.view(for: .session("retained-session", machineId: identity.machineId)) === sessionView)
        #endif
        _ = try router.perform(DesktopActionRequest("focus", at: shellTarget))
        #expect(store.focusedPane == shell)
    }

    @Test func staleSourceRejectsEveryActionWithoutChangingTheCurrentFocus() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        store.load(sessionId: "old", location: identity)
        let old = target(store.focusedPane, window: window)
        store.load(sessionId: "replacement", location: identity)
        store.newShell(location: identity)
        let peer = target(store.focusedPane, window: window)
        register(router, window: window, registry: registry)
        let before = store.layout, focus = store.focusedPaneId, hidden = store.collapsedPaneIds
        let actions: [(String, [String: String])] = [("hide", [:]), ("show", [:]), ("focus", [:]),
            ("move", ["destination": peer.pane, "axis": "vertical"]), ("resize", ["toward": peer.pane, "ratio": "0.6"]),
            ("zoom", ["on": "true"]), ("run", [:]), ("files", [:]), ("graph-log", [:]),
            ("send", ["text": "draft", "surface": "stale"]), ("send-key", ["key": "enter", "surface": "stale"])]
        for (action, args) in actions {
            #expect(throws: DesktopTargetError.self) { try router.perform(DesktopActionRequest(action, at: old, args: args)) }
            #expect(store.layout == before)
            #expect(store.focusedPaneId == focus)
            #expect(store.zoomedPaneId == nil)
            #expect(store.collapsedPaneIds == hidden)
        }
    }

    @Test func invalidSecondTargetsAndRatiosLeaveBothPanesUntouched() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        store.newShell(location: identity)
        let shell = target(store.focusedPane, window: window)
        store.load(sessionId: "old", location: identity)
        let stale = target(store.focusedPane, window: window)
        store.load(sessionId: "new", location: identity)
        let peer = target(store.focusedPane, window: window)
        register(router, window: window, registry: registry)
        let before = store.layout
        _ = stale
        for destination in [shell.pane, "absent"] {
            for (action, args) in [("move", ["destination": destination, "axis": "vertical"]),
                                   ("resize", ["toward": destination, "ratio": "0.6"])] {
                #expect(throws: DesktopTargetError.self) { try router.perform(DesktopActionRequest(action, at: shell, args: args)) }
                #expect(store.layout == before)
                #expect(store.focusedPaneId == peer.pane)
            }
        }
        let outside = DesktopPaneTarget(repository: "different-plan", window: window.uuidString,
            work: workKey, pane: peer.pane, incarnation: peer.incarnation, tab: nil)
        #expect(throws: DesktopTargetError.self) { try router.perform(DesktopActionRequest("hide", at: outside)) }
        for ratio in [Double.nan, .infinity, 0, 1] {
            #expect(throws: DesktopArgumentError.self) {
                try router.perform(DesktopActionRequest("resize", at: shell, args: ["toward": peer.pane, "ratio": String(ratio)]))
            }
            #expect(store.layout == before)
        }
        #expect(registry.keys == [workKey])
    }

    @Test func focusingAHiddenPaneRevealsItWithoutChangingItsContent() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        store.newShell(location: identity)
        let pane = store.focusedPane, request = target(pane, window: window)
        register(router, window: window, registry: registry)
        _ = try router.perform(DesktopActionRequest("hide", at: request))
        #expect(!store.isVisible(pane))
        _ = try router.perform(DesktopActionRequest("focus", at: request))
        #expect(store.isVisible(pane))
        #expect(store.focusedPane == pane)
    }

    @Test func samePaneAndSessionAfterReplacementDoesNotReviveOldTarget() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        store.load(sessionId: "a", location: identity)
        let old = target(store.focusedPane, window: window)
        register(router, window: window, registry: registry)
        store.load(sessionId: "b", location: identity)
        store.load(sessionId: "a", location: identity)
        #expect(store.focusedPane.id == old.pane)
        #expect(store.focusedPane.incarnation != old.incarnation)
        #expect(throws: DesktopTargetError.self) {
            try router.perform(DesktopActionRequest("hide", at: old))
        }
        #expect(!store.collapsedPaneIds.contains(old.pane))
    }

    @Test("⌘W then ⇧⌘T returns the same shell tab and surface, by key and by Apple event")
    func undoCloseReturnsTheClosedShellTab() throws {
        for byKey in [true, false] {
            let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId), window = UUID()
            let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Pane controls must not launch Work") })
            let workspace = registry.open(workKey, title: "Task", location: identity), store = workspace.multiplexer
            store.load(sessionId: "conversation", location: identity)
            let first = try #require(store.newShell(location: identity)?.content.selectedShell)
            let shell = try #require(store.newShell(location: identity))
            let closing = try #require(shell.content.selectedShell)
            #if canImport(GhosttyKit)
            let view = registry.surfaces.view(for: try #require(shell.terminal))
            #endif
            register(router, window: window, registry: registry, model: model)
            func press(_ key: String, _ modifiers: NSEvent.ModifierFlags) throws -> (pane: String?, tab: String?) {
                let (action, args) = try #require(DesktopAction.binding(characters: key, modifiers: modifiers))
                if byKey {
                    let outcome = try action.perform(DesktopActionContext(model: model, workspaces: registry,
                        workspace: workspace, pane: store.focusedPane, args: args))
                    return (outcome.pane, outcome.tab)
                }
                let receipt = try router.perform(DesktopActionRequest(action.name, at: target(store.focusedPane, window: window), args: args))
                return (receipt.pane, receipt.tab)
            }

            _ = try press("w", .command)
            #expect(store.shellPane?.content.selectedShell == first)
            #expect(store.isVisible(try #require(store.shellPane)))
            let reopened = try press("t", [.command, .shift])
            #expect(reopened.pane == shell.id)
            #expect(reopened.tab == closing.id)
            #expect(store.focusedPane == shell)
            #if canImport(GhosttyKit)
            #expect(registry.surfaces.view(for: try #require(shell.terminal)) === view)
            #endif
            #expect(throws: DesktopTargetError.self) { _ = try press("t", [.command, .shift]) }
        }
    }

    @Test("close closes the tab it names, not the selected one")
    func closeActsOnTheAddressedTab() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        let first = try #require(store.newShell(location: identity)?.content.selectedShell)
        let second = try #require(store.newShell(location: identity)?.content.selectedShell)
        store.selectShell(first.id)
        let pane = try #require(store.shellPane)
        register(router, window: window, registry: registry)
        let addressed = DesktopPaneTarget(repository: repository, window: window.uuidString, work: workKey,
            pane: pane.id, incarnation: second.incarnation, tab: second.id)
        _ = try router.perform(DesktopActionRequest("close", at: addressed))
        guard case .shell(let tabs, let selected) = store.shellPane?.content else { throw RegistryQueryError("No shell pane") }
        #expect(tabs == [first])
        #expect(selected == first.id)
    }

    @Test func wrongMachineAndReplacedWindowNeverChangeOrAllocateWorkspaces() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), old = UUID(), replacement = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        store.newShell(location: identity)
        let pane = store.focusedPane
        register(router, window: old, registry: registry)
        #expect(throws: DesktopTargetError.self) {
            try router.perform(DesktopActionRequest("hide", at: target(pane, window: old, machine: "machine-b")))
        }
        #expect(registry.keys == [workKey])
        register(router, window: replacement, registry: registry)
        router.remove(old, repository: repository)
        #expect(throws: DesktopTargetError.self) {
            try router.perform(DesktopActionRequest("hide", at: target(pane, window: old)))
        }
        #expect(!store.collapsedPaneIds.contains(pane.id))
        _ = try router.perform(DesktopActionRequest("hide", at: target(pane, window: replacement)))
        #expect(store.collapsedPaneIds.contains(pane.id))
        router.remove(replacement, repository: repository)
        #expect(throws: DesktopTargetError.self) {
            try router.perform(DesktopActionRequest("show", at: target(pane, window: replacement)))
        }
        #expect(store.collapsedPaneIds.contains(pane.id))
    }
}
/// Loopback pages titled with their load time, so a reload retitles the page.
private final class LoopbackPages: @unchecked Sendable {
    let listener: NWListener

    init() async throws {
        listener = try NWListener(using: .tcp, on: .any)
        let html = "<script>document.title = performance.timeOrigin</script>"
        let response = Data("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nCache-Control: no-store\r\nContent-Length: \(html.utf8.count)\r\nConnection: close\r\n\r\n\(html)".utf8)
        listener.newConnectionHandler = { connection in
            connection.start(queue: .global())
            connection.receive(minimumIncompleteLength: 1, maximumLength: 65536) { _, _, _, _ in
                connection.send(content: response, completion: .contentProcessed { _ in connection.cancel() })
            }
        }
        try await withCheckedThrowingContinuation { (ready: CheckedContinuation<Void, Error>) in
            listener.stateUpdateHandler = { state in
                switch state {
                case .ready: ready.resume()
                case .failed(let error): ready.resume(throwing: error)
                default: break
                }
            }
            listener.start(queue: .global())
        }
        listener.stateUpdateHandler = nil
    }

    func url(_ path: String) -> URL {
        URL(string: "http://127.0.0.1:\(listener.port?.rawValue ?? 0)/\(path)")!
    }
}
#endif
