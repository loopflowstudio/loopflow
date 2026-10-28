#if os(macOS)
import AppKit
import Combine
import Loopflow
import OSLog
import SwiftUI

extension Notification.Name {
    static let openSessions = Notification.Name("loopflow.openSessions")
}

/// Window chrome stays here; every Work gesture is a registry action.
struct ConsoleCommandActions {
    let navigate: () -> Void
    let toggleSidebar: () -> Void
    let focusSidebar: () -> Void
    let focusTerminal: (() -> Void)?
    /// The focused window's action context, read when a menu item is chosen.
    let context: @MainActor () -> DesktopActionContext
    let failed: @MainActor (Error) -> Void
}

private struct ConsoleCommandsKey: FocusedValueKey { typealias Value = ConsoleCommandActions }
extension FocusedValues {
    var consoleActions: ConsoleCommandActions? {
        get { self[ConsoleCommandsKey.self] }
        set { self[ConsoleCommandsKey.self] = newValue }
    }
}

struct ConsoleCommands: Commands {
    @FocusedValue(\.consoleActions) private var actions
    var body: some Commands {
        CommandGroup(after: .newItem) {
            action("new-task")
            action("run")
            action("close")
            action("reopen")
            Button("Close Window") { NSApp.keyWindow?.performClose(nil) }
                .keyboardShortcut("w", modifiers: [.command, .shift])
        }
        CommandMenu("View") {
            Button("Navigator") { actions?.navigate() }.keyboardShortcut("k")
            Button("Toggle Sidebar") { actions?.toggleSidebar() }.keyboardShortcut("\\")
            Button("Focus Sidebar") { actions?.focusSidebar() }.disabled(actions == nil)
            Button("Focus Terminal") { actions?.focusTerminal?() }.disabled(actions?.focusTerminal == nil)
            action("zoom")
            Menu("Focus Pane") { action("focus") }.disabled(actions == nil)
            action("back")
            action("forward")
            action("next")
            action("previous")
            Menu("Go to Task") { action("goto") }
            action("toggle-shell")
            action("toggle-browser")
            action("page-back")
            action("page-forward")
            action("reload")
        }
    }

    /// One menu item per default key binding, titled and keyed from the registry.
    @MainActor @ViewBuilder
    private func action(_ name: String) -> some View {
        if let action = try? DesktopAction.named(name) {
            let bindings: [DesktopKeyBinding?] = action.keys.isEmpty ? [nil] : action.keys
            ForEach(Array(bindings.enumerated()), id: \.offset) { _, binding in
                let args = binding?.args ?? [:]
                let title = args["index"].map { "Task \($0)" } ?? args["direction"]?.capitalized ?? action.title
                let button = Button(title) {
                    guard let actions else { return }
                    do { try action.perform(actions.context().with(args: args)) } catch { actions.failed(error) }
                }
                .disabled(actions.map { action.available($0.context().with(args: args)) } != true)
                if let binding {
                    button.keyboardShortcut(binding.keyEquivalent, modifiers: binding.eventModifiers)
                } else {
                    button
                }
            }
        }
    }
}

extension ShellTab {
    var terminal: TerminalIdentity? {
        location.map { .shell(id, machineId: $0.machineId) }
    }
}

extension PaneState {
    var terminal: TerminalIdentity? { content.terminal }
}

extension PaneContent {
    /// Content owns terminal identity; workspace placement grants no location fallback.
    var terminal: TerminalIdentity? {
        switch self {
        case .shell: selectedShell?.terminal
        case .session(let id?, let location, _): location.map { .session(id, machineId: $0.machineId) }
        case .session(nil, _, _), .browser: nil
        }
    }
}

/// One retained workspace for Work: the pane layout and the
/// pool of live surfaces behind it.
@MainActor
@Observable
final class SessionsWorkspace {
    var conversationRequest: UUID?
    var openingConversation: Bool { conversationRequest != nil }
    var conversationError: String?
    private var didPrepareTaskPanes = false

    /// Run one conversation opening against this workspace's progress and error.
    func openConversation(_ open: () async throws -> Void) async {
        guard !openingConversation else { return }
        let request = UUID()
        conversationRequest = request
        conversationError = nil
        defer { if conversationRequest == request { conversationRequest = nil } }
        do { try await open() } catch { conversationError = error.localizedDescription }
    }

    /// Populate this retained Work even if navigation changes while lf prepares it.
    func openPrimaryConversation(wave: String? = nil, store: SessionsStore, registry: SessionsWorkspaceRegistry) async {
        await openConversation {
            let record = try await store.query.ensurePrimarySession(wave: wave, cwd: store.repoPath)
            store.reconcile([record])
            store.adoptPreparation(record)
            registry.revealSession(record, in: self)
            presentation.content = .terminals
            presentation.selectedSessionId = record.id
            await store.select(record.id)
        }
    }

    func toggleFocus(_ paneId: String) {
        Perf.begin(Perf.retainedWorkspaceAction, multiplexer.zoomedPaneId == nil ? "focus" : "restore", id: "workspace")
        multiplexer.toggleZoom(paneId)
    }
    func setCollapsed(paneId: String, collapsed: Bool) {
        Perf.begin(Perf.retainedWorkspaceAction, collapsed ? "collapse" : "expand", id: "workspace")
        multiplexer.setCollapsed(paneId: paneId, collapsed: collapsed)
    }

    var key: WorkWorkspaceKey
    var title: String
    var location: WorkspaceIdentity?
    var presentation = WorkNavigation.WorkspacePresentation()
    let localMachineId: String?

    /// A Task's first conversation brings its Files into view once.
    func prepareTaskPanes(query: RegistryQuery = RegistryQueryLocal.shared) {
        guard key.kind == .task, !didPrepareTaskPanes,
              let session = multiplexer.sessionPane, session.sessionId != nil, session.content.location != nil,
              case .browser(_, let tabs, _, _, _)? = multiplexer.browserPane?.content, tabs.isEmpty else { return }
        didPrepareTaskPanes = true
        guard let browser = multiplexer.showBrowser(.files, location: session.content.location,
            execution: session.executionLocation, focus: false) else { return }
        // A location error is presented by the browser's normal error view.
        _ = try? files(for: browser.content, task: nil, query: query)
    }

    /// Add a shell tab at the Task's recorded location.
    @discardableResult
    func newShell(command: [String]? = nil, focus: Bool = true) throws -> PaneState? {
        guard key.kind == .task else {
            throw RegistryQueryError("Shells belong to Tasks; no shell was opened.")
        }
        let (location, execution) = try shellLocation()
        let argv = try command.map { try shellCommand($0, at: location, execution: execution) }
            ?? shellCommand(at: location, execution: execution)
        return multiplexer.newShell(command: argv, location: location, execution: execution, focus: focus)
    }

    func toggleShell() throws {
        guard key.kind == .task, let pane = multiplexer.shellPane else { return }
        if pane.content.selectedShell == nil { try newShell() } else { toggleTool(pane) }
    }

    /// Show or hide the browser; an empty one opens on Files.
    func toggleBrowser() {
        guard key.kind == .task, let pane = multiplexer.browserPane else { return }
        if pane.content.selectedBrowserTab == nil { showBrowser(.files) } else { toggleTool(pane) }
    }

    private func toggleTool(_ pane: PaneState) {
        let hidden = multiplexer.collapsedPaneIds.contains(pane.id)
        multiplexer.setCollapsed(paneId: pane.id, collapsed: !hidden)
        if hidden { multiplexer.setFocusedPane(pane.id) }
    }

    /// Select a browser tab at the location of `owner`, or the Task's own.
    @discardableResult
    func showBrowser(_ tab: BrowserTab, from owner: PaneState? = nil, focus: Bool = true) -> PaneState? {
        guard key.kind == .task else { return nil }
        let source = owner ?? multiplexer.sessionPane
        let place = source?.content.location ?? location
        let execution = source?.executionLocation
            ?? multiplexer.layout.allPanes.first { $0.content.location == place }?.executionLocation
        if tab == .files { selectFilesPage() }
        return multiplexer.showBrowser(tab, location: place, execution: execution, focus: focus)
    }

    /// ⌘W closes the named tab, else the selected one; a session pane, or one
    /// without tabs, hides. The closed tab keeps its shell or page for Undo until
    /// another close replaces it.
    func closePane(_ paneId: String, tab: String? = nil) {
        guard let pane = multiplexer.layout.pane(for: paneId) else { return }
        let id = tab ?? pane.content.selectedTabID
        let replaced: ClosedOccurrence?
        if case .shell(let tabs, _) = pane.content, let id, tabs.contains(where: { $0.id == id }) {
            replaced = multiplexer.closeShell(id)
        } else if case .browser(_, let tabs, _, _, _) = pane.content, let id, tabs.contains(where: { $0.id == id }) {
            replaced = multiplexer.closeBrowserTab(id)
        } else {
            replaced = multiplexer.close(paneId)
        }
        switch replaced {
        case .shellTab(let tab, _):
            if let location = tab.location { surfaces.release(.shell(tab.id, machineId: location.machineId)) }
        case .browserTab(let tab, _): pages.release(tab.id)
        case .pane, nil: break
        }
    }

    var browserPage: TaskFilesStore.Page {
        taskFiles?.store.page ?? .files
    }

    func selectFilesPage() {
        taskFiles?.store.page = .files
    }

    private func shellLocation() throws -> (WorkspaceIdentity, TaskLocationObservation?) {
        guard let location else {
            throw RegistryQueryError("Prepare this Task's location before opening a shell.")
        }
        let execution = multiplexer.layout.allPanes.first { $0.content.location == location }?.executionLocation
        return (location, execution)
    }

    /// A script's command runs in the shell's login shell at the recorded location.
    private func shellCommand(_ command: [String], at identity: WorkspaceIdentity, execution: TaskLocationObservation?) throws -> [String] {
        guard identity.machineId == localMachineId else {
            throw RegistryQueryError("Commands run in local Task shells; this Task is on another Machine. No shell was opened.")
        }
        return command
    }

    private func shellCommand(at identity: WorkspaceIdentity, execution: TaskLocationObservation?) throws -> [String] {
        if identity.machineId == localMachineId { return [] }
        guard let location = execution, location.machineID == identity.machineId,
              case .recorded(let task, let checkout) = location.location,
              checkout == identity.worktree else {
            throw RegistryQueryError("Reopen the remote Task before adding a shell; no local shell was opened.")
        }
        return [try LocalWaveAgentLauncher.controlLfPath(), "--machine", location.machineID,
            "--repository", location.repositoryID, "task", "shell", task,
            "--checkout", identity.worktree]
    }
    let multiplexer: MultiplexerStore
    /// One flag per pane strip, read only by that strip's Close control, so a
    /// crossing never re-renders another pane. Lookup itself is unobserved.
    @ObservationIgnored private var stripHovers: [String: HoverFlag] = [:]
    func stripHover(_ paneId: String) -> HoverFlag {
        if let flag = stripHovers[paneId] { return flag }
        let flag = HoverFlag()
        stripHovers[paneId] = flag
        return flag
    }
    private(set) var isRestoringIntent = false
    private var unavailableDrafts: [WorkFileDraft] = []
    var persistenceError: String?
    private var persistenceQuery: RegistryQuery?
    private var persistenceRepo: String?
    @ObservationIgnored private var paneRevision: Int64 = 0
    private var persistenceLoaded = false
    private var persistenceWriting = false
    @ObservationIgnored private var persistenceTask: Task<Void, Never>?
    private var taskFiles: (store: TaskFilesStore, execution: TaskLocationObservation?)?

    func files(issue: String, cwd: String, query: RegistryQuery = RegistryQueryLocal.shared, execution: TaskLocationObservation? = nil) -> TaskFilesStore {
        if let files = taskFiles { return files.store }
        let files = TaskFilesStore(issue: issue, cwd: cwd, query: query)
        taskFiles = (files, execution)
        if let machine = execution?.machineID ?? location?.machineId ?? localMachineId {
            let place = WorkViewPlan.Location(machineID: machine, cwd: cwd)
            files.restoreDrafts(unavailableDrafts, location: place)
            unavailableDrafts.removeAll { $0.location == place }
        }
        return files
    }
    /// Resolve Files from its content, not the selected checkout or a previous link.
    func files(for content: PaneContent, task: RoadmapTask?, query: RegistryQuery) throws -> TaskFilesStore {
        guard case .browser(let taskId, _, _, let location, let execution) = content else {
            throw RegistryQueryError("The addressed pane is not the browser.")
        }
        let issue: String
        let checkout: String
        if let execution {
            guard [taskId, task?.runtime?.workId, task?.task.identifier].compactMap({ $0 }).contains(execution.taskID) else {
                throw RegistryQueryError("The pane's recorded owner belongs to another Task; no files were read.")
            }
            guard case .recorded(let ownerTask, let path?) = execution.location else {
                throw RegistryQueryError("Task location is unavailable. Retained drafts have not moved.")
            }
            issue = ownerTask
            checkout = path
        } else {
            guard let recorded = location, recorded.machineId == localMachineId,
                  query.remoteMachine == nil else {
                throw RegistryQueryError("Task file location is unavailable. Reopen the Task; no local fallback was used.")
            }
            if let current = task?.reference.workspace?.identity, current != recorded {
                throw RegistryQueryError("The Task file location changed. Its previous drafts are retained, not redirected.")
            }
            issue = task?.runtime?.workId ?? taskId
            checkout = recorded.worktree
        }
        if let entry = taskFiles {
            let retained = entry.store
            guard entry.execution?.repositoryID == execution?.repositoryID,
                  entry.execution?.taskID == execution?.taskID,
                  entry.execution?.location == execution?.location,
                  retained.cwd == checkout,
                  retained.query.remoteMachine == (execution?.machineID ?? query.remoteMachine),
                  execution == nil || retained.issue == issue else {
                throw RegistryQueryError("The Task file location changed. Its previous drafts are retained, not redirected.")
            }
            return retained
        }
        // Retained documents keep their original transport; build it only once.
        let fileQuery = execution.map {
            query.onMachine($0.machineID, repository: $0.repositoryID).pinTaskLocation(to: $0)
        } ?? query
        return files(issue: issue, cwd: checkout, query: fileQuery, execution: execution)
    }
    let surfaces: GhosttySurfacePool
    let pages = BrowserPages()
    private var surfaceClosed: AnyCancellable?

    private var retainedStore: SessionsStore?

    func sessionStore(repoPath: String, query: RegistryQuery) -> SessionsStore {
        if let retainedStore {
            retainedStore.repoPath = WaveOrigin.resolve(repoPath)
            return retainedStore
        }
        let store = SessionsStore(repoPath: repoPath, query: query, surfaces: surfaces)
        retainedStore = store
        return store
    }

    init(key: WorkWorkspaceKey, title: String, location: WorkspaceIdentity?, localMachineId: String?, surfaces: GhosttySurfacePool = GhosttySurfacePool()) {
        self.key = key
        self.title = title
        self.location = location
        self.localMachineId = localMachineId
        self.surfaces = surfaces
        multiplexer = MultiplexerStore(taskId: key.kind == .task ? key.id : nil)
        // The workspace outlives SessionsView, including while a shell exits
        // on the Work screen or while another repository is selected.
        surfaceClosed = NotificationCenter.default.publisher(for: .ghosttySurfaceClosed)
            .sink { [weak self] notification in
                guard let terminal = notification.object as? TerminalIdentity,
                      case .shell(let tabId, _) = terminal else { return }
                MainActor.assumeIsolated {
                    guard let self else { return }
                    self.multiplexer.removeExitedShell(tabId, machineId: terminal.machineId)
                }
            }
    }
}

/// One repository's retained Work and surfaces, mounted by its current window.
/// Different repositories never share native views or multiplexers.
@MainActor
@Observable
final class SessionsWorkspaceRegistry {
    // Observable only for environment delivery; lazy creation must not invalidate views.
    @ObservationIgnored private var workspaces: [WorkWorkspaceKey: SessionsWorkspace] = [:]
    let surfaces = GhosttySurfacePool()
    private(set) var localMachineId: String?
    private(set) var machineError: String?

    init(localMachineId: String? = nil) { self.localMachineId = localMachineId }

    func sessionStore(repository: String, repoPath: String, query: RegistryQuery) -> SessionsStore {
        let path = WaveOrigin.resolve(repoPath)
        let root = WorkWorkspaceKey(repository: repository, kind: .repository, id: repository)
        return workspace(for: root, title: URL(fileURLWithPath: path).lastPathComponent,
            location: localMachineId.map { WorkspaceIdentity(machineId: $0, worktree: path) })
            .sessionStore(repoPath: path, query: query)
    }

    func refreshMachine(query: RegistryQuery) async {
        do { localMachineId = try await query.localMachineId(); machineError = nil }
        catch { machineError = error.localizedDescription }
    }

    func clearPresentation() {
        surfaces.clearFocus()
        openWork = []
        selectedWork = nil
    }

    func present(open: [WorkWorkspaceKey]) {
        openWork = open
        if selectedWork.map({ openWork.contains($0) }) != true {
            selectedWork = openWork.first
            surfaces.clearFocus()
        }
    }

    /// Work shown in the window, in slot fill order: readback, not an arrangement owner.
    private(set) var openWork: [WorkWorkspaceKey] = []

    private(set) var selectedWork: WorkWorkspaceKey?

    var keys: [WorkWorkspaceKey] { Array(workspaces.keys) }

    func retained(_ key: WorkWorkspaceKey) -> SessionsWorkspace? { workspaces[key] }

    @discardableResult
    func open(_ key: WorkWorkspaceKey, title: String, location: WorkspaceIdentity?) -> SessionsWorkspace {
        let workspace = workspace(for: key, title: title, location: location)
        openWork = [key]
        focus(key)
        return workspace
    }

    func focus(_ key: WorkWorkspaceKey) {
        guard openWork.contains(key) else { return }
        if selectedWork != key { surfaces.clearFocus() }
        selectedWork = key
    }

    func inspect() -> [DesktopWorkspaceInspection] {
        workspaces.values.sorted { ($0.key.repository, $0.key.kind.rawValue, $0.key.id) < ($1.key.repository, $1.key.kind.rawValue, $1.key.id) }.map { workspace in
            let panes = workspace.multiplexer
            return DesktopWorkspaceInspection(work: workspace.key, title: workspace.title,
                isOpen: openWork.contains(workspace.key), selected: selectedWork == workspace.key,
                layout: DesktopLayoutInspection(panes.layout,
                    surface: { content in content.terminal.flatMap { surfaces.surfaceIncarnation(for: $0) } },
                    commands: { content in content.terminal.flatMap { surfaces.commandStatus(for: $0) } }),
                focusedPane: panes.focusedPaneId,
                zoomedPane: panes.zoomedPaneId, hiddenPanes: panes.collapsedPaneIds.sorted())
        }
    }

    /// Record the Session owner; only explicit Changes opens a Files companion.
    @discardableResult
    func openLinkedFiles(_ destination: LinkedSession, in key: WorkWorkspaceKey, query: RegistryQuery) throws -> TaskFilesStore? {
        guard let identity = destination.record.workspace?.identity else { return nil }
        let workspace = workspace(for: key, title: destination.task.task.name, location: identity)
        workspace.multiplexer.updateSessionLocation(destination.record.id, location: identity, execution: destination.location)
        guard destination.showsChanges else { return nil }
        let content = PaneContent.browser(taskId: destination.task.id, tabs: [.files], selected: BrowserTab.files.id,
            location: identity, execution: destination.location)
        let files = try workspace.files(for: content, task: destination.task, query: query)
        files.showsChanges = true
        files.page = .files
        workspace.multiplexer.showBrowser(.files, location: identity, execution: destination.location,
            focus: workspace.multiplexer.pane(forSessionId: destination.record.id) != nil)
        return files
    }

    func readText(_ request: DesktopTextRequest) throws -> DesktopTextReading {
        let (workspace, pane, terminal) = try retainedPane(for: request.target)
        let store = workspace.multiplexer
        let result: DesktopTextResult
        if let terminal {
            result = try surfaces.readText(request, terminal: terminal)
        } else {
            result = .unavailable(reason: .notTerminal)
        }
        return DesktopTextReading(request: request, observedAt: Int64(Date().timeIntervalSince1970),
            hidden: !openWork.contains(workspace.key) || !store.isVisible(pane)
                || (request.target.tab != nil && pane.content.selectedShell?.id != request.target.tab),
            result: result)
    }

    /// The exact retained pane a script addressed; a replaced occurrence is a stale target.
    func retainedPane(for target: DesktopPaneTarget) throws -> (SessionsWorkspace, PaneState, TerminalIdentity?) {
        guard target.repository == target.work.repository,
              let workspace = workspaces[target.work],
              let pane = workspace.multiplexer.layout.pane(for: target.pane) else {
            throw DesktopTargetError("The pane content was removed or replaced. Inspect Desktop again; no pane was changed.")
        }
        if let id = target.tab {
            switch pane.content {
            case .shell(let tabs, _):
                guard let tab = tabs.first(where: { $0.id == id && $0.incarnation == target.incarnation }) else {
                    throw DesktopTargetError("The shell tab was removed or replaced; nothing changed.")
                }
                return (workspace, pane, tab.terminal)
            case .browser(_, let tabs, _, _, _):
                guard tabs.contains(where: { $0.id == id }), pane.incarnation == target.incarnation else {
                    throw DesktopTargetError("The browser tab was removed or replaced; nothing changed.")
                }
                return (workspace, pane, nil)
            case .session:
                throw DesktopTargetError("A session pane has no tabs; nothing changed.")
            }
        }
        guard pane.incarnation == target.incarnation else {
            throw DesktopTargetError("The pane content was replaced; inspect Desktop again.")
        }
        // A Shell tool target grants arrangement authority, not input to whichever
        // tab happens to be selected. Terminal requests must capture a tab.
        return (workspace, pane, pane.content.selectedShell == nil ? pane.terminal : nil)
    }

    func key(containingShell id: String, machineId: String) -> WorkWorkspaceKey? {
        workspaces.first {
            $0.value.multiplexer.shell(id)?.location?.machineId == machineId
        }?.key
    }

    /// A partial inventory can update content location, never remove a Work tab.
    func reconcileMembership(_ records: [SessionRecord]) {
        for workspace in workspaces.values {
            for record in records {
                let belongs: Bool?
                switch workspace.key.kind {
                case .task:
                    let tasks = Set(record.taskIds + [record.workspace?.taskId, record.work?.kind == .task ? record.work?.id : nil].compactMap { $0 })
                    belongs = tasks.isEmpty ? nil : tasks.contains(workspace.key.id)
                case .wave: belongs = record.waveId.map { $0 == workspace.key.id }
                case .repository: belongs = nil
                }
                // A peer's physical Task ids cannot disprove the local Work
                // correspondence carried by an explicitly opened remote pane.
                if belongs == false, record.workspace?.machineId == localMachineId {
                    workspace.multiplexer.removeSessions([record.id])
                }
                else { workspace.multiplexer.updateSessionLocation(record.id, location: record.workspace?.identity) }
            }
        }
    }

    /// One native occurrence per repository. A deliberate eligible Work opening can
    /// relocate presentation, but neither copies a surface nor binds the Session.
    func revealSession(_ record: SessionRecord, in workspace: SessionsWorkspace) {
        for other in workspaces.values where other !== workspace {
            other.multiplexer.removeSessions([record.id])
        }
        workspace.multiplexer.reveal(sessionId: record.id, location: record.workspace?.identity)
    }

    func reassociateRepository(from previous: String, to repository: String) {
        for key in keys where key.repository == previous {
            canonicalize(key, as: WorkWorkspaceKey(repository: repository, kind: key.kind,
                id: key.kind == .repository ? repository : key.id))
        }
    }

    func key(containingSession id: String) -> WorkWorkspaceKey? {
        workspaces.first { $0.value.multiplexer.pane(forSessionId: id) != nil }?.key
    }

    func canonicalize(_ previous: WorkWorkspaceKey, as key: WorkWorkspaceKey) {
        guard previous != key, workspaces[key] == nil, let workspace = workspaces.removeValue(forKey: previous) else { return }
        workspace.key = key
        workspaces[key] = workspace
        openWork = openWork.map { $0 == previous ? key : $0 }
        if selectedWork == previous { selectedWork = key }
    }

    func workspace(for key: WorkWorkspaceKey, title: String, location: WorkspaceIdentity?) -> SessionsWorkspace {
        if let existing = workspaces[key] {
            if existing.title != title { existing.title = title }
            if let location, existing.location != location { existing.location = location }
            return existing
        }
        let workspace = SessionsWorkspace(key: key, title: title, location: location,
            localMachineId: localMachineId, surfaces: surfaces)
        workspaces[key] = workspace
        return workspace
    }

}

struct SessionItem: Identifiable, Equatable {
    enum State: Equatable {
        case pending
        case elsewhere
        case opening
        case prepared
        case live
        case failed(String)
        case ended(String)
    }

    var record: SessionRecord
    var state: State
    var agentOverride: String?
    var launchId = UUID()

    var id: String { record.id }
    var presentationProvider: String? { agentOverride ?? record.provider }

    var launchArguments: [String] {
        record.openArgv + (agentOverride.map { ["--agent", $0] } ?? [])
    }

    var surface: SessionRecord? {
        switch state {
        case .prepared, .live: record
        case .pending, .elsewhere, .opening, .failed, .ended: nil
        }
    }

    var error: String? {
        guard case .failed(let message) = state else { return nil }
        return message
    }

    mutating func adoptPreparation(_ record: SessionRecord, intent: Preparation) {
        self.record = record
        // A prepared attachment can join a free viewer even while the provider
        // is active. Only an explicit move may displace an existing viewer.
        if intent == .attach {
            state = .prepared
        } else if record.openArgv.isEmpty {
            // Nothing to start: retained work attaches when shown.
            state = record.action(.open)?.unavailableReason.map(State.failed) ?? .pending
        } else if intent != .moveHere, record.action(.moveHere) != nil {
            state = .elsewhere
        } else {
            state = record.action(intent == .moveHere ? .moveHere : .open)?
                .unavailableReason.map(State.failed) ?? .prepared
        }
    }
}

/// What a pane asked lf to prepare. Attaching never starts a provider; opening
/// is `lf resume`, which attaches when it can and otherwise reopens history.
enum Preparation {
    case attach, open, moveHere
}

@MainActor
@Observable
final class SessionsStore {
    private(set) var sessions: [SessionItem] = []

    let surfaces: GhosttySurfacePool

    var repoPath: String
    let query: RegistryQuery
    private var executionLocations: [String: TaskLocationObservation] = [:]

    func executionLocation(for id: String) -> TaskLocationObservation? { executionLocations[id] }

    func retainLocation(_ location: TaskLocationObservation, for record: SessionRecord) {
        executionLocations[record.id] = location
    }

    func query(for record: SessionRecord) -> RegistryQuery {
        guard let location = executionLocations[record.id] else { return query }
        return query.onMachine(location.machineID, repository: location.repositoryID)
    }

    func taskSubject(for record: SessionRecord) -> WorkReference? {
        executionLocations[record.id].map { .task(id: $0.taskID) }
    }

    func presentationDirectory(for record: SessionRecord, checkout: String) -> String {
        executionLocations[record.id] == nil ? checkout : repoPath
    }

    private let metrics: SessionsLatencyMetrics
    @ObservationIgnored private var hasRecordedSessionsLoad = false

    init(
        repoPath: String,
        query: RegistryQuery = RegistryQueryLocal.shared,
        surfaces: GhosttySurfacePool = GhosttySurfacePool()
    ) {
        self.repoPath = WaveOrigin.resolve(repoPath)
        self.query = query
        self.surfaces = surfaces
        metrics = SessionsLatencyMetrics(scope: URL(fileURLWithPath: repoPath).lastPathComponent)
    }

    func reconcile(_ records: [SessionRecord]) {
        if !hasRecordedSessionsLoad {
            metrics.recordSessionsLoaded(count: records.count)
            hasRecordedSessionsLoad = true
        }
        // Working-set pages are filtered. Absence establishes neither deletion
        // nor completion and must not discard prepared commands or native drafts.

        // Reconcile a copy and publish once: element writes notify even when equal,
        // and every pane reads this array.
        var next = sessions
        for record in records {
            let observedState: SessionItem.State = localTerminal(for: record) != nil
                ? .live : record.action(.moveHere) != nil ? .elsewhere : .pending
            if let index = next.firstIndex(where: { $0.id == record.id }) {
                // Keep a prepared command until the retained native view consumes it.
                if case .prepared = next[index].state {
                    var observed = record
                    observed.openArgv = next[index].record.openArgv
                    next[index].record = observed
                    continue
                }
                next[index].record = record
                if record.provider == next[index].agentOverride {
                    next[index].agentOverride = nil
                }
                if case .opening = next[index].state { continue }
                if observedState == .pending {
                    switch next[index].state {
                    case .failed, .ended: continue
                    default: break
                    }
                }
                next[index].state = observedState
            } else {
                next.append(SessionItem(record: record, state: observedState))
            }
        }
        if next != sessions { sessions = next }
        surfaces.associateProgramStatus(sessions.map(\.record))
    }

    private func prepare(_ id: String, intent: Preparation) async {
        guard let index = _index(id) else { return }
        let replacing = intent == .moveHere
        if replacing, sessions[index].record.action(.moveHere)?.unavailableReason != nil { return }
        switch sessions[index].state {
        case .pending, .failed, .ended:
            sessions[index].state = .opening
        case .elsewhere where intent != .open:
            sessions[index].state = .opening
        case .elsewhere, .opening, .prepared, .live:
            return
        }
        do {
            let record = sessions[index].record
            let executionQuery = query(for: record)
            if let location = executionLocations[id] {
                let current = try await executionQuery.taskLocation(task: location.taskID, repository: location.repositoryID, machine: location.machineID)
                guard current.location == location.location else {
                    throw RegistryQueryError("The recorded execution location changed; reopen the Task before connecting.")
                }
            }
            let stage = intent == .attach ? "attach_prepare" : "connect_prepare"
            Perf.agentInput("\(stage)_begin", session: id)
            let surface: SessionRecord? = try await intent == .attach
                ? executionQuery.prepareSessionAttachment(id: id, cwd: repoPath)
                : executionQuery.prepareSession(id: id, takeover: replacing, cwd: repoPath)
            try Task.checkCancellation()
            Perf.agentInput("\(stage)_returned", session: id)
            guard let latest = _index(id) else { return }
            if case .opening = sessions[latest].state {
                if let surface {
                    sessions[latest].adoptPreparation(surface, intent: intent)
                } else {
                    sessions[latest].state = sessions[latest].record.action(.moveHere) != nil
                        ? .elsewhere : .pending
                }
            }
        } catch {
            guard let latest = _index(id) else { return }
            sessions[latest].state = .failed(error.localizedDescription)
        }
    }

    func adoptPreparation(_ record: SessionRecord) {
        guard let index = _index(record.id), localTerminal(for: record) == nil else { return }
        switch sessions[index].state {
        case .pending, .failed, .ended, .elsewhere:
            sessions[index].adoptPreparation(record, intent: .open)
        case .opening, .prepared, .live:
            break
        }
    }

    func select(_ id: String) async {
        guard let index = _index(id) else { return }
        if localTerminal(for: sessions[index].record) != nil {
            sessions[index].state = .live
            return
        }
        if case .live = sessions[index].state {
            sessions[index].state = sessions[index].record.action(.moveHere) != nil
                ? .elsewhere : .pending
        }
        await prepare(id, intent: .attach)
    }

    func resume(_ id: String) async {
        await prepare(id, intent: .open)
    }

    func moveHere(_ id: String) async {
        await prepare(id, intent: .moveHere)
    }

    @discardableResult
    func switchProvider(_ id: String, to agent: String) async throws -> SessionRecord {
        guard let index = _index(id) else { throw RegistryQueryError("Session is unavailable.") }
        let previous = sessions[index]
        // Preparation checks the requested provider's continuation before releasing
        // the retained terminal. Failed reads leave current drafts and updates intact.
        let prepared = try await query(for: previous.record).prepareSession(
            id: id, takeover: true, agent: agent, cwd: repoPath)
        if let reason = prepared.action(.moveHere)?.unavailableReason {
            throw RegistryQueryError(reason)
        }
        guard let current = _index(id) else { throw RegistryQueryError("Session is unavailable.") }
        if let terminal = localTerminal(for: previous.record) { surfaces.release(terminal) }
        sessions[current].record = prepared
        sessions[current].agentOverride = agent
        sessions[current].launchId = UUID()
        sessions[current].state = .prepared
        return prepared
    }

    func reset(_ id: String) async throws -> SessionRecord {
        guard let index = _index(id) else { throw RegistryQueryError("Session is unavailable.") }
        let previous = sessions[index].record
        let successor = try await query(for: previous).resetSession(
            previous, agent: previous.provider, cwd: repoPath)
        if let location = executionLocations[id] { retainLocation(location, for: successor) }
        reconcile([successor])
        adoptPreparation(successor)
        return successor
    }

    func beginPaneLoad(_ id: String) {
        metrics.beginPaneLoad(id)
    }

    func localTerminal(for record: SessionRecord) -> TerminalIdentity? {
        guard let machineId = record.workspace?.machineId else { return nil }
        let session = TerminalIdentity.session(record.id, machineId: machineId)
        if surfaces.hasSurface(session) { return session }
        return record.terminalIds.lazy.map { TerminalIdentity.shell($0, machineId: machineId) }
            .first(where: surfaces.hasSurface)
    }

    func recordPaneLive(_ id: String) {
        metrics.recordPaneLive(id)
        if let index = _index(id), case .prepared = sessions[index].state {
            sessions[index].state = .live
        }
    }

    func recordPaneFailure(_ id: String, reason: String) {
        guard let index = _index(id), sessions[index].state == .prepared else { return }
        sessions[index].state = .failed(reason)
    }

    /// Viewer exit does not establish provider exit. Keep its output and require
    /// a deliberate retry; inventory refresh must not resume execution.
    func noteSurfaceClosed(_ terminal: TerminalIdentity, output: String? = nil) {
        guard case .session(let id, let machineId) = terminal,
              !surfaces.hasSurface(terminal),
              let index = _index(id), sessions[index].record.workspace?.machineId == machineId
        else { return }
        switch sessions[index].state {
        case .prepared, .live:
            sessions[index].state = sessions[index].record.action(.moveHere) != nil
                ? .elsewhere : .ended(output ?? "Terminal output is unavailable.")
        case .pending, .elsewhere, .opening, .failed, .ended:
            break
        }
    }

    private func _index(_ id: String) -> Int? {
        sessions.firstIndex { $0.id == id }
    }
}

@MainActor
private final class SessionsLatencyMetrics {
    private let logger = Logger(subsystem: "studio.loopflow", category: "sessions-latency")
    private let scope: String
    private let sessionsStartedAt = DispatchTime.now()
    private var paneStarts: [String: DispatchTime] = [:]

    init(scope: String) {
        self.scope = scope
    }

    func recordSessionsLoaded(count: Int) {
        let elapsed = Self._milliseconds(since: sessionsStartedAt)
        logger.info(
            "metric=time_to_sessions scope=\(self.scope, privacy: .public) value_ms=\(elapsed, privacy: .public) count=\(count, privacy: .public)"
        )
    }

    func beginPaneLoad(_ id: String) {
        paneStarts[id] = DispatchTime.now()
    }

    func recordPaneLive(_ id: String) {
        guard let start = paneStarts.removeValue(forKey: id) else { return }
        let elapsed = Self._milliseconds(since: start)
        logger.info(
            "metric=time_to_live_pane session=\(id, privacy: .private(mask: .hash)) value_ms=\(elapsed, privacy: .public)"
        )
    }

    private static func _milliseconds(since start: DispatchTime) -> Double {
        Double(DispatchTime.now().uptimeNanoseconds - start.uptimeNanoseconds) / 1_000_000
    }
}

struct SessionsView: View {
    let model: WorkModel
    let repoPath: String
    let workspaces: SessionsWorkspaceRegistry
    var repository: String? = nil
    @State private var resolvedRepository: String?
    @State private var repositoryError: String?
    var query: RegistryQuery = RegistryQueryLocal.shared

    var body: some View {
        let taskPage = model.linkedTaskPage
        Group {
            if let home = workspaces.localMachineId, let repository = repository ?? resolvedRepository {
                SessionsContentView(model: model, repoPath: repoPath, workspaces: workspaces,
                                    machineId: home, repository: repository, query: query)
            } else {
                HStack {
                    if model.navigation.showsNavigator {
                        WorkNavigator(model: model, onOpenSession: { _ in })
                    }
                    if let error = workspaces.machineError ?? repositoryError {
                        ContentUnavailableView("Workspace unavailable", systemImage: "folder", description: Text(error))
                            .task(id: taskPage) {
                                if let request = taskPage { model.finishTaskPage(request, error: error) }
                            }
                    } else {
                        ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                }
            }
        }
        .task {
            await workspaces.refreshMachine(query: query)
            if repository == nil {
                do { resolvedRepository = try await query.repositoryIdentity(cwd: repoPath).id }
                catch { repositoryError = error.localizedDescription }
            }
            if let home = workspaces.localMachineId, workspaces.machineError == nil { model.confirmMachine(home) }
        }
    }
}

/// Unified Work navigation around the existing retained native workspace.
/// The sidebar's width, read here so a divider drag re-renders this column
/// rather than the whole window.
private struct SidebarColumn<Content: View>: View {
    let navigation: WorkNavigation
    let availableWidth: CGFloat
    @ViewBuilder let content: Content

    var body: some View {
        content.frame(width: min(navigation.sidebarWidth, max(240, availableWidth - 360)))
    }
}

/// Applies each Session reading to the window's stores. Zero-size, so a Session
/// change re-runs this view and not the window that hosts it.
private struct SessionsReconciler: View {
    let model: WorkModel
    let store: SessionsStore
    let workspaces: SessionsWorkspaceRegistry

    var body: some View {
        Color.clear.frame(width: 0, height: 0)
            .onChange(of: model.sessions.value, initial: true) { _, records in
                guard model.repoPath?.normalizedFilePath == store.repoPath.normalizedFilePath,
                      let records else { return }
                store.reconcile(records)
                if model.sessions.errorMessage == nil { workspaces.reconcileMembership(records); model.revealPrimaryConversation() }
            }
    }
}

struct SessionsContentView: View {
    @Environment(\.theme) private var theme
    let machineId: String
    private var rootIdentity: WorkspaceIdentity { WorkspaceIdentity(machineId: machineId, worktree: store.repoPath) }
    let repository: String
    private var currentKey: WorkWorkspaceKey { key(for: model.selection) }
    private func key(for reference: WorkReference?) -> WorkWorkspaceKey {
        model.workspaceKey(for: reference, repository: repository)
    }
    private func title(for key: WorkWorkspaceKey) -> String { model.workspaceTitle(key) }
    private var workspace: SessionsWorkspace {
        workspaces.workspace(for: currentKey, title: title(for: currentKey),
            location: currentKey.kind == .task ? taskIdentity : rootIdentity)
    }
    @Bindable var model: WorkModel
    private let workspaces: SessionsWorkspaceRegistry
    private let query: RegistryQuery
    private let store: SessionsStore
    @State private var launchError: String?
    @State private var launchErrorTitle = "Could not start conversation"
    @State private var actionTaskID: String?
    @Environment(\.palette) private var palette

    private var multiplexer: MultiplexerStore {
        workspace.multiplexer
    }
    private var taskIdentity: WorkspaceIdentity? {
        if let task = fileTask {
            return navigation.preparedTaskWorktrees[task.task.id] ?? task.task.reference.workspace?.identity
        }
        return selectedWorkspace?.taskId == nil ? nil : selectedWorkspace?.identity
    }
    private var selectedWorkspace: SessionWorkspace? {
        guard model.repoPath?.normalizedFilePath == store.repoPath.normalizedFilePath else { return nil }
        return store.sessions.first { $0.id == navigation.selectedSessionId }?.record.workspace
    }
    init(model: WorkModel, repoPath: String, workspaces: SessionsWorkspaceRegistry,
         machineId: String, repository: String, query: RegistryQuery = RegistryQueryLocal.shared) {
        self.machineId = machineId
        self.repository = repository
        self.model = model
        self.workspaces = workspaces
        self.query = query
        model.bindWorkspaces(workspaces, repository: repository)
        self.store = workspaces.sessionStore(repository: repository, repoPath: repoPath, query: query)
    }

    private var navigation: WorkNavigation { model.navigation }
    private var terminalsVisible: Bool {
        !navigation.showsSelection && workspaces.selectedWork != nil && fileTask?.task.awaitsStart != true
            && (currentKey.kind != .task || fileTask?.task.runtime?.owner != nil || workspace.presentation.content == .terminals)
    }
    private var fileTask: TaskProjection? {
        // A departing repository view must not mount the next repository's
        // retained terminals while SwiftUI replaces its hierarchy.
        guard model.repoPath?.normalizedFilePath == store.repoPath.normalizedFilePath else { return nil }
        return model.breadcrumb?.task
    }

    private var mainPane: some View {
        ZStack {
            if navigation.showsSelection {
                DashboardView(model: model, workspaces: workspaces, sessions: store)
            } else if let key = workspaces.selectedWork {
                VStack(spacing: 0) {
                    workspacePanel(key).id(key)
                }
                .padding([.horizontal, .bottom], PaneGeometry.windowInset)
                .background(palette.surfaceMuted)
                .accessibilityIdentifier("work-layout")
                .transition(.zoom(from: navigation.zoomOrigin))
            } else {
                ContentUnavailableView("No Work open", systemImage: "rectangle.on.rectangle",
                    description: Text("Open a repository, Wave or Task from the sidebar."))
            }
        }
        .coordinateSpace(.named(ZoomFromTile.space))
    }

    var body: some View {
        GeometryReader { geometry in
            HStack(spacing: 0) {
                if navigation.showsNavigator {
                    SidebarColumn(navigation: navigation, availableWidth: geometry.size.width) {
                        WorkNavigator(model: model, onOpenSession: openSession, onNewTaskSession: { id in
                            Task { do { try await newTaskSession(id) } catch { launchError = error.localizedDescription } }
                        })
                        .equatable()
                    }
                    SidebarDivider(navigation: navigation, availableWidth: geometry.size.width)
                }
                VStack(spacing: 0) {
                    // The main pane alone: the sidebar stays put when a Task opens.
                    if let task = model.breadcrumb?.task?.task {
                        TaskWorkflowBar(model: model, task: task, work: model.taskWork[task.id])
                    }
                    mainPane
                }
            }
        }
        .background(palette.background)
        .tint(palette.actionInk)
        .environment(model)
        .focusedSceneValue(\.consoleActions, ConsoleCommandActions(
            navigate: { navigation.palette = .search }, toggleSidebar: { navigation.toggleNavigator() },
            focusSidebar: { navigation.showsNavigator = true; navigation.sidebarFocusRequested = true },
            focusTerminal: multiplexer.focusedPane.terminal.map { terminal in { workspaces.surfaces.focus(terminal) } },
            context: { actionContext }, failed: { actionFailed($0) }))
        .background {
            Button("Task actions") { actionTaskID = fileTask?.task.id }
                .keyboardShortcut("/", modifiers: .command)
                .disabled(fileTask == nil)
                .hidden().accessibilityHidden(true).frame(width: 0, height: 0)
        }
        .popover(isPresented: Binding(get: { actionTaskID != nil }, set: { if !$0 { actionTaskID = nil } })) {
            if let id = actionTaskID, let found = model.task(id: id) {
                VStack(alignment: .leading, spacing: 10) {
                    TaskActionItems(model: model, task: found.task, wave: found.wave?.wave,
                        newConversation: {
                            Task { do { try await newTaskSession(id) } catch { launchError = error.localizedDescription } }
                        }, openSession: openSession)
                }.padding(12).onExitCommand { actionTaskID = nil }
            }
        }
        .overlay {
            if navigation.palette == nil {
                DesktopActionMonitor(context: { actionContext }, failed: { actionFailed($0) })
                    .allowsHitTesting(false).frame(width: 0, height: 0)
            }
        }
        .background {
            WorkPaletteShortcut(presented: navigation.palette != nil, restoreFocus: navigation.restorePaletteFocus,
                toggleSidebar: { navigation.toggleNavigator() }, modifiersChanged: { if navigation.showsSlotHints != $0 { navigation.showsSlotHints = $0 } }) {
                navigation.palette = .search
            }.frame(width: 0, height: 0)
        }
        .sheet(isPresented: Binding(
            get: { navigation.palette != nil },
            set: { if !$0 { navigation.palette = nil } }
        )) {
            switch navigation.palette {
            case .workflow(let name):
                WorkflowCatalogInspector(entry: model.workflowCatalog.value?.first { $0.name == name }, model: model)
            case .graph(let name):
                GraphCatalogInspector(entry: model.graphCatalog.value?.first { $0.name == name }, navigation: navigation, model: model)
            case .search:
                WorkPalette(model: model, activate: navigate)
            case .sidebarSearch:
                WorkPalette(model: model, sidebarOnly: true, activate: navigate)
            case .newTask:
                NewTaskSheet(model: model)
            case nil:
                EmptyView()
            }
        }
        .sheet(item: $model.historyWave) { wave in
            ProjectHistoryView(wave: wave.name, repo: wave.repo, sourceReference: model.historyReference)
        }
        .sheet(isPresented: Binding(
            get: { navigation.renaming != nil || navigation.binding != nil },
            set: { if !$0 { model.cancelSessionRename(); model.cancelSessionBinding() } }
        )) { SessionActionSheet(model: model) }
        .onChange(of: multiplexer.focusedPaneId) { _, _ in
            guard terminalsVisible, model.repoPath?.normalizedFilePath == store.repoPath.normalizedFilePath else { return }
            if !focusedPaneSessions.contains(where: { $0.id == navigation.selectedSessionId }) {
                navigation.selectedSessionId = focusedPaneSessions.first?.id
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didResignKeyNotification)) { _ in
            navigation.showsSlotHints = false
        }
        .background { SessionsReconciler(model: model, store: store, workspaces: workspaces) }

        .onReceive(NotificationCenter.default.publisher(for: .ghosttySurfaceClosed)) { notification in
            guard let terminal = notification.object as? TerminalIdentity else { return }
            store.noteSurfaceClosed(terminal, output: notification.userInfo?["output"] as? String)
        }
        .onReceive(NotificationCenter.default.publisher(for: .openSessions)) { _ in
            navigation.content = .terminals
        }
        .alert(launchErrorTitle, isPresented: Binding(
            get: { launchError != nil },
            set: { if !$0 { launchError = nil } }
        )) {
            Button("OK") { launchError = nil }
        } message: { Text(launchError ?? "") }
        .accessibilityIdentifier("sessions-surface")
    }

    private func focusPane(_ pane: String, session: String?, in workspace: SessionsWorkspace) {
        if currentKey != workspace.key { model.select(workspace.key.reference) }
        workspace.multiplexer.setFocusedPane(pane)
        // Companion shells retain the current conversation context.
        guard let session, navigation.selectedSessionId != session else { return }
        navigation.selectedSessionId = session
        navigation.content = .terminals
    }

    private func workspacePanel(_ key: WorkWorkspaceKey) -> some View {
        let task = key.kind == .task ? model.task(id: key.id) : nil
        let location = task.flatMap { navigation.preparedTaskWorktrees[$0.task.id] ?? $0.task.reference.workspace?.identity }
        let entry = workspaces.workspace(for: key, title: title(for: key),
            location: key.kind == .task ? location : rootIdentity)
        let active = currentKey == key
        let terminals = key.kind != .task || task?.task.runtime?.owner != nil || entry.presentation.content == .terminals
        return VStack(spacing: 0) {
            if entry.isRestoringIntent { ProgressView("Restoring workspace") }
            if let error = entry.persistenceError {
                HStack {
                    Text("Layout not saved: " + error).foregroundStyle(palette.warning).textSelection(.enabled)
                    Button("Reload saved intent") { Task { await entry.loadPaneIntent() } }
                }.padding(8)
            }
            if terminals, let task, active {
                missingCheckout
                    .task(id: model.linkedTaskPage) {
                        if let request = model.linkedTaskPage,
                           request.taskID == task.task.id, request.repo == store.repoPath.normalizedFilePath {
                            model.finishTaskPage(request)
                        }
                    }
            }
            if let task, task.task.awaitsStart, active {
                // The Session slot offers Start; its brief stays beside it.
                GeometryReader { geometry in
                    HStack(spacing: 0) {
                        TaskStartOffer(model: model, task: task.task)
                            .frame(width: geometry.size.width * 0.62)
                            .background(theme.terminal.background.color)
                        Divider()
                        WorkSurfaceView(model: model, onOpenSession: openSession, onNewSession: newTaskSession)
                        if navigation.showsActivity { WorkActivityView(model: model).frame(minWidth: 230, maxWidth: 360) }
                    }
                }
            } else if key.kind != .task, active, let view = navigation.waveView {
                // A Wave view, the repository page included: the conversation and the
                // standup board share the top band; the Tasks run along the bottom.
                WaveViewLayout(model: model, view: view, workspace: entry) {
                    conversationPanes(key, entry, isFocused: navigation.palette == nil)
                } board: {
                    WaveStandupBoard(model: model, view: view)
                } tiles: {
                    WaveTiles(model: model, workspaces: workspaces, sessions: store, view: view)
                }
            } else if terminals {
                conversationPanes(key, entry, isFocused: active && navigation.palette == nil)
            } else if active {
                HSplitView {
                    WorkSurfaceView(model: model, onOpenSession: openSession,
                                    onNewSession: newTaskSession)
                    if navigation.showsActivity { WorkActivityView(model: model).frame(minWidth: 230, maxWidth: 360) }
                }
            } else {
                Button("Open \(entry.title)") { model.select(key.reference) }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .clipShape(PaneGeometry.workspaceShape)
        .accessibilityIdentifier("work-workspace-\(key.id)")
    }

    /// A workspace's panes, with a retry when its conversation failed to open.
    private func conversationPanes(_ key: WorkWorkspaceKey, _ entry: SessionsWorkspace, isFocused: Bool) -> some View {
        WorkspacePanesView(workspace: entry, isFocused: isFocused, sessions: store, model: model,
            onFocus: { pane, session in focusPane(pane, session: session, in: entry) })
            .equatable()
            .overlay(alignment: .top) {
                if let error = entry.conversationError {
                    HStack {
                        Text(error).textSelection(.enabled)
                        Button("Retry") {
                            if key.kind == .task, let work = key.reference { openTask(work) }
                            else if let work = key.reference { openWaveConversation(work) }
                            else { openRepositoryConversation() }
                        }
                    }.foregroundStyle(theme.palette.text)
                        .padding(8).background(theme.terminal.background.color)
                }
            }
    }

    @ViewBuilder
    private var missingCheckout: some View {
        if let task = fileTask, task.task.reference.workspace?.localExists == false,
           navigation.preparedTaskWorktrees[task.task.id] == nil {
            HStack(spacing: Spacing.sm) {
                if let runtime = task.task.runtime, runtime.status.isTerminal {
                    Text("Recorded checkout is missing. This Task is \(runtime.status.label); it cannot be restored.")
                } else {
                    Text("Recorded checkout is missing")
                    Button("Restore checkout") { prepareTaskWorkspace() }
                }
                Spacer()
            }
            .font(Typography.meta).foregroundStyle(palette.textSecondary)
            .padding(.horizontal, 14).padding(.vertical, 6)
            .accessibilityIdentifier("work-missing-checkout")
        }
    }

    private func prepareTaskWorkspace() {
        guard let task = fileTask else { return }
        Task { @MainActor in
            do {
                let repo = store.repoPath
                let issue = task.task.task.identifier
                let path = try await LocalWaveAgentLauncher.checkoutTask(repoPath: repo, issue: issue)
                navigation.preparedTaskWorktrees[task.task.id] = path
            } catch {
                launchErrorTitle = "Could not prepare checkout"
                launchError = error.localizedDescription
            }
        }
    }

    func navigate(_ destination: WorkDestination) {
        // Refreshes can remove an action while the palette is open.
        guard model.paletteRows.contains(where: { $0.id == destination }) else { return }
        navigation.restorePaletteFocus = false
        navigation.palette = nil
        switch destination {
        case .repository, .openRepository, .wave, .task:
            model.jump(to: destination, source: "palette")
            return
        case .graph(let name): navigation.palette = .graph(name)
        case .rename(let id):
            guard let record = store.sessions.first(where: { $0.id == id })?.record else { return }
            openSession(record)
            model.beginSessionRename(record)
        case .bind(let id):
            guard let record = store.sessions.first(where: { $0.id == id })?.record else { return }
            model.beginSessionBinding(record)
        case .graphLog(let id):
            model.select(.task(id: id))
            navigation.content = .terminals
            workspace.showBrowser(.graphLog, from: multiplexer.focusedPane)
        }
        model.remember(destination)
    }

    private func openSession(_ record: SessionRecord) {
        model.openSession(record, store: store)
    }

    private func openTask(_ work: WorkReference) {
        let trace = UUID().uuidString
        Perf.$inputTrace.withValue(trace) {
            Perf.agentInput("task_click", task: work.id)
            Task { await model.openTaskConversation(work.id) }
        }
    }

    private func openRepositoryConversation() {
        guard model.repoPath?.normalizedFilePath == store.repoPath.normalizedFilePath else { return }
        model.select(nil)
        navigation.content = .terminals
        let entry = workspace
        Task { await entry.openPrimaryConversation(store: store, registry: workspaces) }
    }

    private func openWaveConversation(_ work: WorkReference) {
        guard let wave = model.wave(id: work.id)?.wave else { return }
        model.select(work)
        navigation.content = .terminals
        let entry = workspace
        Task { await entry.openPrimaryConversation(wave: wave.name, store: store, registry: workspaces) }
    }

    /// An independent conversation with Task context in the Task's checkout.
    /// Resolving the checkout prepares Task Work only; it never starts a
    /// Graph or replaces another Session.
    private func newTaskSession(_ taskId: String) async throws {
        guard let found = model.task(id: taskId) else { return }
        let origin = navigation
        let key = key(for: .task(id: taskId))
        let entry = workspaces.workspace(for: key, title: found.task.task.name, location: found.task.reference.workspace?.identity)
        let issue = found.task.task.identifier
        let checkout: WorkspaceIdentity
        if let workspace = found.task.reference.workspace, workspace.localExists == true, let identity = workspace.identity {
            checkout = identity
        } else {
            let repo = store.repoPath
            checkout = try await LocalWaveAgentLauncher.checkoutTask(repoPath: repo, issue: issue)
            origin.preparedTaskWorktrees[taskId] = checkout
        }
        // The captured entry receives a late preparation reply without changing selection.
        let lf = try LocalWaveAgentLauncher.controlLfPath()
        guard checkout.machineId == machineId else { throw RegistryQueryError("Open a conversation on its owning Machine") }
        entry.presentation.content = .terminals
        entry.location = checkout
        entry.multiplexer.newShell(command: TaskConversationLaunch(task: issue).arguments(lf: lf), location: checkout)
    }

    /// Sessions attached to the focused pane: a Session pane's own record, or
    /// every conversation running inside a shell pane.
    private var focusedPaneSessions: [SessionItem] {
        let pane = multiplexer.focusedPane
        switch pane.content {
        case .shell:
            guard let terminal = pane.terminal else { return [] }
            return store.sessions.filter { store.localTerminal(for: $0.record) == terminal }
        case .session(let id?, _, _):
            return store.sessions.filter { $0.id == id }
        case .session(nil, _, _), .browser:
            return []
        }
    }

    /// Keys and menus act on the shown workspace's focused pane; window actions need none.
    private var actionContext: DesktopActionContext {
        let shown = terminalsVisible
        return DesktopActionContext(model: model, workspaces: workspaces, workspace: shown ? workspace : nil,
            pane: shown ? multiplexer.focusedPane : nil)
    }

    private func actionFailed(_ error: Error) {
        launchErrorTitle = "Could not complete the action"
        launchError = error.localizedDescription
    }
}

private struct SidebarDivider: View {
    let navigation: WorkNavigation
    let availableWidth: CGFloat
    @Environment(\.palette) private var palette
    @State private var start: CGFloat?

    var body: some View {
        // A shadow's edge: dark against the sidebar, fading into the workspace.
        LinearGradient(colors: [palette.border, palette.border.opacity(0)], startPoint: .leading, endPoint: .trailing)
            .frame(width: 5)
            .contentShape(Rectangle())
            .onHover { if $0 { NSCursor.resizeLeftRight.push() } else { NSCursor.pop() } }
            .gesture(DragGesture(minimumDistance: 0).onChanged { value in
                if start == nil { start = navigation.sidebarWidth }
                navigation.resizeSidebar((start ?? navigation.sidebarWidth) + value.translation.width,
                                         availableWidth: availableWidth)
            }.onEnded { _ in start = nil })
            .accessibilityLabel("Sidebar width")
            .accessibilityValue("\(Int(navigation.sidebarWidth)) points")
            .accessibilityAdjustableAction { direction in
                navigation.resizeSidebar(navigation.sidebarWidth + (direction == .increment ? 20 : -20),
                                         availableWidth: availableWidth)
            }
            .accessibilityIdentifier("work-sidebar-divider")
    }
}

#endif

extension SessionsWorkspace {
    func startPersistence(query: RegistryQuery, repoPath: String?) {
        guard persistenceQuery == nil, let repoPath else { return }
        persistenceQuery = query
        persistenceRepo = repoPath
        persistenceTask = Task { [weak self] in await self?.loadPaneIntent() }
    }

    func portableLayout() -> WorkPaneLayout {
        let place = taskFiles.map { files in
            WorkViewPlan.Location(machineID: files.execution?.machineID ?? location?.machineId ?? localMachineId ?? "",
                                  cwd: files.store.cwd)
        }
        return multiplexer.paneLayout(work: key, revision: paneRevision,
            files: taskFiles?.store.browserTab,
            drafts: (place.flatMap { place in taskFiles?.store.drafts(location: place) } ?? []) + unavailableDrafts)
    }

    func loadPaneIntent() async {
        guard let query = persistenceQuery, let repo = persistenceRepo, !persistenceWriting, !isRestoringIntent else { return }
        isRestoringIntent = true
        defer { isRestoringIntent = false }
        let before = portableLayout()
        let hadLocalTools = multiplexer.layout.allPanes.contains {
            switch $0.content {
            case .shell(let tabs, _): !tabs.isEmpty
            case .browser(_, let tabs, _, _, _): !tabs.isEmpty
            case .session(let id, _, _): id != nil
            }
        }
        let preserveCurrent = persistenceLoaded || persistenceError != nil || hadLocalTools
        persistenceError = nil
        do {
            let saved = try await query.paneLayout(work: key.selector, repoPath: repo)
            let needsSave: Bool
            if let saved {
                guard saved.work == key else { throw RegistryQueryError("Saved layout belongs to different Work; local edits retained.") }
                let current = portableLayout()
                let restored = current == before && !preserveCurrent ? saved : Self.merging(saved, with: current)
                multiplexer.restore(restored)
                paneRevision = saved.revision
                unavailableDrafts = restored.drafts
                if case .browser(let place?, let browserTabs, _)? = restored.tree.intent(.browser),
                   case .files(_, let tabs, let selected)? = browserTabs.first(where: { if case .files = $0 { true } else { false } }) {
                    let fileQuery = place.machineID == localMachineId ? query : query.onMachine(place.machineID, repository: key.repository)
                    let documents = files(issue: key.id, cwd: place.cwd, query: fileQuery)
                    documents.restoreTabs(tabs, selected: selected)
                    documents.restoreDrafts(restored.drafts, location: place)
                    unavailableDrafts.removeAll { $0.location == place }
                    // Reconcile every recovered base, including documents with closed tabs.
                    for draft in restored.drafts where draft.location == place {
                        do {
                            let disk = try await fileQuery.taskFile(issue: key.id, path: draft.path)
                            await documents.document(draft.path).reconcile(disk)
                        } catch { persistenceError = error.localizedDescription }
                    }
                }
                needsSave = portableLayout() != saved
            } else {
                needsSave = hadLocalTools
            }
            // Reloads reuse the workspace's existing observation chain.
            if !persistenceLoaded {
                persistenceLoaded = true
                observePaneIntent()
            }
            isRestoringIntent = false
            if needsSave { await savePaneIntent() }
        } catch { persistenceError = error.localizedDescription }
    }

    private func observePaneIntent() {
        withObservationTracking { _ = portableLayout() } onChange: { [weak self] in
            Task { @MainActor [weak self] in
                guard let self else { return }
                self.observePaneIntent()
                guard self.persistenceError == nil, !self.persistenceWriting, !self.isRestoringIntent else { return }
                self.persistenceTask?.cancel()
                self.persistenceTask = Task { [weak self] in
                    do { try await Task.sleep(for: .milliseconds(300)) } catch { return }
                    await self?.savePaneIntent()
                }
            }
        }
    }

    func savePaneIntent() async {
        guard persistenceLoaded, !persistenceWriting, !isRestoringIntent, persistenceError == nil,
              let query = persistenceQuery, let repo = persistenceRepo else { return }
        persistenceWriting = true
        defer { persistenceWriting = false }
        do {
            // Serial writes absorb edits arriving during the previous write without
            // reusing its revision or applying its older content to the workspace.
            while true {
                let submitted = portableLayout()
                let saved = try await query.savePaneLayout(submitted, repoPath: repo)
                let unchanged = portableLayout() == submitted
                paneRevision = saved.revision
                if unchanged { break }
            }
        } catch { persistenceError = error.localizedDescription }
    }

    /// A delayed read contributes missing retained content; current edits own selection and geometry.
    /// Both layouts hold the same designated panes, so merging is per kind.
    private static func merging(_ saved: WorkPaneLayout, with current: WorkPaneLayout) -> WorkPaneLayout {
        let prior = saved.tree
        func merge(_ node: WorkPaneTree) -> WorkPaneTree {
            switch node {
            case .split(let axis, let ratio, let first, let second):
                return .split(axis: axis, ratio: ratio, first: merge(first), second: merge(second))
            case .pane(let id, let content):
                switch content {
                case .session(nil, _):
                    guard case .session(let old?, let place)? = prior.intent(.session) else { return node }
                    return .pane(id: id, content: .session(id: old, location: place))
                case .shell(let tabs, let selected):
                    guard case .shell(let oldTabs, let oldSelected)? = prior.intent(.shell) else { return node }
                    let merged = tabs + oldTabs.filter { old in !tabs.contains { $0.id == old.id } }
                    return .pane(id: id, content: .shell(tabs: merged, selected: selected ?? oldSelected))
                case .browser(let place, let tabs, let selected):
                    guard case .browser(let oldPlace, let oldTabs, let oldSelected)? = prior.intent(.browser),
                          oldPlace == place || place == nil else { return node }
                    var merged = tabs
                    for old in oldTabs {
                        if let index = merged.firstIndex(where: { $0.id == old.id }) {
                            if case .files(let id, let newFiles, let fileSelected) = merged[index],
                               case .files(_, let oldFiles, _) = old {
                                merged[index] = .files(id: id, tabs: newFiles + oldFiles.filter { old in
                                    !newFiles.contains { $0.id == old.id || $0.path == old.path } }, selected: fileSelected)
                            }
                        } else { merged.append(old) }
                    }
                    return .pane(id: id, content: .browser(location: place ?? oldPlace, tabs: merged, selected: selected ?? oldSelected))
                default: return node
                }
            }
        }
        let drafts = current.drafts + saved.drafts.filter { old in
            !current.drafts.contains { $0.path == old.path && $0.location == old.location }
        }
        return WorkPaneLayout(work: current.work, revision: saved.revision, tree: merge(current.tree), focused: current.focused,
                              zoomed: current.zoomed, hidden: current.hidden, drafts: drafts)
    }
}

private extension WorkPaneTree {
    var panes: [(String, WorkPaneIntent)] {
        switch self {
        case .pane(let id, let content): [(id, content)]
        case .split(_, _, let first, let second): first.panes + second.panes
        }
    }
    /// The designated pane of this kind.
    func intent(_ kind: PaneContent.Kind) -> WorkPaneIntent? {
        panes.first { $0.1.kind == kind }?.1
    }
}

private extension WorkPaneIntent {
    var kind: PaneContent.Kind {
        switch self {
        case .session: .session
        case .shell: .shell
        case .browser: .browser
        }
    }
}
