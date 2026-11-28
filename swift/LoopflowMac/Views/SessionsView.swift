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
    let appearance: AppearanceStore
    @Environment(\.undoManager) private var undoManager
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
            Picker("Appearance", selection: Binding(get: { appearance.selection.mode }, set: { mode in
                Task { await appearance.applyMode(mode, manager: undoManager) }
            })) {
                ForEach(AppearanceMode.allCases, id: \.self) { Text($0.menuTitle).tag($0) }
            }
            .disabled(appearance.saving || appearance.editing)
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
            action("details")
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
                let title = args["index"].map { "Task \($0)" } ?? args["direction"]?.capitalized
                    ?? args["page"]?.capitalized ?? action.title
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
        case .session(let id?, let location): location.map { .session(id, machineId: $0.machineId) }
        case .session(nil, _), .browser: nil
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
    /// Whether Desktop opens this conversation without being asked when it shows
    /// no Session yet. Repository and Wave primaries always do; a Task does while it works.
    var opensConversationAutomatically: Bool
    private var didPrepareTaskPanes = false
    private let opensUnvisitedWork: Bool
    private(set) var lastVisit = Date.distantPast
    var canOpenTerminals: Bool { opensUnvisitedWork || lastVisit != .distantPast }

    func visit() { lastVisit = Date() }

    var conversationClosed = false
    var onConversationClosed: (() -> Void)?

    /// Run one conversation opening against this workspace's progress and error.
    func openConversation(_ open: () async throws -> Void) async {
        guard !openingConversation else { return }
        let request = UUID()
        conversationRequest = request
        conversationError = nil
        defer { if conversationRequest == request { conversationRequest = nil } }
        do { try await open() } catch { if !Task.isCancelled { conversationError = error.localizedDescription } }
    }

    /// Whether a pane shows a Session the store holds. A saved layout can name a
    /// headless Run or an aged-out Session the store doesn't hold, and that pane
    /// renders as "Conversation not open".
    func showsConversation(held: Set<String>) -> Bool {
        multiplexer.layout.allPanes.contains { $0.sessionId.map(held.contains) == true }
    }

    let performanceOwner = UUID()
    var measurementRepository: String?

    func toggleFocus(_ paneId: String) {
        Perf.begin(Perf.retainedWorkspaceAction, multiplexer.zoomedPaneId == nil ? "focus" : "restore", id: "workspace", owner: performanceOwner)
        multiplexer.toggleZoom(paneId)
    }
    func setCollapsed(paneId: String, collapsed: Bool) {
        Perf.begin(Perf.retainedWorkspaceAction, collapsed ? "collapse" : "expand", id: "workspace", owner: performanceOwner)
        multiplexer.setCollapsed(paneId: paneId, collapsed: collapsed)
    }

    var key: WorkWorkspaceKey
    var title: String
    var location: WorkspaceIdentity?
    /// The Machine this Mac's plan says runs the Task, known before any checkout is read.
    private(set) var owner: String?
    /// The Task has not started: nothing runs anywhere for warm opening to find.
    var awaitsStart = false

    /// The plan names `next` as the Task's Machine. A move away from the
    /// Machine it named before retires what that Machine's checkout held: the
    /// Session pane's location and viewer, and the Browser's location, which
    /// the next preparation and Files reading supply from the new owner. The
    /// caller retires the old viewer through its Sessions store. Shell tabs
    /// keep the Machine that started them. Returns whether it moved.
    @discardableResult
    func follow(owner next: String?) -> Bool {
        let next = next.map { StructuredIdentity.prefixed($0, "home_") }
        defer { owner = next }
        guard let previous = owner, let next, previous != next else { return false }
        if let location, location.machineId != next { self.location = nil }
        if case .session(let id?, let place?)? = multiplexer.sessionPane?.content, place.machineId != next {
            multiplexer.updateSessionLocation(id, location: nil)
        }
        if case .browser(_, _, _, let place?)? = multiplexer.browserPane?.content, place.machineId != next {
            multiplexer.clearBrowserLocation()
        }
        return true
    }
    var presentation = WorkNavigation.WorkspacePresentation()
    var localMachineId: String?

    /// A Task's first conversation brings its Files into view once.
    func prepareTaskPanes(query: RegistryQuery = RegistryQueryLocal.shared) {
        guard key.kind == .task, !didPrepareTaskPanes,
              let session = multiplexer.sessionPane, session.sessionId != nil, session.content.location != nil,
              case .browser(_, let tabs, _, _)? = multiplexer.browserPane?.content, tabs.isEmpty else { return }
        didPrepareTaskPanes = true
        guard let browser = multiplexer.showBrowser(.files, location: session.content.location,
            focus: false) else { return }
        // A location error is presented by the browser's normal error view.
        _ = try? files(for: browser.content, task: nil, query: query)
    }

    /// Add a shell tab; the server starts it wherever the Task runs, and the
    /// tab keeps that Machine.
    @discardableResult
    func newShell(command: [String]? = nil, focus: Bool = true) throws -> PaneState? {
        guard key.kind == .task else {
            throw RegistryQueryError("Shells belong to Tasks; no shell was opened.")
        }
        // The server starts the shell wherever the Task runs; the tab keeps that
        // Machine for its label and Close.
        guard let place = location ?? owner.map({ WorkspaceIdentity(machineId: $0, worktree: "") }) else {
            throw RegistryQueryError("Where this Task runs is not known yet; no shell was opened.")
        }
        visit()
        return multiplexer.newShell(command: command ?? [], location: place, focus: focus)
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
        if tab == .files { selectFilesPage() }
        return multiplexer.showBrowser(tab, location: place, focus: focus)
    }

    /// ⌘W closes the named tab, else the selected one; a session pane, or one
    /// without tabs, hides. Shell Close ends execution; Undo restores its identity
    /// and history, never a new process.
    func closePane(_ paneId: String, tab: String? = nil) {
        guard let pane = multiplexer.layout.pane(for: paneId) else { return }
        let id = tab ?? pane.content.selectedTabID
        let replaced: ClosedOccurrence?
        if case .shell(let tabs, _) = pane.content, let id, let shell = tabs.first(where: { $0.id == id }) {
            if let started = shell.location {
                // Close ends the shell on the Machine that started it, even after a move.
                do {
                    let argv = try shellCommand(shell) + ["--close", "--started-on", started.machineId]
                    Task { [weak self] in
                        do {
                            _ = try await LocalWaveAgentLauncher.queryLf(Array(argv.dropFirst()), cwd: nil)
                        } catch { self?.conversationError = error.localizedDescription }
                    }
                } catch { conversationError = error.localizedDescription }
            }
            disconnectedShells.insert(id)
            if let terminal = shell.terminal { surfaces.release(terminal) }
            replaced = multiplexer.closeShell(id)
        } else if case .browser(_, let tabs, _, _) = pane.content, let id, tabs.contains(where: { $0.id == id }) {
            replaced = multiplexer.closeBrowserTab(id)
        } else {
            if case .session = pane.content {
                conversationRequest = nil
                conversationClosed = true
                onConversationClosed?()
            }
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
        taskFiles?.page ?? .files
    }

    func selectFilesPage() {
        taskFiles?.page = .files
    }

    /// The local server runs the shell wherever the Task runs.
    private func shellCommand(_ tab: ShellTab) throws -> [String] {
        [try LocalWaveAgentLauncher.machineLf().path, "desktop", "shell", key.id, "--id", tab.id]
    }

    func shellAttachment(_ tab: ShellTab) throws -> [String] {
        var argv = try shellCommand(tab)
        if let initial = tab.command {
            argv += ["--create", "--bootstrap", try desktopShellBootstrap()]
            if !initial.isEmpty { argv += ["--"] + initial }
        }
        return argv
    }

    func reconnectShell(_ tab: ShellTab) {
        guard let terminal = tab.terminal else { return }
        surfaces.release(terminal)
        disconnectedShells.remove(tab.id)
        notStartedShells.remove(tab.id)
    }

    // Exiting an attachment does not establish that its remote execution ended.
    var disconnectedShells: Set<String> = []
    /// Ended shell tabs whose Task has no checkout on its Machine: **Not started**.
    var notStartedShells: Set<String> = []

    /// An ended attachment can be the server refusing a Task with no checkout;
    /// ask it in its typed form rather than reading the terminal's text.
    private func checkStarted(_ tabId: String) {
        guard key.kind == .task else { return }
        let query = retainedStore?.query ?? RegistryQueryLocal.shared
        Task { [weak self, issue = key.id] in
            do {
                _ = try await query.taskFiles(issue: issue, directory: "")
                self?.notStartedShells.remove(tabId)
            } catch let refusal as ServerError where refusal.notStarted {
                self?.notStartedShells.insert(tabId)
            } catch {}
        }
    }
    var shellScreens: [String: String] = [:]

    /// Whether this Task's terminals are shown. An open LF viewer retains its
    /// Task checkout, so departure ends this Work's viewers rather than keeping
    /// them resident; shell and agent execution continue on their Machine.
    private(set) var presented = false

    func present(_ shown: Bool) {
        guard key.kind == .task, presented != shown else { return }
        presented = shown
        if !shown {
            conversationRequest = nil
            detachTerminals()
        }
    }

    private func detachTerminals() {
        for pane in multiplexer.layout.allPanes {
            if case .shell(let tabs, _) = pane.content {
                for tab in tabs { if let terminal = tab.terminal { surfaces.release(terminal) } }
            } else if let terminal = pane.terminal { surfaces.release(terminal) }
        }
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
    private var paneLayouts: PaneLayoutStore?
    private var persistenceLoaded = false
    @ObservationIgnored private var persistenceTask: Task<Void, Never>?
    private var taskFiles: TaskFilesStore?

    /// The Task's one Files store. It reads by Task through the server, so the
    /// Machine is only where it watches; drafts restore whatever their location.
    func files(issue: String, cwd: String, machine: String? = nil, machineLabel: String? = nil,
               query: RegistryQuery = RegistryQueryLocal.shared) -> TaskFilesStore {
        if let files = taskFiles { return files }
        let files = TaskFilesStore(issue: issue, cwd: cwd, machine: machine, machineLabel: machineLabel,
                                   localMachineID: localMachineId, query: query)
        files.measurementRepository = measurementRepository
        files.localRepository = retainedStore?.repoPath ?? measurementRepository
        taskFiles = files
        files.restoreDrafts(unavailableDrafts)
        unavailableDrafts.removeAll()
        return files
    }

    /// Where a browser's Files reads: the Machine (nil for this Mac), its name
    /// and the checkout there; an empty checkout keeps the one already read.
    struct FilesPlace: Equatable {
        let taskId: String
        let machine: String?
        let label: String?
        let cwd: String
    }

    /// Place Files from its content: the Task and the checkout it was read in.
    func filesPlace(for content: PaneContent, task: RoadmapTask?) throws -> FilesPlace {
        guard case .browser(let taskId, _, _, let location) = content else {
            throw RegistryQueryError("The addressed pane is not the browser.")
        }
        return try filesPlace(taskId: taskId, task: task, location: location)
    }

    func filesPlace(taskId: String, task: RoadmapTask?, location: WorkspaceIdentity?) throws -> FilesPlace {
        // This Mac's plan names another owner without its checkout; the
        // served watch reads that checkout from the owner.
        let reading = task?.reference.workspace?.identity
        let owner = task?.ownerMachineId
        let place = reading ?? location
        let machine: String? = if let reading { reading.machineId }
            else if let owner, owner != localMachineId { owner }
            else { place?.machineId }
        guard let machine else {
            if task?.lifecycle == .unstarted { throw TaskNotStarted() }
            throw RegistryQueryError("Where this Task runs is not known yet; open its conversation.")
        }
        return FilesPlace(taskId: task?.runtime?.workId ?? taskId,
                          machine: machine == localMachineId ? nil : machine,
                          label: owner == machine ? task?.runtime?.owner?.machine.label : nil,
                          cwd: (place?.machineId == machine ? place?.worktree : nil) ?? "")
    }

    /// The Task's Files store. `follow` moves a retained store to this content's
    /// place; a view body passes false and follows from `onChange`, because two
    /// windows reading the Task differently would otherwise move it on every render.
    func files(for content: PaneContent, task: RoadmapTask?, query: RegistryQuery,
               follow: Bool = true) throws -> TaskFilesStore {
        let place = try filesPlace(for: content, task: task)
        if let retained = taskFiles {
            if follow { self.follow(place) }
            return retained
        }
        return files(issue: place.taskId, cwd: place.cwd, machine: place.machine, machineLabel: place.label, query: query)
    }

    func follow(_ place: FilesPlace) {
        guard let retained = taskFiles else { return }
        retained.relocate(machine: place.machine, label: place.label, cwd: place.cwd.isEmpty ? retained.cwd : place.cwd)
    }
    let surfaces: GhosttySurfacePool
    let pages = BrowserPages()
    private var surfaceClosed: AnyCancellable?

    private(set) var retainedStore: SessionsStore?

    func sessionStore(repoPath: String, query: RegistryQuery) -> SessionsStore {
        if let retainedStore {
            retainedStore.repoPath = WaveOrigin.resolve(repoPath)
            retainedStore.localMachineId = localMachineId
            return retainedStore
        }
        let store = SessionsStore(repoPath: repoPath, query: query, surfaces: surfaces, work: key.reference)
        store.localMachineId = localMachineId
        retainedStore = store
        return store
    }

    init(key: WorkWorkspaceKey, title: String, location: WorkspaceIdentity?, localMachineId: String?,
         surfaces: GhosttySurfacePool = GhosttySurfacePool(), opensUnvisitedWork: Bool = true) {
        self.opensUnvisitedWork = opensUnvisitedWork
        self.key = key
        self.title = title
        self.location = location
        self.localMachineId = localMachineId
        self.surfaces = surfaces
        opensConversationAutomatically = key.kind != .task
        multiplexer = MultiplexerStore(taskId: key.kind == .task ? key.id : nil)
        // The workspace outlives SessionsView, including while a shell exits
        // on the Work screen or while another repository is selected.
        surfaceClosed = NotificationCenter.default.publisher(for: .ghosttySurfaceClosed)
            .sink { [weak self] notification in
                guard let terminal = notification.object as? TerminalIdentity,
                      case .shell(let tabId, _) = terminal else { return }
                MainActor.assumeIsolated {
                    guard let self, self.multiplexer.shell(tabId)?.terminal == terminal else { return }
                    self.disconnectedShells.insert(tabId)
                    if let output = notification.userInfo?["output"] as? String { self.shellScreens[tabId] = output }
                    self.checkStarted(tabId)
                }
            }
    }
}

/// One repository's retained Work and surfaces, mounted by its current window.
/// Different repositories never share native views or multiplexers.
@MainActor
@Observable
final class SessionsWorkspaceRegistry {
    static let warmConversationLimit = 16
    static let preparationLimit = 2

    // Observable only for environment delivery; lazy creation must not invalidate views.
    @ObservationIgnored private var workspaces: [WorkWorkspaceKey: SessionsWorkspace] = [:]
    let surfaces: GhosttySurfacePool
    private(set) var localMachineId: String?
    private(set) var machineError: String?

    private(set) var recentTasks: [WorkWorkspaceKey] = []
    /// Repository and Wave primaries: always warm outside the Task bound until Close.
    private(set) var warmPrimaries: [WorkWorkspaceKey] = []
    private(set) var restoreCandidates: [WorkWorkspaceKey] = []
    private var completedTasks: Set<WorkWorkspaceKey> = []
    private struct WarmIntent: Codable {
        let recent: [WorkWorkspaceKey]
        let closed: Set<WorkWorkspaceKey>
    }
    private var closedConversations: Set<WorkWorkspaceKey> = []
    /// Tasks the bound or a window Close let go; only a visit admits them again. Off-page
    /// tiles keep workspaces, so without this each refresh would readmit what the
    /// cross-repository bound just evicted.
    private var releasedTasks: Set<WorkWorkspaceKey> = []
    private var warmPersistenceKey: String?
    private let opensUnvisitedWork: Bool
    private var quitting = false
    private var openings: [WorkWorkspaceKey: Task<Void, Never>] = [:]
    private var sessions: SessionsStore?
    var onWarmthChanged: (() -> Void)?
    var canPrepare: (() -> Bool)?
    var onPreparationFinished: (() -> Void)?
    var preparationCount: Int { openings.count }

    func continueOpenings() {
        if let sessions { openWarmConversations(store: sessions) }
    }

    func refreshSession(_ id: String, in workspace: SessionsWorkspace) async {
        await store(for: workspace)?.refresh(id)
    }

    private func store(for workspace: SessionsWorkspace) -> SessionsStore? {
        let repository = WorkWorkspaceKey(repository: workspace.key.repository, kind: .repository, id: workspace.key.repository)
        return workspaces[repository]?.retainedStore
    }

    func evict(_ key: WorkWorkspaceKey) {
        recentTasks.removeAll { $0 == key }
        releasedTasks.insert(key)
        reconcileWarmth()
    }

    /// `runtime` lets prepared Sessions start their terminals before a pane shows
    /// them; a registry without one retains and shows but never launches.
    init(localMachineId: String? = nil, runtime: GhosttyManager? = nil,
         opensUnvisitedWork: Bool = Bundle.main.bundleIdentifier?.hasSuffix(".dev") != true) {
        self.opensUnvisitedWork = opensUnvisitedWork
        self.localMachineId = localMachineId
        surfaces = GhosttySurfacePool(runtime: runtime)
    }

    func loadWarmIntent(repository: String) {
        guard warmPersistenceKey == nil else { return }
        let repository = WorkWorkspaceKey(repository: repository, kind: .repository, id: repository).repository
        let key = "warm-conversations-\(localMachineId ?? "unknown")-\(repository)"
        warmPersistenceKey = key
        if let data = UserDefaults.standard.data(forKey: key),
           let saved = try? JSONDecoder().decode(WarmIntent.self, from: data) {
            closedConversations = saved.closed
            restoreCandidates = Array(saved.recent.filter { $0.repository == repository && $0.kind == .task }.prefix(Self.warmConversationLimit))
        }
    }

    /// Restore saved intent or fill spare warmth from fresh working Tasks. A tile's
    /// workspace was shown, not chosen, so it qualifies on or off its page; Close or
    /// eviction already decided the others. Tiles on screen share the bound and
    /// never spend it: admitting one keeps it warm after its board leaves.
    func prefetch(_ key: WorkWorkspaceKey, title: String, location: WorkspaceIdentity, completed: Bool) {
        guard !presentationClosed, !quitting else { return }
        restoreCandidates.removeAll { $0 == key }
        let warm = Set(recentTasks).union(visibleWork.filter { $0.kind == .task })
        guard !completed, !closedConversations.contains(key), !recentTasks.contains(key), !releasedTasks.contains(key),
              recentTasks.count < Self.warmConversationLimit,
              warm.contains(key) || warm.count < Self.warmConversationLimit else { return }
        _ = workspace(for: key, title: title, location: location)
        recentTasks.append(key)
        reconcileWarmth()
    }

    /// A repository or Wave primary opens on load like any working Task and stays
    /// warm beside the Task bound; only Close leaves it unopened. Each load
    /// refreshes its location. Wave preparation uses the Work identity.
    func prefetchPrimary(_ key: WorkWorkspaceKey, title: String, location: WorkspaceIdentity) {
        guard !presentationClosed, !quitting, !closedConversations.contains(key) else { return }
        _ = workspace(for: key, title: title, location: location)
        guard !warmPrimaries.contains(key) else { return }
        warmPrimaries.append(key)
        reconcileWarmth()
    }

    func updateCompletion(_ key: WorkWorkspaceKey, completed: Bool) {
        guard completedTasks.contains(key) != completed else { return }
        if completed { completedTasks.insert(key); recentTasks.removeAll { $0 == key } }
        else { completedTasks.remove(key) }
        reconcileWarmth()
    }

    private func saveWarmIntent() {
        let pending = restoreCandidates.filter { !recentTasks.contains($0) && !closedConversations.contains($0) }
        let intent = WarmIntent(recent: Array((recentTasks + pending).prefix(Self.warmConversationLimit)), closed: closedConversations)
        guard let key = warmPersistenceKey, let data = try? JSONEncoder().encode(intent) else { return }
        UserDefaults.standard.set(data, forKey: key)
    }

    private func reconcileWarmth() {
        let visible = Set(visibleWork)
        while recentTasks.count > Self.warmConversationLimit, let index = recentTasks.lastIndex(where: { !visible.contains($0) }) {
            releasedTasks.insert(recentTasks.remove(at: index))
        }
        for workspace in workspaces.values {
            let retained = visible.contains(workspace.key) || recentTasks.contains(workspace.key)
                || warmPrimaries.contains(workspace.key)
            if (!retained || workspace.conversationClosed) && (workspace.presented || openings[workspace.key] != nil) {
                openings.removeValue(forKey: workspace.key)?.cancel()
                for pane in workspace.multiplexer.layout.allPanes {
                    if let id = pane.sessionId { sessions?.release(id) }
                }
            }
            workspace.present(workspace.canOpenTerminals && retained && !workspace.conversationClosed)
        }
        saveWarmIntent()
        onWarmthChanged?()
    }

    private func closeConversation(in key: WorkWorkspaceKey) {
        closedConversations.insert(key)
        recentTasks.removeAll { $0 == key }
        warmPrimaries.removeAll { $0 == key }
        restoreCandidates.removeAll { $0 == key }
        reconcileWarmth()
    }

    /// Selected first, then visible Tasks, which the warm limit excepts, then
    /// repository and Wave primaries, then bounded restored, visited and
    /// prefetched Tasks.
    var warmWork: [WorkWorkspaceKey] {
        var keys = [selectedWork].compactMap { $0 }
        for key in visibleWork.filter({ $0.kind == .task }) + warmPrimaries + recentTasks where !keys.contains(key) {
            keys.append(key)
        }
        return keys.filter { workspaces[$0]?.canOpenTerminals == true }
    }

    func openWarmConversations(store: SessionsStore) {
        guard !presentationClosed, !quitting else { return }
        sessions = store
        for key in warmWork {
            guard openings.count < Self.preparationLimit, canPrepare?() != false else { break }
            guard let workspace = workspaces[key], !workspace.conversationClosed, openings[key] == nil,
                  !workspace.openingConversation, workspace.conversationError == nil else { continue }
            let id = workspace.multiplexer.sessionPane?.sessionId
            let existing = id.flatMap { id in store.sessions.first { $0.id == id } }
            if let existing {
                guard existing.state == .pending else { continue }
            } else {
                // A started Task another Machine runs opens there through the
                // server once shown; prefetching stays with checkouts read here.
                let shown = key == selectedWork || visibleWork.contains(key)
                guard workspace.location != nil || (shown && workspace.owner != nil && !workspace.awaitsStart)
                else { continue }
                // Opening creates a missing conversation, so a tile of a Task that
                // is not working waits for a click; a visited Task opens.
                guard id != nil || workspace.opensConversationAutomatically || key == selectedWork
                        || recentTasks.contains(key) else { continue }
            }
            openings[key] = opening(workspace, existing: existing, store: store)
        }
    }

    /// The placeholder's explicit action for a Work shown in place, a primary's
    /// pane or a Dashboard tile. The same opener as automatic opening, so a
    /// failure lands the same error here, and a deliberate press readmits a
    /// closed one.
    func openConversation(in workspace: SessionsWorkspace, store: SessionsStore) async {
        let key = workspace.key
        guard openings[key] == nil, !workspace.openingConversation else { return }
        workspace.visit()
        sessions = store
        if workspace.conversationClosed {
            closedConversations.remove(key)
            workspace.conversationClosed = false
            reconcileWarmth()
        }
        let id = workspace.multiplexer.sessionPane?.sessionId
        let existing = id.flatMap { id in store.sessions.first { $0.id == id } }
        let opening = opening(workspace, existing: existing, store: store)
        openings[key] = opening
        await opening.value
    }

    /// One opening in the preparation slots.
    private func opening(_ workspace: SessionsWorkspace, existing: SessionItem?,
                         store: SessionsStore) -> Task<Void, Never> {
        let key = workspace.key
        return Task { [weak self, weak workspace] in
            guard let self, let workspace else { return }
            defer {
                if !Task.isCancelled {
                    self.openings[key] = nil
                    self.openWarmConversations(store: store)
                    self.onPreparationFinished?()
                }
            }
            await workspace.openConversation {
                try await self.openWarmConversation(in: workspace, existing: existing, store: store)
            }
        }
    }

    /// Prepare the conversation through the server, which answers it from
    /// wherever the Task runs; its record says where that is.
    private func openWarmConversation(in workspace: SessionsWorkspace, existing: SessionItem?,
                                      store: SessionsStore) async throws {
        if let existing {
            await store.select(existing.id)
            return
        }
        let key = workspace.key
        let record: SessionRecord
        if let id = workspace.multiplexer.sessionPane?.sessionId {
            record = try await store.query.prepareSession(id: id, work: key.reference, cwd: store.repoPath)
        } else if key.kind != .task {
            record = try await store.query.ensurePrimarySession(wave: key.kind == .wave ? key.id : nil, cwd: store.repoPath)
        } else {
            record = try await store.query.prepareTaskSession(issue: key.id, cwd: store.repoPath)
        }
        try Task.checkCancellation()
        guard !workspace.conversationClosed else { return }
        if key.kind == .task, let place = record.workspace?.identity { workspace.location = place }
        try Task.checkCancellation()
        if let existing {
            await store.select(existing.id)
        } else {
            store.reconcile([record])
            store.adoptPreparation(record)
            // Its Session frame can arrive first; the person may have moved on since.
            if workspace.multiplexer.pane(forSessionId: record.id) == nil { presentSession(record, in: workspace) }
        }
    }

    func sessionStore(repository: String, repoPath: String, query: RegistryQuery) -> SessionsStore {
        loadWarmIntent(repository: repository)
        let path = WaveOrigin.resolve(repoPath)
        let root = WorkWorkspaceKey(repository: repository, kind: .repository, id: repository)
        let store = workspace(for: root, title: URL(fileURLWithPath: path).lastPathComponent,
            location: localMachineId.map { WorkspaceIdentity(machineId: $0, worktree: path) })
            .sessionStore(repoPath: path, query: query)
        store.onPrepared = { [weak self] record in self?.locate(record) }
        return store
    }

    /// Every pane showing `record`'s Session follows it to where it runs now.
    func locate(_ record: SessionRecord) {
        for workspace in workspaces.values {
            workspace.multiplexer.updateSessionLocation(record.id, location: record.workspace?.identity)
        }
    }

    func refreshMachine(query: RegistryQuery) async {
        do {
            localMachineId = try await query.localMachineId()
            for workspace in workspaces.values {
                workspace.localMachineId = localMachineId
                workspace.retainedStore?.localMachineId = localMachineId
            }
            machineError = nil
        }
        catch { machineError = error.localizedDescription }
    }

    func clearPresentation() {
        surfaces.clearFocus()
        openWork = []
        shownWork = []
        selectedWork = nil
        reconcileWarmth()
    }

    func prepareForQuit() {
        quitting = true
        for task in openings.values { task.cancel() }
        openings.removeAll()
        surfaces.clearFocus()
    }

    /// Closing the window ends its terminal viewers; Work and history remain.
    func closePresentation() {
        guard !quitting else { return }
        presentationClosed = true
        releasedTasks.formUnion(recentTasks)
        recentTasks = []
        restoreCandidates = []
        saveWarmIntent()
        for task in openings.values { task.cancel() }
        openings.removeAll()
        for workspace in workspaces.values { workspace.present(false) }
        surfaces.clearFocus()
        onPreparationFinished?()
    }

    func present(open: [WorkWorkspaceKey], shown: [WorkWorkspaceKey]) {
        presentationClosed = false
        openWork = open
        shownWork = shown
        if selectedWork.map({ openWork.contains($0) }) != true {
            selectedWork = openWork.first
            surfaces.clearFocus()
        }
        reconcileWarmth()
    }

    /// The window's Work in slot fill order, every board page included: readback,
    /// not an arrangement owner.
    private(set) var openWork: [WorkWorkspaceKey] = []
    /// The part of `openWork` on the current page.
    private var shownWork: [WorkWorkspaceKey] = []
    private var presentationClosed = false
    /// Work shown in the window. A selection a page turn left behind keeps its viewer.
    var visibleWork: [WorkWorkspaceKey] {
        guard !presentationClosed else { return [] }
        guard let selectedWork, !shownWork.contains(selectedWork) else { return shownWork }
        return shownWork + [selectedWork]
    }

    private(set) var selectedWork: WorkWorkspaceKey?

    var keys: [WorkWorkspaceKey] { Array(workspaces.keys) }

    func retained(_ key: WorkWorkspaceKey) -> SessionsWorkspace? { workspaces[key] }

    @discardableResult
    func open(_ key: WorkWorkspaceKey, title: String, location: WorkspaceIdentity?, deliberate: Bool = true) -> SessionsWorkspace {
        let workspace = workspace(for: key, title: title, location: location)
        openWork = [key]
        shownWork = [key]
        focus(key, deliberate: deliberate)
        return workspace
    }

    func focus(_ key: WorkWorkspaceKey, deliberate: Bool = true) {
        guard openWork.contains(key) else { return }
        presentationClosed = false
        if selectedWork != key { surfaces.clearFocus() }
        selectedWork = key
        if let terminal = workspaces[key]?.multiplexer.sessionPane?.terminal { surfaces.scrollToBottom(terminal) }
        if deliberate || opensUnvisitedWork { workspaces[key]?.visit() }
        if deliberate { closedConversations.remove(key) }
        if deliberate, let workspace = workspaces[key], workspace.conversationClosed {
            workspace.conversationClosed = false
            if let id = workspace.multiplexer.sessionPane?.sessionId {
                workspace.multiplexer.reveal(sessionId: id, location: workspace.location)
            }
        }
        if key.kind == .task, !completedTasks.contains(key), !closedConversations.contains(key) {
            recentTasks.removeAll { $0 == key }
            recentTasks.insert(key, at: 0)
            releasedTasks.remove(key)
        } else if key.kind != .task, deliberate, !warmPrimaries.contains(key) {
            warmPrimaries.append(key)
        }
        reconcileWarmth()
        if let sessions { openWarmConversations(store: sessions) }
    }

    func inspect() -> [DesktopWorkspaceInspection] {
        workspaces.values.sorted { ($0.key.repository, $0.key.kind.rawValue, $0.key.id) < ($1.key.repository, $1.key.kind.rawValue, $1.key.id) }.map { workspace in
            let panes = workspace.multiplexer
            let repository = WorkWorkspaceKey(repository: workspace.key.repository, kind: .repository, id: workspace.key.repository)
            let sessions = workspaces[repository]?.retainedStore
            return DesktopWorkspaceInspection(work: workspace.key, title: workspace.title,
                isOpen: openWork.contains(workspace.key), selected: selectedWork == workspace.key,
                layout: DesktopLayoutInspection(panes.layout,
                    surface: { content in content.terminal.flatMap { surfaces.surfaceIncarnation(for: $0) } },
                    commands: { content in content.terminal.flatMap { surfaces.commandStatus(for: $0) } },
                    attachment: { id in sessions?.attachment(id) ?? .failed }),
                focusedPane: panes.focusedPaneId,
                zoomedPane: panes.zoomedPaneId, hiddenPanes: panes.collapsedPaneIds.sorted())
        }
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
            case .browser(_, let tabs, _, _):
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
    @discardableResult
    func reconcileMembership(_ records: [SessionRecord]) -> WorkReference? {
        var selection: WorkReference?
        for record in records {
            guard let localMachineId, record.workspace?.machineId == localMachineId,
                  record.work?.kind == .task, let task = record.work?.id,
                  let sourceKey = key(containingSession: record.id),
                  let source = retained(sourceKey) else { continue }
            let destination = WorkWorkspaceKey(repository: sourceKey.repository, kind: .task, id: task)
            guard sourceKey != destination else { continue }
            let target = workspace(for: destination, title: record.title, location: record.workspace?.identity)
            revealSession(record, in: target)
            target.presentation.selectedSessionId = record.id
            target.presentation.content = .terminals
            if source.canOpenTerminals, selectedWork == sourceKey, source.presentation.selectedSessionId == record.id {
                selection = destination.reference
            }
            source.presentation.selectedSessionId = nil
        }
        for workspace in workspaces.values {
            for record in records {
                let belongs: Bool?
                switch workspace.key.kind {
                case .task:
                    belongs = record.taskIds.isEmpty ? nil : record.names(task: workspace.key.id)
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
        return selection
    }

    /// One native occurrence per repository. A deliberate eligible Work opening can
    /// relocate presentation, but neither copies a surface nor binds the Session.
    func revealSession(_ record: SessionRecord, in workspace: SessionsWorkspace) {
        for other in workspaces.values where other !== workspace {
            other.multiplexer.removeSessions([record.id])
        }
        workspace.multiplexer.reveal(sessionId: record.id, location: record.workspace?.identity)
    }

    func presentSession(_ record: SessionRecord, in workspace: SessionsWorkspace) {
        revealSession(record, in: workspace)
        workspace.presentation.selectedSessionId = record.id
        workspace.presentation.content = .terminals
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
        shownWork = shownWork.map { $0 == previous ? key : $0 }
        recentTasks = recentTasks.map { $0 == previous ? key : $0 }
        warmPrimaries = warmPrimaries.map { $0 == previous ? key : $0 }
        restoreCandidates = restoreCandidates.map { $0 == previous ? key : $0 }
        if closedConversations.remove(previous) != nil { closedConversations.insert(key) }
        if completedTasks.remove(previous) != nil { completedTasks.insert(key) }
        if releasedTasks.remove(previous) != nil { releasedTasks.insert(key) }
        saveWarmIntent()
        if selectedWork == previous { selectedWork = key }
    }

    func workspace(for key: WorkWorkspaceKey, title: String, location: WorkspaceIdentity?) -> SessionsWorkspace {
        if let existing = workspaces[key] {
            if existing.title != title { existing.title = title }
            if let location, existing.location != location { existing.location = location }
            return existing
        }
        let workspace = SessionsWorkspace(key: key, title: title, location: location,
            localMachineId: localMachineId, surfaces: surfaces, opensUnvisitedWork: opensUnvisitedWork)
        workspace.conversationClosed = closedConversations.contains(key)
        workspace.onConversationClosed = { [weak self, weak workspace] in
            guard let workspace else { return }
            self?.closeConversation(in: workspace.key)
        }
        workspaces[key] = workspace
        return workspace
    }

}

struct SessionItem: Identifiable, Equatable {
    enum State: Equatable {
        case pending
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
    private(set) var lastScreen: String?

    mutating func retainScreen(_ screen: String?) {
        guard let screen else { return }
        lastScreen = screen.contains(where: { !$0.isWhitespace }) ? screen : nil
    }

    var attachment: DesktopOpening.Status {
        switch state {
        case .opening, .prepared: .opening
        case .live: .usable
        case .pending, .failed, .ended: .failed
        }
    }

    var id: String { record.id }
    var presentationProvider: String? { agentOverride ?? record.provider }

    var launchArguments: [String] {
        record.openArgv + (agentOverride.map { ["--agent", $0] } ?? [])
    }

    var surface: SessionRecord? {
        switch state {
        case .prepared, .live: record
        case .pending, .opening, .failed, .ended: nil
        }
    }

    var error: String? {
        guard case .failed(let message) = state else { return nil }
        return message
    }

    mutating func adoptPreparation(_ record: SessionRecord) {
        self.record = record
        // Nothing to start: retained work attaches when shown.
        state = record.openArgv.isEmpty ? .pending : .prepared
    }
}

/// A Task with no checkout anywhere yet: its views say **Not started**.
struct TaskNotStarted: LocalizedError {
    var errorDescription: String? { "Not started" }
}

@MainActor
@Observable
final class SessionsStore {
    private(set) var sessions: [SessionItem] = []

    let surfaces: GhosttySurfacePool

    var repoPath: String
    let query: RegistryQuery
    /// The Task or Wave whose workspace holds this store; none for a repository.
    let work: WorkReference?
    /// This Mac, once the workspace registry has read it.
    var localMachineId: String?
    /// A preparation answered where its Session runs now; a moved Session keeps
    /// its ID, so every pane showing it follows the record.
    var onPrepared: ((SessionRecord) -> Void)?

    /// A terminal starts in the Session's checkout when it is on this Mac; a
    /// viewer of another Machine's Session starts in the repository here.
    func presentationDirectory(for record: SessionRecord, checkout: String) -> String {
        record.workspace?.machineId == localMachineId ? checkout : repoPath
    }

    @discardableResult
    func release(_ id: String) -> Task<Void, Never>? {
        guard let index = _index(id) else { return nil }
        var retirement: Task<Void, Never>?
        if let machine = sessions[index].record.workspace?.machineId {
            // Shells retain their own lifetime, including agents launched inside them.
            let terminal = TerminalIdentity.session(id, machineId: machine)
            sessions[index].retainScreen(surfaces.screen(terminal))
            retirement = surfaces.release(terminal)
        }
        sessions[index].state = .pending
        sessions[index].launchId = UUID()
        return retirement
    }

    private let metrics: SessionsLatencyMetrics
    @ObservationIgnored private var hasRecordedSessionsLoad = false

    init(
        repoPath: String,
        query: RegistryQuery = RegistryQueryLocal.shared,
        surfaces: GhosttySurfacePool = GhosttySurfacePool(),
        work: WorkReference? = nil
    ) {
        self.repoPath = WaveOrigin.resolve(repoPath)
        self.query = query
        self.surfaces = surfaces
        self.work = work
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
            let observedState: SessionItem.State = localTerminal(for: record) != nil ? .live : .pending
            if let index = next.firstIndex(where: { $0.id == record.id }) {
                // Keep the command and connecting state until the relay confirms input.
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
                if observedState == .live {
                    // A retained surface does not repair its failed attachment.
                    switch next[index].state {
                    case .failed, .ended: continue
                    default: break
                    }
                }
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

    /// This Mac's server knows a Session born on another Machine only through
    /// its Task, so a repository-wide store names it for those.
    private func preparationWork(_ record: SessionRecord) -> WorkReference? {
        guard work == nil, let local = localMachineId, let machine = record.workspace?.machineId, machine != local,
              let task = record.taskIds.first else { return work }
        return WorkReference(kind: .task, id: task)
    }

    private func prepare(_ id: String) async {
        guard let index = _index(id) else { return }
        switch sessions[index].state {
        case .pending, .failed, .ended:
            sessions[index].state = .opening
        case .opening, .prepared, .live:
            return
        }
        // Release, reset and provider switches invalidate preparation and its surface together.
        let generation = sessions[index].launchId
        do {
            Perf.agentInput("connect_prepare_begin", session: id)
            let surface = try await query.prepareSession(id: id, work: preparationWork(sessions[index].record), cwd: repoPath)
            try Task.checkCancellation()
            Perf.agentInput("connect_prepare_returned", session: id)
            guard let latest = _index(id), sessions[latest].launchId == generation else { return }
            if case .opening = sessions[latest].state {
                sessions[latest].adoptPreparation(surface)
                onPrepared?(surface)
                startTerminal(id)
            }
        } catch {
            guard let latest = _index(id), sessions[latest].launchId == generation else { return }
            sessions[latest].state = Task.isCancelled ? .pending : .failed(error.localizedDescription)
        }
    }

    func adoptPreparation(_ record: SessionRecord) {
        onPrepared?(record)
        guard let index = _index(record.id) else { return }
        if sessions[index].state != .pending, attachment(record.id) == .failed { release(record.id) }
        guard localTerminal(for: record) == nil else { return }
        switch sessions[index].state {
        case .pending, .failed, .ended:
            sessions[index].adoptPreparation(record)
            startTerminal(record.id)
        case .opening, .prepared, .live:
            break
        }
    }

    /// A prepared Session's terminal starts at once, shown or not: warm Work runs
    /// before any pane mounts, and the pane later mounted adopts the running view.
    private func startTerminal(_ id: String) {
        guard let index = _index(id), case .prepared = sessions[index].state else { return }
        let item = sessions[index]
        guard let workspace = item.record.workspace, workspace.unavailable == nil else { return }
        surfaces.start(TerminalIdentity.session(id, machineId: workspace.machineId),
            workingDirectory: presentationDirectory(for: item.record, checkout: workspace.worktree),
            argv: item.launchArguments,
            onSurfaceCreated: { [weak self] in
                Perf.agentInput("surface_created", session: id)
                self?.beginPaneLoad(id)
            },
            onSurfaceFailed: { [weak self] reason in
                self?.recordPaneFailure(item, reason: reason)
            })
    }

    /// Open the Session here: view its held terminal, resume it, or take it over
    /// from a native client, as `lf resume` does.
    func select(_ id: String) async {
        guard let index = _index(id) else { return }
        // A failed viewer retires before its replacement starts.
        if sessions[index].state != .pending, attachment(id) == .failed {
            await refresh(id)
            return
        }
        if localTerminal(for: sessions[index].record) != nil {
            switch sessions[index].state {
            case .failed, .ended:
                release(id)
            case .opening, .prepared, .live:
                return
            case .pending:
                // An agent in a retained shell owns its own connection.
                if case .shell = localTerminal(for: sessions[index].record) {
                    sessions[index].state = .live
                    return
                }
                release(id)
            }
        }
        if case .live = sessions[index].state { sessions[index].state = .pending }
        await prepare(id)
    }

    /// Replace this pane's viewer and open the same conversation again.
    func refresh(_ id: String) async {
        guard let index = _index(id) else { return }
        let retirement = release(id)
        let generation = sessions[index].launchId
        sessions[index].state = .opening
        await retirement?.value
        guard let current = _index(id), sessions[current].launchId == generation else { return }
        sessions[current].state = .pending
        guard !Task.isCancelled else { return }
        await select(id)
    }

    @discardableResult
    func switchProvider(_ id: String, to agent: String) async throws -> SessionRecord {
        guard let index = _index(id) else { throw RegistryQueryError("Session is unavailable.") }
        let previous = sessions[index]
        // Preparation checks the requested provider's continuation before releasing
        // the retained terminal. Failed reads leave current drafts and updates intact.
        let prepared = try await query.prepareSession(id: id, work: preparationWork(previous.record), agent: agent, cwd: repoPath)
        guard let current = _index(id) else { throw RegistryQueryError("Session is unavailable.") }
        if let terminal = localTerminal(for: previous.record) { surfaces.release(terminal) }
        sessions[current].record = prepared
        onPrepared?(prepared)
        sessions[current].agentOverride = agent
        sessions[current].launchId = UUID()
        sessions[current].state = .prepared
        startTerminal(id)
        return prepared
    }

    func reset(_ id: String) async throws -> SessionRecord {
        guard let index = _index(id) else { throw RegistryQueryError("Session is unavailable.") }
        let previous = sessions[index]
        // The same Session continues on a new thread; its old terminal ends.
        let requestID = UUID().uuidString
        let restarted = try await query.resetSession(
            previous.record, work: preparationWork(previous.record), agent: previous.record.provider,
            cwd: repoPath, requestID: requestID)
        guard let current = _index(id) else { throw RegistryQueryError("Session is unavailable.") }
        if let terminal = localTerminal(for: previous.record) { surfaces.release(terminal) }
        sessions[current].record = restarted
        onPrepared?(restarted)
        CreationRecovery.shared.awaitingPresentation(requestID, of: .session(restarted.id))
        sessions[current].launchId = UUID()
        sessions[current].state = .prepared
        startTerminal(id)
        return restarted
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

    func connectionReading(_ id: String) -> ServerConnectionReading? {
        guard let item = sessions.first(where: { $0.id == id }),
              let machine = item.record.workspace?.machineId else { return nil }
        return surfaces.connectionReading(for: .session(id, machineId: machine))
    }

    func attachment(_ id: String) -> DesktopOpening.Status? {
        guard let item = sessions.first(where: { $0.id == id }) else { return nil }
        switch item.state {
        case .failed, .ended: return .failed
        default: return connectionReading(id)?.attachment ?? item.attachment
        }
    }

    func recordPaneLive(_ id: String) {
        Perf.agentInput("input_ready", session: id)
        metrics.recordPaneLive(id)
        if let index = _index(id), case .prepared = sessions[index].state {
            sessions[index].state = .live
        }
    }

    func recordPaneFailure(_ item: SessionItem, reason: String) {
        guard let index = _index(item.id), sessions[index].launchId == item.launchId,
              sessions[index].state == .prepared else { return }
        sessions[index].state = .failed(reason)
    }

    /// Viewer exit does not establish provider exit. Keep its output and require
    /// a deliberate retry; inventory refresh must not resume execution.
    func noteSurfaceClosed(_ terminal: TerminalIdentity, output: String? = nil) {
        guard case .session(let id, let machineId) = terminal,
              !surfaces.hasSurface(terminal),
              let index = _index(id), sessions[index].record.workspace?.machineId == machineId
        else { return }
        sessions[index].retainScreen(output)
        switch sessions[index].state {
        case .prepared, .live:
            sessions[index].state = .ended(output ?? "Terminal output is unavailable.")
        case .pending, .opening, .failed, .ended:
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
                        RepositoryDidNotOpen(error: error) { Task { await readMachine() } }
                            .task(id: taskPage) {
                                if let request = taskPage { model.finishTaskPage(request, error: error) }
                            }
                    } else {
                        ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                }
            }
        }
        .task { await readMachine() }
        .answeringLostLines(model)
    }

    /// The Machine and repository this window stands on; Retry reads them again.
    private func readMachine() async {
        await workspaces.refreshMachine(query: query)
        if repository == nil {
            do {
                resolvedRepository = try await query.repositoryIdentity(cwd: repoPath).id
                repositoryError = nil
            } catch { repositoryError = error.localizedDescription }
        }
        if let home = workspaces.localMachineId, workspaces.machineError == nil { model.confirmMachine(home) }
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
                if model.sessions.failure == nil {
                    if let selection = workspaces.reconcileMembership(records) { model.select(selection) }
                    model.openWarmConversations()
                }
            }
            .onChange(of: model.roadmap.value) { _, _ in
                guard model.repoPath?.normalizedFilePath == store.repoPath.normalizedFilePath,
                      model.roadmap.failure == nil, model.sessions.failure == nil else { return }
                model.openWarmConversations()
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
        workspaces.workspace(for: currentKey, title: title(for: currentKey), location: location(for: currentKey))
    }
    /// One reading per workspace: the getter above and `workspacePanel` both
    /// write it during a render, so they must agree or each undoes the other.
    private func location(for key: WorkWorkspaceKey) -> WorkspaceIdentity? {
        guard key.kind == .task else { return rootIdentity }
        if let task = model.task(id: key.id) { return model.taskWorkspaceIdentity(task.task) }
        return key == currentKey ? taskIdentity : nil
    }
    @Bindable var model: WorkModel
    private let workspaces: SessionsWorkspaceRegistry
    private let query: RegistryQuery
    private let store: SessionsStore
    @State private var actionTaskID: String?
    @Environment(\.palette) private var palette

    private var multiplexer: MultiplexerStore {
        workspace.multiplexer
    }
    private var taskIdentity: WorkspaceIdentity? {
        if let task = fileTask {
            return model.taskWorkspaceIdentity(task.task)
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
            && showsTerminals(currentKey, fileTask?.task, workspace)
    }

    /// Every primary shows terminals; a Task's show once it is working or once a
    /// person chose them. A finished Task shows its record until its conversation
    /// opens; who owns it says nothing about what there is to show.
    private func showsTerminals(_ key: WorkWorkspaceKey, _ task: RoadmapTask?, _ entry: SessionsWorkspace) -> Bool {
        key.kind != .task || task?.opensConversationAutomatically == true || entry.presentation.content == .terminals
    }
    private var fileTask: TaskProjection? {
        // A departing repository view must not mount the next repository's
        // retained terminals while SwiftUI replaces its hierarchy.
        guard model.repoPath?.normalizedFilePath == store.repoPath.normalizedFilePath else { return nil }
        return model.breadcrumb?.task
    }

    private var mainPane: some View {
        ZStack {
            if let starting = navigation.startingTask {
                StartingTaskView(model: model, starting: starting)
            } else if navigation.showsSelection {
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
                ChooseWork()
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
                            model.newTaskSession(id, open: newTaskSession)
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
                VStack(alignment: .leading, spacing: Spacing.sm) {
                    TaskActionItems(model: model, task: found.task, wave: found.wave?.wave,
                        work: model.taskWork[id].value,
                        newConversation: {
                            model.newTaskSession(id, open: newTaskSession)
                        }, openSession: openSession)
                }.padding(Spacing.md).onExitCommand { actionTaskID = nil }
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
                WorkflowCatalogInspector(entry: model.graphCatalog.value?.first { $0.name == name }, model: model)
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
        .modifier(TaskAbandonConfirmation(model: model))
        .modifier(TaskCompleteConfirmation(model: model))
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

        .onReceive(NotificationCenter.default.publisher(for: .ghosttySessionReady)) { notification in
            guard let terminal = notification.object as? TerminalIdentity,
                  case .session(let id, _) = terminal,
                  notification.userInfo?["surface"] as? String == store.surfaces.surfaceIncarnation(for: terminal) else { return }
            store.recordPaneLive(id)
        }
        .onReceive(NotificationCenter.default.publisher(for: .ghosttySurfaceClosed)) { notification in
            guard let terminal = notification.object as? TerminalIdentity else { return }
            store.noteSurfaceClosed(terminal, output: notification.userInfo?["output"] as? String)
        }
        .onReceive(NotificationCenter.default.publisher(for: .openSessions)) { _ in
            navigation.content = .terminals
        }
        .accessibilityIdentifier("sessions-surface")
    }

    private func focusPane(_ pane: String, session: String?, in workspace: SessionsWorkspace) {
        if currentKey != workspace.key || !workspace.canOpenTerminals { model.select(workspace.key.reference) }
        workspace.multiplexer.setFocusedPane(pane)
        // Companion shells retain the current conversation context.
        guard let session, navigation.selectedSessionId != session else { return }
        navigation.selectedSessionId = session
        navigation.content = .terminals
    }

    private func workspacePanel(_ key: WorkWorkspaceKey) -> some View {
        let task = key.kind == .task ? model.task(id: key.id) : nil
        let entry = workspaces.workspace(for: key, title: title(for: key), location: location(for: key))
        let active = currentKey == key
        let terminals = showsTerminals(key, task?.task, entry)
        return VStack(spacing: 0) {
            if entry.isRestoringIntent { ProgressView("Restoring workspace") }
            if let error = entry.persistenceError {
                FailureLine("Layout not saved.", machine: error) {
                    Button("Reload saved intent") { Task { await entry.loadPaneIntent() } }
                }.padding(Spacing.sm)
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
            } else if active, navigation.waveView == .repository, model.repositoryIsEmpty(entry) {
                NoPlannedWork(model: model) { Task { await model.openConversation(in: entry, store: store) } }
            } else if key.kind != .task, active, let view = navigation.waveView {
                // A Wave view, the repository page included: the conversation and the
                // backlog share the top band; the Tasks run along the bottom.
                WaveViewLayout(model: model, view: view, workspace: entry) {
                    conversationPanes(key, entry, isFocused: navigation.palette == nil)
                } backlog: {
                    BacklogView(model: model, view: view)
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
                Button("Open \(entry.title)") { model.select(key.reference) }.buttonStyle(LoopflowButtonStyle())
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .clipShape(PaneGeometry.workspaceShape)
        .accessibilityIdentifier("work-workspace-\(key.id)")
        .modifier(CreationPresentation(recovery: .shared, resource: .task(key.id), visible: active && task != nil))
        .onChange(of: task.map { $0.task.isCompleted }, initial: true) { _, completed in
            if let completed { workspaces.updateCompletion(key, completed: completed) }
        }
    }

    /// A workspace's panes, with a retry above them when its conversation
    /// failed to open and no pane says so itself.
    private func conversationPanes(_ key: WorkWorkspaceKey, _ entry: SessionsWorkspace, isFocused: Bool) -> some View {
        WorkspacePanesView(workspace: entry, isFocused: isFocused, sessions: store, model: model,
            onFocus: { pane, session in focusPane(pane, session: session, in: entry) })
            .equatable()
            .overlay(alignment: .top) {
                if let error = entry.conversationError,
                   !ConversationPlaceholder.namesFailure(in: entry, model: model, sessions: store) {
                    FailureLine("The conversation did not open.", machine: error) {
                        Button("Retry") { Task { await model.openConversation(in: entry, store: store) } }
                    }.padding(Spacing.sm).background(theme.terminal.background.color)
                }
            }
    }

    @ViewBuilder
    private var missingCheckout: some View {
        if let task = fileTask, task.task.reference.workspace?.localExists == false,
           navigation.preparedTaskWorktrees[task.task.id] == nil {
            HStack(spacing: Spacing.sm) {
                if task.task.isCompleted {
                    Text("Recorded checkout is missing. This Task is \(task.task.lifecycle.label); it cannot be restored.")
                } else {
                    Text("Recorded checkout is missing")
                    Button("Restore checkout") { prepareTaskWorkspace() }.buttonStyle(LoopflowButtonStyle())
                }
                Spacer()
            }
            .font(Typography.meta).foregroundStyle(palette.textSecondary)
            .padding(.horizontal, Spacing.md).padding(.vertical, Spacing.xs)
            .accessibilityIdentifier("work-missing-checkout")
        }
    }

    private func prepareTaskWorkspace() {
        guard let task = fileTask else { return }
        let notices = navigation.notices
        Task { @MainActor in
            do {
                let repo = store.repoPath
                let issue = task.task.task.identifier
                let path = try await LocalWaveAgentLauncher.checkoutTask(repoPath: repo, issue: issue)
                navigation.preparedTaskWorktrees[task.task.id] = path
            } catch {
                notices.post(Notice("\(task.task.task.identifier) Could not prepare checkout: \(error.localizedDescription)"))
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
        case .sortWorkingTasks:
            model.toggleSidebarSortByStatus()
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

    /// An independent conversation with Task context in the Task's checkout.
    /// Resolving the checkout prepares Task Work only; it never starts a
    /// Graph or replaces another Session.
    private func newTaskSession(_ taskId: String) async throws {
        guard let found = model.task(id: taskId) else { return }
        let origin = navigation
        let key = key(for: .task(id: taskId))
        let entry = workspaces.workspace(for: key, title: found.task.task.name, location: found.task.reference.workspace?.identity)
        let issue = found.task.task.identifier
        let lf: String
        let checkout: WorkspaceIdentity
        if let owner = found.task.ownerMachineId, owner != StructuredIdentity.prefixed(machineId, "home_") {
            // The server starts the shell where the Task runs, and that
            // Machine's own `lf` opens the conversation; nothing checks out here.
            lf = "lf"
            checkout = WorkspaceIdentity(machineId: owner, worktree: "")
        } else {
            if let workspace = found.task.reference.workspace, workspace.localExists == true, let identity = workspace.identity {
                checkout = identity
            } else {
                checkout = try await LocalWaveAgentLauncher.checkoutTask(repoPath: store.repoPath, issue: issue)
                origin.preparedTaskWorktrees[taskId] = checkout
            }
            lf = try LocalWaveAgentLauncher.machineLf().path
            guard checkout.machineId == machineId else { throw RegistryQueryError("Open a conversation on its owning Machine") }
            // The captured entry receives a late preparation reply without changing selection.
            entry.location = checkout
        }
        entry.presentation.content = .terminals
        entry.visit()
        if let pane = entry.multiplexer.newShell(command: TaskConversationLaunch(task: issue).arguments(lf: lf), location: checkout),
           let tab = pane.content.selectedShell {
            Perf.retarget(.newConversation, from: taskId, to: tab.id, owner: origin.performanceOwner)
        }
    }

    /// Sessions attached to the focused pane: a Session pane's own record, or
    /// every conversation running inside a shell pane.
    private var focusedPaneSessions: [SessionItem] {
        let pane = multiplexer.focusedPane
        switch pane.content {
        case .shell:
            guard let terminal = pane.terminal else { return [] }
            return store.sessions.filter { store.localTerminal(for: $0.record) == terminal }
        case .session(let id?, _):
            return store.sessions.filter { $0.id == id }
        case .session(nil, _), .browser:
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
        navigation.notices.post(Notice("Could not complete the action: \(error.localizedDescription)"))
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
    /// A Work created in this window has no saved layout: it starts saving
    /// edits without reading one.
    func startNewLayout(layouts: PaneLayoutStore, query: RegistryQuery) {
        guard persistenceQuery == nil, localMachineId != nil else { return }
        persistenceQuery = query
        paneLayouts = layouts
        persistenceLoaded = true
        observePaneIntent()
    }

    /// Restore this Mac's saved layout for the Work, then save each change to it.
    func startPersistence(layouts: PaneLayoutStore, query: RegistryQuery) {
        guard persistenceQuery == nil, localMachineId != nil else { return }
        persistenceQuery = query
        paneLayouts = layouts
        persistenceTask = Task { [weak self] in await self?.loadPaneIntent() }
    }

    func portableLayout() -> WorkPaneLayout {
        let place = taskFiles.map { files in
            WorkViewPlan.Location(machineID: files.machine ?? localMachineId ?? "", cwd: files.cwd)
        }
        return multiplexer.paneLayout(files: taskFiles?.browserTab,
            drafts: (place.flatMap { place in taskFiles?.drafts(location: place) } ?? []) + unavailableDrafts)
    }

    func loadPaneIntent() async {
        guard let query = persistenceQuery, let layouts = paneLayouts, let machine = localMachineId, !isRestoringIntent else { return }
        isRestoringIntent = true
        defer { isRestoringIntent = false }
        let before = portableLayout()
        let hadLocalTools = multiplexer.layout.allPanes.contains {
            switch $0.content {
            case .shell(let tabs, _): !tabs.isEmpty
            case .browser(_, let tabs, _, _): !tabs.isEmpty
            case .session(let id, _): id != nil
            }
        }
        let preserveCurrent = persistenceLoaded || persistenceError != nil || hadLocalTools
        persistenceError = nil
        let saved = layouts.load(machineId: machine, work: key)
        let needsSave: Bool
        if let saved {
            let current = portableLayout()
            let restored = current == before && !preserveCurrent ? saved : Self.merging(saved, with: current)
            multiplexer.restore(restored, taskId: key.id)
            unavailableDrafts = restored.drafts
            if case .browser(let place?, let browserTabs, _)? = restored.tree.intent(.browser),
               case .files(_, let tabs, let selected)? = browserTabs.first(where: { if case .files = $0 { true } else { false } }) {
                let documents = files(issue: key.id, cwd: place.cwd,
                                      machine: place.machineID == localMachineId ? nil : place.machineID, query: query)
                documents.restoreTabs(tabs, selected: selected)
                // Reconcile every recovered base, including documents with closed tabs.
                for draft in restored.drafts {
                    do {
                        let disk = try await query.taskFile(issue: key.id, path: draft.path)
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
        if needsSave { savePaneIntent() }
    }

    private func observePaneIntent() {
        withObservationTracking { _ = portableLayout() } onChange: { [weak self] in
            Task { @MainActor [weak self] in
                guard let self else { return }
                self.observePaneIntent()
                guard self.persistenceError == nil, !self.isRestoringIntent else { return }
                self.persistenceTask?.cancel()
                self.persistenceTask = Task { [weak self] in
                    do { try await Task.sleep(for: .milliseconds(300)) } catch { return }
                    // A newer edit can cancel this task after its timer fires but before it
                    // resumes; the save it would start belongs to its successor.
                    guard !Task.isCancelled else { return }
                    self?.savePaneIntent()
                }
            }
        }
    }

    func savePaneIntent() {
        guard persistenceLoaded, !isRestoringIntent, persistenceError == nil,
              let layouts = paneLayouts, let machine = localMachineId else { return }
        do { try layouts.save(portableLayout(), machineId: machine, work: key) }
        catch { persistenceError = error.localizedDescription }
    }

    /// Saved content fills in what the workspace already holds; current edits own selection and geometry.
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
        return WorkPaneLayout(tree: merge(current.tree), focused: current.focused,
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
