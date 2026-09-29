#if os(macOS)
import AppKit
import Combine
import Loopflow
import OSLog
import SwiftUI

extension Notification.Name {
    static let openSessions = Notification.Name("loopflow.openSessions")
}

/// The pane whose strip the pointer is over: only that pane shows its trio.
@MainActor
@Observable
final class PaneHover {
    var paneId: String?
}

/// One window's terminal workspace for a checkout: the pane layout and the
/// pool of live surfaces behind it.
@MainActor
final class SessionsWorkspace {
    let multiplexer = MultiplexerStore()
    let hover = PaneHover()
    private var taskFiles: [String: TaskFilesStore] = [:]

    func files(taskId: String, issue: String, cwd: String, query: RegistryQuery = RegistryQueryLocal.shared) -> TaskFilesStore {
        if let files = taskFiles[taskId] { return files }
        let files = TaskFilesStore(issue: issue, cwd: cwd, query: query)
        taskFiles[taskId] = files
        return files
    }
    let surfaces: GhosttySurfacePool
    private var surfaceClosed: AnyCancellable?

    private var retainedStore: SessionsStore?

    func sessionStore(repoPath: String, query: RegistryQuery) -> SessionsStore {
        if let retainedStore { return retainedStore }
        let store = SessionsStore(repoPath: repoPath, query: query, surfaces: surfaces)
        retainedStore = store
        return store
    }

    init(surfaces: GhosttySurfacePool = GhosttySurfacePool()) {
        self.surfaces = surfaces
        // The workspace outlives SessionsView, including while a shell exits
        // on the Work screen or while another repository is selected.
        surfaceClosed = NotificationCenter.default.publisher(for: .ghosttySurfaceClosed)
            .sink { [weak self] notification in
                guard let terminal = notification.object as? TerminalIdentity,
                      case .shell(let paneId) = terminal else { return }
                MainActor.assumeIsolated {
                    guard let self,
                          self.multiplexer.layout.pane(for: paneId)?.content == .shell
                    else { return }
                    self.multiplexer.close(paneId)
                }
            }
    }
}

/// Owned by each window's root view. Keying workspaces per window keeps
/// Sessions↔Work navigation and repo switches lossless inside that window
/// while never sharing an NSView or multiplexer across windows — a Session
/// viewed in another window is just another "active elsewhere" client.
@MainActor
@Observable
final class SessionsWorkspaceRegistry {
    // Observable only for environment delivery; lazy creation must not invalidate views.
    @ObservationIgnored private var workspaces: [String: SessionsWorkspace] = [:]
    @ObservationIgnored private var layouts: [String: WorktreeLayoutStore] = [:]
    let surfaces = GhosttySurfacePool()

    func layout(for repoPath: String) -> WorktreeLayoutStore {
        if let layout = layouts[repoPath] { return layout }
        let layout = WorktreeLayoutStore(path: repoPath)
        layouts[repoPath] = layout
        return layout
    }

    var paths: [String] { workspaces.keys.sorted() }

    func path(containingShell id: String) -> String? {
        workspaces.first { $0.value.multiplexer.layout.pane(for: id)?.content == .shell }?.key
    }

    func removeSessions(_ ids: Set<String>) {
        for workspace in workspaces.values {
            let sessions = Set(workspace.multiplexer.layout.allPanes.compactMap { pane -> String? in
                if case .session(let id) = pane.content { return id }
                return nil
            })
            workspace.multiplexer.reconcileSessions(sessions.subtracting(ids))
        }
    }

    func workspace(for repoPath: String) -> SessionsWorkspace {
        if let existing = workspaces[repoPath] { return existing }
        let workspace = SessionsWorkspace(surfaces: surfaces)
        workspaces[repoPath] = workspace
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
    }

    var record: SessionRecord
    var state: State
    var resolutionError: String?

    var id: String { record.id }

    var surface: SessionRecord? {
        switch state {
        case .prepared, .live: record
        case .pending, .elsewhere, .opening, .failed: nil
        }
    }

    var error: String? {
        guard case .failed(let message) = state else { return nil }
        return message
    }
}

@MainActor
final class SessionsStore: ObservableObject {
    @Published private(set) var sessions: [SessionItem] = []
    var onResolved: ((String) -> Void)?

    let surfaces: GhosttySurfacePool

    let repoPath: String
    private let query: RegistryQuery
    private let metrics: SessionsLatencyMetrics
    private var hasRecordedSessionsLoad = false

    init(
        repoPath: String,
        query: RegistryQuery = RegistryQueryLocal.shared,
        surfaces: GhosttySurfacePool = GhosttySurfacePool()
    ) {
        self.repoPath = WaveOrigin.resolve(repoPath)
        self.query = query
        self.surfaces = surfaces
        metrics = SessionsLatencyMetrics(scope: URL(fileURLWithPath: self.repoPath).lastPathComponent)
    }

    func reconcile(_ records: [SessionRecord]) {
        if !hasRecordedSessionsLoad {
            metrics.recordSessionsLoaded(count: records.count)
            hasRecordedSessionsLoad = true
        }
        let incoming = Set(records.map(\.id))
        sessions.removeAll { !incoming.contains($0.id) }

        for record in records {
            let observedState: SessionItem.State = localTerminal(for: record) != nil
                ? .live : record.action(.moveHere) != nil ? .elsewhere : .pending
            if let index = _index(record.id) {
                // Keep a prepared command until the retained native view consumes it.
                if case .prepared = sessions[index].state { continue }
                sessions[index].record = record
                if case .opening = sessions[index].state { continue }
                if case .failed = sessions[index].state, observedState == .pending { continue }
                sessions[index].state = observedState
            } else {
                sessions.append(SessionItem(record: record, state: observedState))
            }
        }
    }

    private func recover(_ id: String, replacing: Bool = false) async {
        guard let index = _index(id),
              let action = sessions[index].record.action(replacing ? .moveHere : .open),
              action.unavailableReason == nil else { return }
        switch sessions[index].state {
        case .pending, .failed:
            sessions[index].state = .opening
        case .elsewhere where replacing:
            sessions[index].state = .opening
        case .elsewhere, .opening, .prepared, .live:
            return
        }
        do {
            let surface = try await query.openSession(
                id: id,
                replacing: replacing,
                cwd: repoPath
            )
            guard let latest = _index(id) else { return }
            if case .opening = sessions[latest].state {
                sessions[latest].record = surface
                sessions[latest].state = .prepared
            }
        } catch {
            guard let latest = _index(id) else { return }
            sessions[latest].state = .failed(error.localizedDescription)
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
        await recover(id)
    }

    func moveHere(_ id: String) async {
        await recover(id, replacing: true)
    }

    func beginPaneLoad(_ id: String) {
        metrics.beginPaneLoad(id)
    }

    func localTerminal(for record: SessionRecord) -> TerminalIdentity? {
        if surfaces.hasSurface(.session(record.id)) { return .session(record.id) }
        return record.terminalIds.lazy.map { TerminalIdentity.shell($0) }
            .first(where: surfaces.hasSurface)
    }

    func recordPaneLive(_ id: String) {
        metrics.recordPaneLive(id)
        if let index = _index(id), case .prepared = sessions[index].state {
            sessions[index].state = .live
        }
    }

    func complete(_ id: String) async -> Bool {
        guard let index = _index(id) else { return false }
        sessions[index].resolutionError = nil
        do {
            try await query.completeSession(id: id, cwd: repoPath)
            sessions.removeAll { $0.id == id }
            onResolved?(id)
            return true
        } catch {
            guard let latest = _index(id) else { return false }
            sessions[latest].resolutionError = error.localizedDescription
            return false
        }
    }

    /// Frees the retained terminal surface for a Session whose provider client
    /// this window no longer owns (completed, decided, or gone from the list).
    func releaseSurface(_ id: String) {
        surfaces.release(.session(id))
    }

    /// A retained surface's child ended — provider exit or an external
    /// takeover. Reclassify at once so the pane swaps to its placeholder
    /// instead of presenting a dead terminal until the next poll. A window
    /// whose own pool still holds a live surface for this Session (it just
    /// took the client over) is unaffected.
    func noteSurfaceClosed(_ terminal: TerminalIdentity) {
        guard case .session(let id) = terminal,
              !surfaces.hasSurface(terminal),
              let index = _index(id)
        else { return }
        switch sessions[index].state {
        case .prepared, .live:
            sessions[index].state = sessions[index].record.action(.moveHere) != nil
                ? .elsewhere : .pending
        case .pending, .elsewhere, .opening, .failed:
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

/// Unified Work navigation around the existing retained native workspace.
struct SessionsView: View {
    @AppStorage("taskFilesVisible") private var showsFiles = false
    @Bindable var model: PodiumModel
    private let workspaces: SessionsWorkspaceRegistry
    private let query: RegistryQuery
    private let worktreeLayout: WorktreeLayoutStore
    @ObservedObject private var store: SessionsStore
    @State private var layoutRevision = 0
    @State private var launchError: String?
    @State private var completing: String?
    @Environment(\.palette) private var palette

    private var multiplexer: MultiplexerStore {
        let path = navigation.selectedSessionId == nil
            ? taskPath ?? worktreeLayout.focusedPath ?? store.repoPath
            : worktreeLayout.focusedPath ?? store.repoPath
        return workspaces.workspace(for: path).multiplexer
    }
    private var taskPath: String? {
        guard let task = fileTask else { return nil }
        let workspace = task.task.reference.workspace
        if workspace?.localExists != true, let prepared = navigation.preparedTaskWorktrees[task.task.id] {
            return prepared
        }
        return workspace?.worktree
    }
    private var availablePaths: [String] {
        worktreeLayout.knownPaths.union(store.sessions.map(\.record.cwd)).sorted()
    }

    init(model: PodiumModel, repoPath: String, workspaces: SessionsWorkspaceRegistry,
         query: RegistryQuery = RegistryQueryLocal.shared) {
        self.model = model
        self.workspaces = workspaces
        self.query = query
        worktreeLayout = workspaces.layout(for: repoPath)
        let store = workspaces.workspace(for: repoPath).sessionStore(repoPath: repoPath, query: query)
        store.onResolved = { [weak model] id in model?.sessionResolved(id, repo: repoPath) }
        _store = ObservedObject(wrappedValue: store)
    }

    private var navigation: WorkspaceNavigation { model.navigation }
    private var terminalsVisible: Bool { navigation.content == .terminals }
    private var fileTask: WorkspaceTask? {
        // A departing repository view must not mount the next repository's
        // retained terminals while SwiftUI replaces its hierarchy.
        guard model.repoPath?.normalizedFilePath == store.repoPath.normalizedFilePath else { return nil }
        return model.workspace.breadcrumb(selection: model.selection, sessionId: navigation.selectedSessionId)?.task
    }

    var body: some View {
        let _ = layoutRevision
        VStack(spacing: 0) {
            if let error = model.sessions.errorMessage {
                Text("Sessions unavailable — \(error)")
                    .font(Typography.caption(11)).foregroundStyle(Color.statusWarning)
                    .padding(Spacing.sm)
            }
            HStack(spacing: 0) {
                WorkspaceNavigator(model: model, onOpenSession: openSession, onConversation: { work in
                    model.select(work)
                    startConversation()
                }, onNewShell: {
                    if worktreeLayout.focusedPath == nil { worktreeLayout.select(store.repoPath) }
                    multiplexer.newShell()
                    navigation.content = .terminals
                }, onShowTerminals: { navigation.content = .terminals }, onOpenTask: openTask)
                    .frame(width: 264)
                Rectangle().fill(palette.border).frame(width: 1)
                HSplitView {
                    VStack(spacing: 0) {
                        WorkspaceBreadcrumbBar(
                            model: model,
                            crumb: model.workspace.breadcrumb(selection: model.selection, sessionId: navigation.selectedSessionId),
                            onOpenSession: openSession, onMonitor: showMonitor
                        ) {
                            if taskPath != nil {
                                Button(showsFiles ? "Hide Files" : "Show Files") { showsFiles.toggle() }
                                    .buttonStyle(.plain).fixedSize()
                            }
                            if terminalsVisible {
                                if navigation.selectedSessionId != nil { worktreeChip }
                                else if let taskPath {
                                    Text(URL(fileURLWithPath: taskPath).lastPathComponent)
                                        .font(Typography.code(11)).lineLimit(1)
                                        .foregroundStyle(palette.textSecondary).help(taskPath)
                                        .accessibilityLabel("Task worktree")
                                        .accessibilityIdentifier("task-worktree-location")
                                } else if fileTask == nil { worktreeChip }
                                if navigation.selectedSessionId != nil || fileTask == nil || taskPath != nil { completionControls }
                            }
                        }
                        ZStack {
                            Group {
                                if navigation.selectedSessionId != nil || fileTask == nil {
                                    // Binding changes ancestry, never the conversation's panes.
                                    WorktreeNodeView(
                                        node: worktreeLayout.layout, layout: worktreeLayout,
                                        workspaces: workspaces, isActive: terminalsVisible,
                                        showsStrips: worktreeLayout.layout.isSplit, sessions: store)
                                } else if let taskPath {
                                    WorktreeTerminalsView(workspace: workspaces.workspace(for: taskPath),
                                        path: taskPath, isFocused: terminalsVisible, sessions: store)
                                        .id(taskPath)
                                } else {
                                    ContentUnavailableView("Task workspace unavailable", systemImage: "folder")
                                }
                            }
                            .opacity(terminalsVisible ? 1 : 0)
                            .disabled(!terminalsVisible)
                            .allowsHitTesting(terminalsVisible)
                            .accessibilityHidden(!terminalsVisible)
                            VStack(spacing: 0) {
                                HSplitView {
                                    ScrollViewReader { reader in
                                        WorkSurfaceView(model: model, onOpenSession: openSession, onOpenTask: openTask,
                                                        onNewSession: newTaskSession)
                                            .onChange(of: navigation.content == .details
                                                ? navigation.flowDrafts[model.selection?.id ?? ""]?.selectedNode : nil,
                                                      initial: true) { _, selected in
                                                if selected != nil { reader.scrollTo("task-flow-anchor", anchor: .top) }
                                            }
                                    }
                                    .frame(minWidth: 300, maxWidth: .infinity)
                                    if navigation.showsActivity {
                                        WorkActivityView(model: model)
                                            .frame(minWidth: 230, idealWidth: 280, maxWidth: 360)
                                    }
                                }
                            }
                            .background(palette.background)
                            .opacity(terminalsVisible ? 0 : 1)
                            .allowsHitTesting(!terminalsVisible)
                            .accessibilityHidden(terminalsVisible)
                        }
                        .frame(maxWidth: .infinity)
                        .clipped()
                    }
                    if showsFiles, let task = fileTask, let taskPath {
                        TaskFilesView(
                            store: workspaces.workspace(for: taskPath).files(
                                taskId: task.task.id, issue: task.task.task.identifier, cwd: taskPath, query: query),
                            prURL: task.task.activePr?.publication?.github?.url
                        )
                        .id(task.task.id)
                        .frame(minWidth: 480, idealWidth: 700)
                    }
                }
            }
        }
        .background(palette.background)
        .tint(palette.accent)
        .environment(model)
        .overlay {
            if terminalsVisible, navigation.selectedSessionId != nil || fileTask == nil || taskPath != nil {
                SessionsShortcutMonitor { _handle($0) }
                    .allowsHitTesting(false).frame(width: 0, height: 0)
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .multiplexerStoreDidChange)) { notification in
            guard notification.object is MultiplexerStore else { return }
            layoutRevision += 1
        }
        .onChange(of: model.sessions.value, initial: true) { _, records in
            guard model.repoPath?.normalizedFilePath == store.repoPath.normalizedFilePath,
                  let records else { return }
            let previous = Set(store.sessions.map(\.id))
            store.reconcile(records)
            let ids = Set(store.sessions.map(\.id))
            for id in previous.subtracting(ids) { store.releaseSurface(id) }
            workspaces.removeSessions(previous.subtracting(ids))
        }
        .onChange(of: store.sessions.map(\.id)) { previous, ids in
            for id in Set(previous).subtracting(ids) { store.releaseSurface(id) }
            workspaces.removeSessions(Set(previous).subtracting(ids))
        }
        .onReceive(NotificationCenter.default.publisher(for: .ghosttySurfaceClosed)) { notification in
            guard let terminal = notification.object as? TerminalIdentity else { return }
            store.noteSurfaceClosed(terminal)
        }
        .onReceive(NotificationCenter.default.publisher(for: .openSessions)) { _ in
            navigation.content = .terminals
        }
        .alert("Could not start conversation", isPresented: Binding(
            get: { launchError != nil },
            set: { if !$0 { launchError = nil } }
        )) {
            Button("OK") { launchError = nil }
        } message: { Text(launchError ?? "") }
        .accessibilityIdentifier("sessions-surface")
    }

    private func openSession(_ record: SessionRecord) {
        let subject = model.workspace.subject(for: record.id)
        model.select(subject)
        Perf.begin(Perf.taskWorkspaceReady, "session", id: record.id)
        navigation.selectedSessionId = record.id
        navigation.content = .terminals
        // Place the opening/error/elsewhere pane immediately. A slow preparation
        // must never change focus after the human selects another subject.
        if case .shell(let id) = store.localTerminal(for: record),
           let path = workspaces.path(containingShell: id) {
            worktreeLayout.select(path)
            multiplexer.setFocusedPane(id)
            store.surfaces.focus(.shell(id))
        } else {
            worktreeLayout.select(record.cwd)
            multiplexer.load(sessionId: record.id)
            store.surfaces.focus(.session(record.id))
        }
        store.beginPaneLoad(record.id)
        Task { @MainActor in await store.select(record.id) }
    }

    /// A Task with exactly one open Session drills into that Session; zero or
    /// several open the Task overview, which names each conversation.
    private func openTask(_ work: WorkReference) {
        let sessions = model.visibleWorkspace.waves.lazy.flatMap(\.tasks)
            .first { $0.id.work == work }?.sessions ?? []
        if sessions.count == 1, let session = sessions.first {
            openSession(session)
        } else {
            model.select(work)
        }
    }

    private func showMonitor(_ taskId: String) {
        let workspace = model.task(id: taskId)?.task.reference.workspace
        let existing = workspaces.paths.first { path in
            workspaces.workspace(for: path).multiplexer.layout.allPanes.contains {
                $0.content == .monitor(taskId: taskId)
            }
        }
        let path = existing ?? workspace.flatMap { $0.localExists == true ? $0.worktree : nil } ?? store.repoPath
        worktreeLayout.select(path)
        navigation.selectedSessionId = nil
        navigation.content = .terminals
        multiplexer.showMonitor(taskId: taskId)
        model.observeActiveSessions()
    }

    private func startConversation() {
        guard let conversationScope = model.conversationScope else { return }
        do { try launch(conversationScope, in: navigation) }
        catch { launchError = error.localizedDescription }
    }

    /// `navigation` belongs to the repository that requested the launch; a
    /// launch that finishes after the human switched repositories must not
    /// redirect the repository now on screen.
    private func launch(_ scope: ConversationScope, in navigation: WorkspaceNavigation) throws {
        let lf = try LocalWaveAgentLauncher.controlLfPath()
        worktreeLayout.select(scope.repoPath)
        navigation.content = .terminals
        workspaces.workspace(for: scope.repoPath).multiplexer.newShell(
            command: ConversationLaunch(scope: scope).arguments(lf: lf))
    }

    /// An independent conversation with Task context in the Task's checkout.
    /// Resolving the checkout prepares Task Work only; it never starts the
    /// managed Flow or replaces another Session.
    private func newTaskSession(_ taskId: String) async throws {
        guard let found = model.task(id: taskId) else { return }
        let origin = navigation
        let issue = found.task.task.identifier
        let worktree: String
        if let workspace = found.task.reference.workspace, workspace.localExists == true {
            worktree = workspace.worktree
        } else {
            let repo = store.repoPath
            worktree = try await Task.detached(priority: .userInitiated) {
                try LocalWaveAgentLauncher.checkoutTask(repoPath: repo, issue: issue)
            }.value
            origin.preparedTaskWorktrees[taskId] = worktree
        }
        try launch(.task(repo: worktree, id: issue), in: origin)
    }

    /// The focused worktree as a quiet mono chip; its menu holds every
    /// worktree action, so no slot needs a header of its own.
    private var worktreeChip: some View {
        let slot = worktreeLayout.focusedSlotId
        let path = worktreeLayout.focusedPath
        return Menu {
            ForEach(availablePaths, id: \.self) { candidate in
                Button(URL(fileURLWithPath: candidate).lastPathComponent) {
                    worktreeLayout.select(candidate, in: slot)
                }
            }
            Divider()
            if let path {
                Button("New terminal in this worktree") {
                    worktreeLayout.focus(slot)
                    workspaces.workspace(for: path).multiplexer.newShell()
                }
                .accessibilityLabel("New terminal in worktree")
            }
            Button("Split worktrees right") { worktreeLayout.split(slot, axis: .vertical) }
                .accessibilityLabel("Split worktrees right")
            Button("Split worktrees down") { worktreeLayout.split(slot, axis: .horizontal) }
                .accessibilityLabel("Split worktrees down")
            Button("Hide this worktree") { worktreeLayout.close(slot) }
                .help("Its terminals keep running")
                .accessibilityLabel("Hide worktree")
        } label: {
            HStack(spacing: 5) {
                Image(systemName: "folder").font(.system(size: 10))
                Text(path.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Choose worktree")
                    .font(Typography.code(11))
                    .lineLimit(1)
            }
            .foregroundStyle(palette.textSecondary)
            .padding(.horizontal, 7)
            .padding(.vertical, 2)
            .background(palette.surface, in: RoundedRectangle(cornerRadius: 4))
            .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(palette.border))
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .fixedSize()
        .help(path ?? "Choose a worktree for this slot")
        .accessibilityLabel("Worktree")
        .accessibilityIdentifier("worktree-chip")
    }

    /// Sessions attached to the focused pane: a Session pane's own record, or
    /// every conversation running inside a shell pane.
    private var focusedPaneSessions: [SessionItem] {
        let pane = multiplexer.focusedPane
        switch pane.content {
        case .shell:
            return store.sessions.filter { store.localTerminal(for: $0.record) == .shell(pane.id) }
        case .session(let id):
            return store.sessions.filter { $0.id == id }
        case .empty, .monitor:
            return []
        }
    }

    /// Complete, as text at the toolbar's right end, only for a Session whose
    /// shared actions accept it. A rejected completion stays beside it.
    @ViewBuilder
    private var completionControls: some View {
        let items = focusedPaneSessions
        let completable = items.filter { $0.record.action(.complete) != nil && $0.surface != nil }
        ForEach(items) { item in
            if let error = item.resolutionError {
                Text(error)
                    .font(Typography.meta)
                    .foregroundStyle(WorkspaceTone.blocked.ink)
                    .lineLimit(1)
                    .help(error)
            }
        }
        ForEach(completable) { item in
            if let action = item.record.action(.complete) {
                completionButton(item, action: action, several: completable.count > 1)
            }
        }
    }

    private func completionButton(_ item: SessionItem, action: SessionAction, several: Bool) -> some View {
        Button {
            completing = item.id
            Task { @MainActor in
                defer { completing = nil }
                guard await store.complete(item.id) else { return }
                multiplexer.reconcileSessions(Set(store.sessions.map(\.id)))
                store.releaseSurface(item.id)
            }
        } label: {
            HStack(spacing: 6) {
                if completing == item.id { ProgressView().controlSize(.mini) }
                Text(several ? "\(action.label) · \(item.record.title)" : action.label)
                    .font(Typography.text)
            }
            .foregroundStyle(palette.accentInk)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .fixedSize()
        .disabled(completing != nil || action.unavailableReason != nil)
        .help(action.unavailableReason ?? action.help)
        .accessibilityLabel(several ? "Complete session: \(item.record.title)" : "Complete session")
        .accessibilityHint(action.unavailableReason ?? action.help)
        .accessibilityIdentifier("session-action-complete")
    }

    private func _destroySurface(in pane: PaneState) {
        switch pane.content {
        case .empty, .monitor:
            return
        case .shell:
            store.surfaces.release(.shell(pane.id))
        case .session:
            return
        }
    }

    private func _handle(_ shortcut: SessionsShortcut) {
        switch shortcut {
        case .splitRight:
            _ = multiplexer.split(multiplexer.focusedPaneId, axis: .vertical)
        case .splitDown:
            _ = multiplexer.split(multiplexer.focusedPaneId, axis: .horizontal)
        case .close:
            _destroySurface(in: multiplexer.focusedPane)
            multiplexer.close(multiplexer.focusedPaneId)
        case .undoClose:
            multiplexer.undoClose()
        case .zoom:
            multiplexer.toggleZoom(multiplexer.focusedPaneId)
        case .focus(let direction):
            multiplexer.focus(direction)
        }
    }
}

private struct WorktreeNodeView: View {
    let node: WorktreeLayout
    let layout: WorktreeLayoutStore
    let workspaces: SessionsWorkspaceRegistry
    let isActive: Bool
    /// Split worktrees name themselves on a strip; a single worktree is named
    /// by the toolbar chip and gets no chrome of its own.
    let showsStrips: Bool
    @ObservedObject var sessions: SessionsStore

    var body: some View { content }

    private var content: AnyView {
        switch node {
        case .leaf(let id, let path):
            return AnyView(VStack(spacing: 0) {
                if showsStrips {
                    HStack(spacing: 8) {
                        Image(systemName: "folder").font(.system(size: 10))
                        Text(path.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Choose worktree")
                            .font(Typography.code(11))
                            .lineLimit(1)
                        Spacer()
                    }
                    .foregroundStyle(layout.focusedSlotId == id ? TerminalPalette.foreground : TerminalPalette.dim)
                    .padding(.horizontal, 12)
                    .frame(height: 24)
                    .background(TerminalPalette.background)
                    .contentShape(Rectangle())
                    .onTapGesture { layout.focus(id) }
                    .accessibilityIdentifier("worktree-strip-\(id)")
                    Rectangle().fill(TerminalPalette.divider).frame(height: 1)
                }
                if let path {
                    WorktreeTerminalsView(
                        workspace: workspaces.workspace(for: path), path: path,
                        isFocused: isActive && layout.focusedSlotId == id,
                        sessions: sessions
                    )
                    .id(path)
                    .simultaneousGesture(TapGesture().onEnded { layout.focus(id) })
                } else {
                    ContentUnavailableView("Choose a worktree", systemImage: "folder",
                                           description: Text("Choose a worktree from the toolbar to restore its conversation and terminals."))
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .background(TerminalPalette.background)
                        .environment(\.colorScheme, .dark)
                }
            }
            .accessibilityIdentifier("worktree-slot-\(id)"))
        case .split(let axis, let first, let second):
            if axis == .vertical {
                return AnyView(HSplitView { child(first); child(second) })
            }
            return AnyView(VSplitView { child(first); child(second) })
        }
    }

    private func child(_ node: WorktreeLayout) -> WorktreeNodeView {
        WorktreeNodeView(node: node, layout: layout, workspaces: workspaces, isActive: isActive,
                         showsStrips: showsStrips, sessions: sessions)
    }
}

private struct WorktreeTerminalsView: View {
    let workspace: SessionsWorkspace
    let path: String
    let isFocused: Bool
    @ObservedObject var sessions: SessionsStore
    @State private var revision = 0

    var body: some View {
        let _ = revision
        let store = workspace.multiplexer
        MultiplexerView(
            layout: store.layout,
            focusedPaneId: isFocused ? store.focusedPaneId : "",
            zoomedPaneId: store.zoomedPaneId,
            workingDirectory: path, sessions: sessions, store: store, hover: workspace.hover
        )
        .onReceive(NotificationCenter.default.publisher(for: .multiplexerStoreDidChange)) { notification in
            if let source = notification.object as? MultiplexerStore, source === store { revision += 1 }
        }
    }
}

private struct MultiplexerView: View {
    let layout: LayoutNode
    let focusedPaneId: String
    let zoomedPaneId: String?
    let workingDirectory: String
    @ObservedObject var sessions: SessionsStore
    let store: MultiplexerStore
    let hover: PaneHover

    var body: some View {
        Group {
            if let zoomedPaneId, let pane = layout.pane(for: zoomedPaneId) {
                SessionPaneView(
                    pane: pane,
                    isFocused: !focusedPaneId.isEmpty,
                    workingDirectory: workingDirectory,
                    sessions: sessions,
                    store: store,
                    hover: hover
                )
            } else {
                MultiplexerNodeView(
                    node: layout,
                    focusedPaneId: focusedPaneId,
                    workingDirectory: workingDirectory,
                    sessions: sessions,
                    store: store,
                    hover: hover
                )
            }
        }
        .background(TerminalPalette.background)
        .environment(\.colorScheme, .dark)
        .accessibilityIdentifier("sessions-multiplexer")
    }
}

private struct MultiplexerNodeView: View {
    let node: LayoutNode
    let focusedPaneId: String
    let workingDirectory: String
    @ObservedObject var sessions: SessionsStore
    let store: MultiplexerStore
    let hover: PaneHover

    var body: some View { _content }

    private var _content: AnyView {
        switch node {
        case .leaf(let pane):
            return AnyView(
                SessionPaneView(
                    pane: pane,
                    isFocused: pane.id == focusedPaneId,
                    workingDirectory: workingDirectory,
                    sessions: sessions,
                    store: store,
                    hover: hover
                )
                .id(pane.id)
            )
        case .split(let axis, let first, let second, let ratio):
            return AnyView(
                GeometryReader { geometry in
                    if axis == .vertical {
                        HStack(spacing: 0) {
                            _child(first)
                                .frame(width: max(120, geometry.size.width * ratio - 3))
                            SplitDivider(
                                axis: axis,
                                ratio: ratio,
                                extent: geometry.size.width,
                                firstPaneId: first.firstPane.id,
                                secondPaneId: second.firstPane.id,
                                store: store
                            )
                            _child(second)
                        }
                    } else {
                        VStack(spacing: 0) {
                            _child(first)
                                .frame(height: max(100, geometry.size.height * ratio - 3))
                            SplitDivider(
                                axis: axis,
                                ratio: ratio,
                                extent: geometry.size.height,
                                firstPaneId: first.firstPane.id,
                                secondPaneId: second.firstPane.id,
                                store: store
                            )
                            _child(second)
                        }
                    }
                }
            )
        }
    }

    private func _child(_ child: LayoutNode) -> MultiplexerNodeView {
        MultiplexerNodeView(
            node: child,
            focusedPaneId: focusedPaneId,
            workingDirectory: workingDirectory,
            sessions: sessions,
            store: store,
            hover: hover
        )
    }
}

private struct SplitDivider: View {
    let axis: SplitAxis
    let ratio: Double
    let extent: CGFloat
    let firstPaneId: String
    let secondPaneId: String
    let store: MultiplexerStore

    @State private var dragStart: Double?

    var body: some View {
        Rectangle()
            .fill(Color.clear)
            .frame(
                width: axis == .vertical ? 6 : nil,
                height: axis == .horizontal ? 6 : nil
            )
            .overlay {
                Rectangle()
                    .fill(TerminalPalette.divider)
                    .frame(width: axis == .vertical ? 1 : nil, height: axis == .horizontal ? 1 : nil)
            }
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { value in
                        let start = dragStart ?? ratio
                        if dragStart == nil { dragStart = ratio }
                        let delta = axis == .vertical
                            ? value.translation.width
                            : value.translation.height
                        guard extent > 0 else { return }
                        store.updateRatio(
                            between: firstPaneId,
                            and: secondPaneId,
                            ratio: start + Double(delta / extent)
                        )
                    }
                    .onEnded { _ in dragStart = nil }
            )
    }
}

private struct SessionPaneView: View {
    @Environment(PodiumModel.self) private var model
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let pane: PaneState
    let isFocused: Bool
    let workingDirectory: String
    @ObservedObject var sessions: SessionsStore
    let store: MultiplexerStore
    let hover: PaneHover
    @State private var bellRinging = false
    @State private var terminalTitle: String?

    private var hovering: Bool { hover.paneId == pane.id }

    private var item: SessionItem? {
        guard case .session(let id) = pane.content else { return nil }
        return sessions.sessions.first { $0.id == id }
    }

    /// One pane needs no chrome; with two or more, a 24pt strip names each.
    private var showsStrip: Bool { store.layout.allPanes.count > 1 }

    var body: some View {
        VStack(spacing: 0) {
            if showsStrip {
                strip
                Rectangle().fill(TerminalPalette.divider).frame(height: 1)
            }
            content
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(TerminalPalette.background)
        // Focus is the pane at full opacity; unfocused panes dim, no border.
        .opacity(isFocused || !showsStrip ? 1 : 0.85)
        .animation(DesignAnimation.standard(reduceMotion), value: isFocused)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("\(_title), \(isFocused ? "active" : "open")")
        .accessibilityIdentifier(item.map { "session-pane-\($0.id)" } ?? "session-pane-empty")
        .onReceive(NotificationCenter.default.publisher(for: .ghosttyTerminalBell)) { notification in
            guard let terminal = notification.object as? TerminalIdentity,
                  terminal == _terminalIdentity else { return }
            bellRinging = true
        }
        .onReceive(NotificationCenter.default.publisher(for: .ghosttyTerminalTitle)) { notification in
            guard let update = notification.object as? GhosttyTerminalTitle,
                  update.terminal == _terminalIdentity else { return }
            terminalTitle = sessions.surfaces.title(for: update.terminal)
        }
        .onChange(of: isFocused) { _, focused in
            if focused { bellRinging = false }
        }
        .onChange(of: _terminalIdentity) { _, _ in
            bellRinging = false
            terminalTitle = nil
        }
    }

    /// State dot, mono name, and on the focused pane a hover-only trio. Split,
    /// close and zoom otherwise live in keybinds and the context menu.
    private var strip: some View {
        HStack(spacing: 8) {
            Circle().fill(stateDot).frame(width: 6, height: 6)
            Text(_title)
                .font(Typography.code(11))
                .foregroundStyle(TerminalPalette.dim)
                .lineLimit(1)
            Spacer(minLength: 8)
            if bellRinging {
                Image(systemName: "bell.fill")
                    .font(.system(size: 10))
                    .foregroundStyle(TerminalPalette.stateDot(.human))
                    .accessibilityLabel("Session ready")
            }
            if isFocused && hovering {
                HStack(spacing: 2) {
                    stripButton("rectangle.split.2x1", label: "Split right", id: "pane-split-right", help: "Split right (⌘D)") {
                        _ = store.split(pane.id, axis: .vertical)
                    }
                    stripButton("rectangle.split.1x2", label: "Split down", id: "pane-split-down", help: "Split down (⌘⇧D)") {
                        _ = store.split(pane.id, axis: .horizontal)
                    }
                    stripButton("xmark", label: "Close view", id: "pane-close", help: closeHelp, disabled: !store.canClose) {
                        closePane()
                    }
                }
                .transition(.opacity)
                .accessibilityIdentifier("pane-actions")
            }
        }
        .padding(.leading, 12)
        .padding(.trailing, 4)
        .frame(height: 24)
        .contentShape(Rectangle())
        .background(PaneHoverRegion(identifier: "pane-hover-\(pane.id)") { inside in
            if inside { hover.paneId = pane.id } else if hover.paneId == pane.id { hover.paneId = nil }
        })
        .animation(DesignAnimation.fast(reduceMotion), value: hovering)
        .onTapGesture { focusPane() }
        .contextMenu { paneActions }
        .accessibilityIdentifier("pane-strip-\(pane.id)")
    }

    @ViewBuilder
    private var paneActions: some View {
        Button("Split right (⌘D)") { _ = store.split(pane.id, axis: .vertical) }
        Button("Split down (⌘⇧D)") { _ = store.split(pane.id, axis: .horizontal) }
        Button(store.zoomedPaneId == pane.id ? "Exit zoom (⌘⇧↩)" : "Zoom pane (⌘⇧↩)") { store.toggleZoom(pane.id) }
        Divider()
        Button("Close view (⌘W)") { closePane() }.disabled(!store.canClose)
    }

    private var closeHelp: String {
        pane.content == .shell ? "Close this shell (⌘W)" : "Close this view; the Session keeps running (⌘W)"
    }

    private func closePane() {
        if case .shell = pane.content {
            sessions.surfaces.release(.shell(pane.id))
        }
        store.close(pane.id)
    }

    /// One strip glyph with a hit target larger than its symbol.
    private func stripButton(
        _ systemImage: String, label: String, id: String, help: String,
        disabled: Bool = false, action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.system(size: 11))
                .foregroundStyle(TerminalPalette.dim)
                .frame(width: 22, height: 20)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(disabled)
        .help(help)
        .accessibilityLabel(label)
        .accessibilityIdentifier(id)
    }

    /// The conversation's shared state, or nothing for a plain shell or Monitor.
    private var stateDot: Color {
        guard let state = paneSessions.first?.record.state else { return TerminalPalette.divider }
        switch state {
        case .unknown: return TerminalPalette.divider
        case .active: return TerminalPalette.stateDot(.running)
        case .waiting, .ready: return TerminalPalette.stateDot(.human)
        case .closed: return TerminalPalette.stateDot(.stopped)
        }
    }

    private var paneSessions: [SessionItem] {
        if case .shell = pane.content {
            return sessions.sessions.filter {
                sessions.localTerminal(for: $0.record) == .shell(pane.id)
            }
        }
        return item.map { [$0] } ?? []
    }

    private func focusPane() {
        store.setFocusedPane(pane.id)
        // Only an exact, unambiguous Session attachment changes conversation
        // context. Companion shells retain the current breadcrumb.
        guard paneSessions.count == 1, let session = paneSessions.first,
              model.navigation.selectedSessionId != session.id else { return }
        model.select(model.workspace.subject(for: session.id))
        model.navigation.selectedSessionId = session.id
        model.navigation.content = .terminals
    }

    @ViewBuilder
    private var content: some View {
        switch pane.content {
        case .empty:
            emptyWorkspace
        case .session:
            if let item {
                if let surface = item.surface {
                    _terminal(item: item, surface: surface)
                } else {
                    _sessionPlaceholder(item)
                }
            } else {
                emptyWorkspace
            }
        case .shell:
            GhosttyTerminalView(
                workingDirectory: workingDirectory,
                argv: store.shellCommands[pane.id] ?? [],
                terminal: .shell(pane.id),
                surfacePool: sessions.surfaces,
                isFocused: isFocused,
                onFocus: focusPane
            )
            .id(pane.id)
        case .monitor(let taskId):
            TaskMonitorView(taskId: taskId, model: model)
                .background(MonitorFocusTarget(isFocused: isFocused))
                .simultaneousGesture(TapGesture().onEnded { store.setFocusedPane(pane.id) })
        }
    }

    private var emptyWorkspace: some View {
        ContentUnavailableView {
            Label("No session open", systemImage: "terminal")
        } description: {
            Text("Choose a session from the sidebar or start a terminal.")
        } actions: {
            Button {
                store.newShell()
            } label: {
                Label("New terminal", systemImage: "terminal")
            }
            .buttonStyle(.bordered)
            .tint(TerminalPalette.accent)
            .accessibilityIdentifier("sessions-empty-new-shell")
        }
    }

    /// The pane a Session occupies before its terminal exists here. Each state
    /// explains itself and carries its one explicit next action.
    @ViewBuilder
    private func _sessionPlaceholder(_ item: SessionItem) -> some View {
        switch item.state {
        case .elsewhere:
            _elsewherePlaceholder(item)
        case .opening, .prepared:
            ProgressView("Opening session…")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .failed(let message):
            ContentUnavailableView {
                Label("Session failed to open", systemImage: "exclamationmark.triangle")
            } description: {
                Text(message)
            } actions: {
                openButton(item, retrying: true)
            }
        case .pending, .live:
            ContentUnavailableView {
                Label("Not running here", systemImage: "terminal")
            } description: {
                Text("This Session has no live terminal in this pane yet.")
            } actions: {
                openButton(item, retrying: false)
            }
        }
    }

    @ViewBuilder
    private func openButton(_ item: SessionItem, retrying: Bool) -> some View {
        if let action = item.record.action(.moveHere) ?? item.record.action(.open) {
            Button(action.kind == .open && retrying ? "Try again" : action.label) {
                sessions.beginPaneLoad(item.id)
                Task { @MainActor in
                    if action.kind == .moveHere { await sessions.moveHere(item.id) }
                    else { await sessions.select(item.id) }
                }
            }
            .buttonStyle(.bordered)
            .tint(TerminalPalette.accent)
            .disabled(action.unavailableReason != nil)
            .help(action.unavailableReason ?? action.help)
            .accessibilityIdentifier(retrying ? "session-retry-\(item.id)" : "session-open-here-\(item.id)")
        }
    }

    /// A Session whose provider client lives in another terminal. The pane
    /// explains what that means and makes the takeover explicit: nothing is
    /// stopped until Move here.
    private func _elsewherePlaceholder(_ item: SessionItem) -> some View {
        ContentUnavailableView {
            Label("Active in another terminal", systemImage: "rectangle.on.rectangle")
        } description: {
            Text(
                """
                \(item.record.title) is attached to a provider client outside \
                this window — Warp, another Loopflow window, or an SSH session. \
                It keeps running there until you move it.
                """
            )
        } actions: {
            if let action = item.record.action(.moveHere) {
                VStack(spacing: Spacing.sm) {
                    Button {
                        sessions.beginPaneLoad(item.id)
                        Task { @MainActor in await sessions.moveHere(item.id) }
                    } label: {
                        Label(action.label, systemImage: "arrow.down.forward.square")
                    }
                    .buttonStyle(.bordered)
                    .tint(TerminalPalette.accent)
                    .disabled(action.unavailableReason != nil)
                    .help(action.unavailableReason ?? action.help)
                    .accessibilityLabel("Move session here")
                    .accessibilityHint("Stops the other client; unsent text typed there is lost")
                    .accessibilityIdentifier("session-move-here-\(item.id)")
                    Text("Moving stops the other client. Unsent text typed there is lost.")
                        .font(Typography.caption(9))
                        .foregroundStyle(.white.opacity(0.55))
                }
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("session-elsewhere-\(item.id)")
    }

    @ViewBuilder
    private func _terminal(item: SessionItem, surface: SessionRecord) -> some View {
        GhosttyTerminalView(
            workingDirectory: workingDirectory,
            argv: surface.openArgv,
            terminal: .session(surface.id),
            surfacePool: sessions.surfaces,
            isFocused: isFocused,
            onSurfaceCreated: {
                sessions.recordPaneLive(item.id)
            },
            onFocus: focusPane
        )
        .id(surface.id)
    }

    /// A pane hosting exactly one conversation is named by that Session;
    /// otherwise the terminal's own title, then the pane kind.
    private var _title: String {
        if case .shell = pane.content, paneSessions.count == 1, let session = paneSessions.first {
            return session.record.title
        }
        let title = terminalTitle ?? _terminalIdentity.flatMap { sessions.surfaces.title(for: $0) }
        if let title, !title.isEmpty { return title }
        return switch pane.content {
        case .empty: "Workspace"
        case .session: item?.record.detail ?? "Workspace"
        case .shell: "Shell"
        case .monitor: "Monitor"
        }
    }

    private var _terminalIdentity: TerminalIdentity? {
        switch pane.content {
        case .session:
            item?.surface.map { .session($0.id) }
        case .shell:
            .shell(pane.id)
        case .empty, .monitor:
            nil
        }
    }
}

/// Hover over a pane strip, tracked by an AppKit tracking area so it stays
/// reliable beside a Metal-backed terminal. It never takes a click.
private struct PaneHoverRegion: NSViewRepresentable {
    let identifier: String
    let onChange: (Bool) -> Void

    func makeNSView(context: Context) -> TrackingView {
        let view = TrackingView()
        view.identifier = NSUserInterfaceItemIdentifier(identifier)
        view.onChange = onChange
        return view
    }

    func updateNSView(_ view: TrackingView, context: Context) {
        view.onChange = onChange
    }

    final class TrackingView: NSView {
        var onChange: ((Bool) -> Void)?

        override func updateTrackingAreas() {
            super.updateTrackingAreas()
            for area in trackingAreas { removeTrackingArea(area) }
            addTrackingArea(NSTrackingArea(
                rect: bounds, options: [.mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect],
                owner: self, userInfo: nil
            ))
        }

        override func mouseEntered(with event: NSEvent) { onChange?(true) }
        override func mouseExited(with event: NSEvent) { onChange?(false) }
        override func hitTest(_ point: NSPoint) -> NSView? { nil }
    }
}

// Giving AppKit an observation responder prevents clearing terminal focus from
// advancing straight to another visible terminal in the same key-view loop.
private struct MonitorFocusTarget: NSViewRepresentable {
    let isFocused: Bool

    func makeNSView(context: Context) -> FocusView { FocusView() }

    func updateNSView(_ view: FocusView, context: Context) {
        guard view.focusRequested != isFocused else { return }
        view.focusRequested = isFocused
        view.applyFocus()
    }

    final class FocusView: NSView {
        var focusRequested = false
        override var acceptsFirstResponder: Bool { focusRequested }

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            applyFocus()
        }

        func applyFocus() {
            if focusRequested {
                window?.makeFirstResponder(self)
            } else if window?.firstResponder === self {
                window?.makeFirstResponder(nil)
            }
        }
    }
}

private extension WorktreeLayout {
    var isSplit: Bool {
        if case .split = self { return true }
        return false
    }
}

private enum SessionsShortcut {
    case splitRight
    case splitDown
    case close
    case undoClose
    case zoom
    case focus(SpatialDirection)
}

private struct SessionsShortcutMonitor: NSViewRepresentable {
    let handler: (SessionsShortcut) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(handler: handler)
    }

    func makeNSView(context: Context) -> NSView {
        let view = NSView(frame: .zero)
        context.coordinator.install(for: view)
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        context.coordinator.view = nsView
    }

    static func dismantleNSView(_ nsView: NSView, coordinator: Coordinator) {
        coordinator.uninstall()
    }

    @MainActor
    final class Coordinator {
        weak var view: NSView?
        private let handler: (SessionsShortcut) -> Void
        private var monitor: Any?

        init(handler: @escaping (SessionsShortcut) -> Void) {
            self.handler = handler
        }

        func install(for view: NSView) {
            self.view = view
            monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
                guard let self,
                      let window = self.view?.window,
                      event.window === window,
                      let shortcut = Self._shortcut(for: event)
                else { return event }
                // A focused text view owns ⌘Z. Every other shortcut stays here, so ⌘W
                // closes a pane instead of falling through to close the window.
                if case .undoClose = shortcut, window.firstResponder is NSTextView { return event }
                self.handler(shortcut)
                return nil
            }
        }

        func uninstall() {
            if let monitor {
                NSEvent.removeMonitor(monitor)
                self.monitor = nil
            }
        }

        private static func _shortcut(for event: NSEvent) -> SessionsShortcut? {
            let modifiers = event.modifierFlags.intersection([
                .command, .shift, .option, .control,
            ])
            // Shift is the one modifier `charactersIgnoringModifiers` keeps, so
            // ⌘⇧D arrives as "D".
            let key = event.charactersIgnoringModifiers?.lowercased()
            if modifiers == [.command], key == "d" {
                return .splitRight
            }
            if modifiers == [.command, .shift], key == "d" {
                return .splitDown
            }
            if modifiers == [.command], event.charactersIgnoringModifiers == "w" {
                return .close
            }
            if modifiers == [.command], event.charactersIgnoringModifiers == "z" {
                return .undoClose
            }
            if modifiers == [.command, .shift], event.keyCode == 36 || event.keyCode == 76 {
                return .zoom
            }
            guard modifiers == [.command, .option] else { return nil }
            return switch event.specialKey {
            case .leftArrow: .focus(.left)
            case .rightArrow: .focus(.right)
            case .upArrow: .focus(.up)
            case .downArrow: .focus(.down)
            default: nil
            }
        }
    }
}

#endif
