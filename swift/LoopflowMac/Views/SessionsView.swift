#if os(macOS)
import AppKit
import Combine
import Loopflow
import OSLog
import SwiftUI

extension Notification.Name {
    static let openSessions = Notification.Name("loopflow.openSessions")
}

/// One window's terminal workspace for a checkout: the pane layout and the
/// pool of live surfaces behind it.
@MainActor
final class SessionsWorkspace {
    let multiplexer = MultiplexerStore()
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
final class SessionsWorkspaceRegistry {
    private var workspaces: [String: SessionsWorkspace] = [:]
    private var layouts: [String: WorktreeLayoutStore] = [:]
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

    func reconcileSessions(_ ids: Set<String>, in paths: Set<String>) {
        for path in paths { workspaces[path]?.multiplexer.reconcileSessions(ids) }
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

    var statusLabel: String {
        switch record.state {
        case .waiting: "WAITING"
        case .active: "ACTIVE"
        case .ready: "READY"
        case .closed: "CLOSED"
        }
    }

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
    @Bindable var model: PodiumModel
    private let workspaces: SessionsWorkspaceRegistry
    private let worktreeLayout: WorktreeLayoutStore
    @ObservedObject private var store: SessionsStore
    @State private var layoutRevision = 0
    @State private var launchError: String?
    @Environment(\.palette) private var palette

    private var multiplexer: MultiplexerStore {
        workspaces.workspace(for: worktreeLayout.focusedPath ?? store.repoPath).multiplexer
    }
    private var availablePaths: [String] {
        worktreeLayout.knownPaths.union(store.sessions.map(\.record.cwd)).sorted()
    }

    init(model: PodiumModel, repoPath: String, workspaces: SessionsWorkspaceRegistry,
         query: RegistryQuery = RegistryQueryLocal.shared) {
        self.model = model
        self.workspaces = workspaces
        worktreeLayout = workspaces.layout(for: repoPath)
        let store = workspaces.workspace(for: repoPath).sessionStore(repoPath: repoPath, query: query)
        store.onResolved = { [weak model] id in model?.sessionResolved(id, repo: repoPath) }
        _store = ObservedObject(wrappedValue: store)
    }

    private var navigation: WorkspaceNavigation { model.navigation }
    private var terminalsVisible: Bool { navigation.content == .terminals }

    var body: some View {
        let _ = layoutRevision
        VStack(spacing: 0) {
            if let error = model.sessions.errorMessage {
                Text("Sessions unavailable — \(error)")
                    .font(Typography.caption(11)).foregroundStyle(Color.statusWarning)
                    .padding(Spacing.sm)
            }
            HStack(spacing: 0) {
                WorkspaceNavigator(model: model, onOpenSession: openSession, sessionStatus: { record in
                    guard let item = store.sessions.first(where: { $0.id == record.id }) else {
                        return record.state.rawValue.uppercased()
                    }
                    return _status(item, pane: _pane(for: record.id))
                }, onConversation: { work in
                    model.select(work)
                    startConversation()
                }, onNewShell: {
                    if worktreeLayout.focusedPath == nil { worktreeLayout.select(store.repoPath) }
                    multiplexer.newShell()
                    navigation.content = .terminals
                }, onShowTerminals: { navigation.content = .terminals }, onOpenTask: openTask)
                    .frame(width: 300)
                Divider()
                VStack(spacing: 0) {
                    if let selected = model.selection, selected.kind == .task {
                        taskControls(selected.id)
                    }
                    ZStack {
                        WorktreeNodeView(
                            node: worktreeLayout.layout, layout: worktreeLayout,
                            workspaces: workspaces, paths: availablePaths, isActive: terminalsVisible, sessions: store
                        )
                        .opacity(terminalsVisible ? 1 : 0)
                        .disabled(!terminalsVisible)
                        .allowsHitTesting(terminalsVisible)
                        .accessibilityHidden(!terminalsVisible)
                        VStack(spacing: 0) {
                            HSplitView {
                                WorkSurfaceView(model: model)
                                    .frame(minWidth: 300, maxWidth: .infinity)
                                WorkActivityView(model: model)
                                    .frame(minWidth: 230, idealWidth: 280, maxWidth: 360)
                            }
                        }
                        .background(palette.background)
                        .opacity(navigation.content == .details ? 1 : 0)
                        .allowsHitTesting(navigation.content == .details)
                        .accessibilityHidden(navigation.content != .details)
                    }
                    .frame(maxWidth: .infinity)
                    .clipped()
                }
            }
        }
        .background(palette.background)
        .environment(model)
        .overlay {
            if terminalsVisible {
                SessionsShortcutMonitor { _handle($0) }
                    .allowsHitTesting(false).frame(width: 0, height: 0)
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .multiplexerStoreDidChange)) { notification in
            guard let source = notification.object as? MultiplexerStore else { return }
            layoutRevision += 1
            if source === multiplexer {
                rememberTaskPane()
            }
        }
        .onChange(of: model.sessions.value, initial: true) { _, records in
            guard let records else { return }
            let previous = Set(store.sessions.map(\.id))
            store.reconcile(records)
            let ids = Set(store.sessions.map(\.id))
            for id in previous.subtracting(ids) { store.releaseSurface(id) }
            workspaces.reconcileSessions(ids, in: worktreeLayout.knownPaths)
        }
        .onChange(of: store.sessions.map(\.id)) { previous, ids in
            for id in Set(previous).subtracting(ids) { store.releaseSurface(id) }
            workspaces.reconcileSessions(Set(ids), in: worktreeLayout.knownPaths)
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
        rememberTaskPane()
        Task { @MainActor in await store.select(record.id) }
    }

    private func openTask(_ work: WorkReference) {
        model.select(work)
        if let saved = navigation.taskPanes[work.id],
           let pane = workspaces.workspace(for: saved.path).multiplexer.layout.pane(for: saved.pane.id),
           pane == saved.pane {
            worktreeLayout.select(saved.path)
            multiplexer.setFocusedPane(saved.pane.id)
            navigation.content = .terminals
            if case .shell = multiplexer.focusedPane.content {
                store.surfaces.focus(.shell(saved.pane.id))
            } else if case .session(let id) = multiplexer.focusedPane.content {
                navigation.selectedSessionId = id
                store.surfaces.focus(.session(id))
            }
            if case .monitor = multiplexer.focusedPane.content {
                Task { await model.refreshActiveRuns() }
            }
        } else {
            showMonitor(work.id)
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
        rememberTaskPane()
        Task { await model.refreshActiveRuns() }
    }

    private func rememberTaskPane() {
        guard navigation.content == .terminals,
              let work = model.selection, work.kind == .task,
              let path = worktreeLayout.focusedPath else { return }
        if case .monitor(let id) = multiplexer.focusedPane.content, id != work.id { return }
        if case .session(let id) = multiplexer.focusedPane.content,
           model.workspace.subject(for: id) != work { return }
        guard multiplexer.focusedPane.content != .empty else { return }
        navigation.taskPanes[work.id] = (path, multiplexer.focusedPane)
    }

    private func taskControls(_ taskId: String) -> some View {
        let work = model.task(id: taskId)?.task.runtime.map { WorkReference.task(id: $0.workId) }
        let records = store.sessions.filter { work != nil && $0.record.work == work }
        return HStack(spacing: 12) {
            Menu("Sessions") {
                ForEach(records) { item in
                    Button(item.record.title) { openSession(item.record) }
                }
            }
            .disabled(records.isEmpty)
            .accessibilityIdentifier("task-sessions-\(taskId)")
            Button("Monitor") { showMonitor(taskId) }
                .accessibilityIdentifier("task-show-monitor-\(taskId)")
            Spacer()
            Button("Inspect") { model.select(.task(id: taskId)) }
        }
        .font(.system(size: 12))
        .padding(8)
    }

    private func _pane(for sessionId: String) -> PaneState? {
        guard let record = store.sessions.first(where: { $0.id == sessionId })?.record else { return nil }
        if case .shell(let id) = store.localTerminal(for: record),
           let path = workspaces.path(containingShell: id) {
            return workspaces.workspace(for: path).multiplexer.layout.pane(for: id)
        }
        return workspaces.workspace(for: record.cwd).multiplexer.pane(forSessionId: sessionId)
    }

    private func _status(_ item: SessionItem, pane: PaneState?) -> String {
        let visible = pane.map { pane in
            worktreeLayout.layout.slots.contains { slot in
                guard let path = slot.path else { return false }
                return workspaces.workspace(for: path).multiplexer.layout.pane(for: pane.id) != nil
            }
        } ?? false
        return sessionRowStatus(item, hasOpenPane: visible)
    }

    private func startConversation() {
        guard let conversationScope = model.conversationScope else { return }
        do {
            let lf = try LocalWaveAgentLauncher.controlLfPath()
            worktreeLayout.select(conversationScope.repoPath)
            navigation.content = .terminals
            multiplexer.newShell(command: ConversationLaunch(scope: conversationScope).arguments(lf: lf))
        } catch {
            launchError = error.localizedDescription
        }
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
    let paths: [String]
    let isActive: Bool
    @ObservedObject var sessions: SessionsStore

    var body: some View { content }

    private var content: AnyView {
        switch node {
        case .leaf(let id, let path):
            return AnyView(VStack(spacing: 0) {
                HStack(spacing: Spacing.sm) {
                    Menu {
                        ForEach(paths, id: \.self) { candidate in
                            Button(URL(fileURLWithPath: candidate).lastPathComponent) {
                                layout.select(candidate, in: id)
                            }
                        }
                    } label: {
                        Label(path.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Choose worktree", systemImage: "folder")
                    }
                    .help(path ?? "Choose a worktree for this slot")
                    Spacer()
                    if let path {
                        Button {
                            layout.focus(id)
                            workspaces.workspace(for: path).multiplexer.newShell()
                        } label: { Image(systemName: "terminal.fill") }
                        .help("New terminal in this worktree")
                        .accessibilityLabel("New terminal in worktree")
                    }
                    Button { layout.split(id, axis: .vertical) } label: {
                        Image(systemName: "rectangle.split.2x1")
                    }
                    .help("Split worktrees right")
                    .accessibilityLabel("Split worktrees right")
                    Button { layout.split(id, axis: .horizontal) } label: {
                        Image(systemName: "rectangle.split.1x2")
                    }
                    .help("Split worktrees down")
                    .accessibilityLabel("Split worktrees down")
                    Button { layout.close(id) } label: { Image(systemName: "xmark") }
                        .help("Hide this worktree; its terminals keep running")
                        .accessibilityLabel("Hide worktree")
                }
                .buttonStyle(.plain)
                .font(Typography.body(11).weight(.semibold))
                .padding(Spacing.sm)
                .background(layout.focusedSlotId == id ? Color.loopflowBurgundy.opacity(0.15) : Color.clear)
                if let path {
                    WorktreeTerminalsView(
                        workspace: workspaces.workspace(for: path), path: path,
                        isFocused: isActive && layout.focusedSlotId == id,
                        sessions: sessions
                    )
                    .id(path)
                    .simultaneousGesture(TapGesture().onEnded { layout.focus(id) })
                } else {
                    ContentUnavailableView("Choose a worktree", systemImage: "folder", description: Text("Select a worktree above to restore its conversation and terminals."))
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
        WorktreeNodeView(node: node, layout: layout, workspaces: workspaces, paths: paths, isActive: isActive,
                         sessions: sessions)
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
            workingDirectory: path, sessions: sessions, store: store
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

    var body: some View {
        Group {
            if let zoomedPaneId, let pane = layout.pane(for: zoomedPaneId) {
                SessionPaneView(
                    pane: pane,
                    isFocused: !focusedPaneId.isEmpty,
                    workingDirectory: workingDirectory,
                    sessions: sessions,
                    store: store
                )
            } else {
                MultiplexerNodeView(
                    node: layout,
                    focusedPaneId: focusedPaneId,
                    workingDirectory: workingDirectory,
                    sessions: sessions,
                    store: store
                )
            }
        }
        .background(LoopflowPalette.dark.background)
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
                    store: store
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
            store: store
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
            .fill(Color.white.opacity(0.09))
            .frame(
                width: axis == .vertical ? 6 : nil,
                height: axis == .horizontal ? 6 : nil
            )
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
    let pane: PaneState
    let isFocused: Bool
    let workingDirectory: String
    @ObservedObject var sessions: SessionsStore
    let store: MultiplexerStore
    @State private var bellRinging = false
    @State private var terminalTitle: String?
    @State private var isCompleting = false

    private var item: SessionItem? {
        guard case .session(let id) = pane.content else { return nil }
        return sessions.sessions.first { $0.id == id }
    }

    private var paneColor: Color { store.color(for: pane.id).color }

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider().overlay(Color.white.opacity(0.08))
            content
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(LoopflowPalette.dark.background)
        .overlay {
            Rectangle()
                .strokeBorder(isFocused ? paneColor : Color.clear, lineWidth: 2)
        }
        .overlay(alignment: .bottomTrailing) {
            resolutionControls
        }
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

    private var header: some View {
        HStack(spacing: Spacing.sm) {
            Circle().fill(paneColor).frame(width: 8, height: 8)
            Text(_title)
                .font(Typography.code(10).weight(.semibold))
                .foregroundStyle(.white)
                .lineLimit(1)
            if case .shell = pane.content {
                Image(systemName: "questionmark.circle")
                    .font(Typography.caption(9))
                    .foregroundStyle(.white.opacity(0.45))
                    .help(
                        "Click a completed command block to select its command and output; "
                            + "Command-C copies the block. Command-Up/Down navigates prompts."
                    )
                    .accessibilityLabel("Shell command selection help")
                    .accessibilityIdentifier("shell-command-selection-help")
            }
            Spacer()
            if bellRinging {
                Image(systemName: "bell.fill")
                    .foregroundStyle(Color.statusWarning)
                    .accessibilityLabel("Session ready")
            }
            _headerButton(
                "rectangle.split.2x1",
                label: "Split right",
                id: "pane-split-right",
                help: "Split right (⌘D)"
            ) {
                _ = store.split(pane.id, axis: .vertical)
            }
            _headerButton(
                "rectangle.split.1x2",
                label: "Split down",
                id: "pane-split-down",
                help: "Split down (⌘⇧D)"
            ) {
                _ = store.split(pane.id, axis: .horizontal)
            }
            _headerButton(
                "xmark",
                label: "Close view",
                id: "pane-close",
                help: pane.content == .shell
                    ? "Close this shell (⌘W)"
                    : "Close this view; the Session keeps running (⌘W)",
                disabled: !store.canClose
            ) {
                if case .shell = pane.content {
                    sessions.surfaces.release(.shell(pane.id))
                }
                store.close(pane.id)
            }
            _headerButton(
                store.zoomedPaneId == pane.id
                    ? "arrow.down.right.and.arrow.up.left"
                    : "arrow.up.left.and.arrow.down.right",
                label: store.zoomedPaneId == pane.id ? "Exit zoom" : "Zoom pane",
                id: "pane-zoom",
                help: "Toggle pane zoom (⌘⇧Return)"
            ) {
                store.toggleZoom(pane.id)
            }
        }
        .buttonStyle(.plain)
        .foregroundStyle(.white.opacity(0.6))
        .padding(.horizontal, Spacing.sm)
        .frame(height: 34)
        .background(isFocused ? paneColor.opacity(0.12) : Color.clear)
        .contentShape(Rectangle())
        .onTapGesture { store.setFocusedPane(pane.id) }
    }

    /// One pane-header icon action with a hit target larger than its glyph.
    private func _headerButton(
        _ systemImage: String,
        label: String,
        id: String,
        help: String,
        disabled: Bool = false,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .frame(width: 24, height: 24)
                .contentShape(Rectangle())
        }
        .disabled(disabled)
        .help(help)
        .accessibilityLabel(label)
        .accessibilityIdentifier(id)
    }

    private var paneSessions: [SessionItem] {
        if case .shell = pane.content {
            return sessions.sessions.filter {
                sessions.localTerminal(for: $0.record) == .shell(pane.id)
            }
        }
        return item.map { [$0] } ?? []
    }

    private var completionItems: [SessionItem] {
        // Placeholder panes lead with their opening or recovery action.
        paneSessions.filter { $0.record.action(.complete) != nil && $0.surface != nil }
    }

    private var resolutionControls: some View {
        VStack(alignment: .trailing, spacing: Spacing.sm) {
            ForEach(paneSessions) { item in
                if let error = item.resolutionError {
                    Text(error)
                        .font(Typography.caption(11))
                        .foregroundStyle(Color.statusWarning)
                        .padding(Spacing.sm)
                        .background(LoopflowPalette.dark.background)
                }
            }
            ForEach(completionItems) { item in
                if let action = item.record.action(.complete) {
                    completionButton(item, action: action)
                }
            }
        }
    }

    private func completionButton(_ item: SessionItem, action: SessionAction) -> some View {
        Button {
            isCompleting = true
            Task { @MainActor in
                defer { isCompleting = false }
                guard await sessions.complete(item.id) else { return }
                store.reconcileSessions(Set(sessions.sessions.map(\.id)))
                sessions.releaseSurface(item.id)
            }
        } label: {
            HStack(spacing: Spacing.sm) {
                if isCompleting {
                    ProgressView()
                        .controlSize(.small)
                        .tint(.white)
                } else {
                    Image(systemName: "checkmark")
                        .font(.system(size: 14, weight: .bold))
                }
                Text(completionItems.count > 1 ? "\(action.label) · \(item.record.title)" : action.label)
                    .font(Typography.body(13).weight(.bold))
            }
            .foregroundStyle(.white)
            .padding(.horizontal, Spacing.xl)
            .frame(height: 46)
            .background(Color.statusSuccess, in: Capsule())
            .overlay {
                Capsule().strokeBorder(Color.white.opacity(0.18), lineWidth: 1)
            }
            .shadow(color: .black.opacity(0.35), radius: 12, y: 4)
        }
        .buttonStyle(.plain)
        .disabled(isCompleting || action.unavailableReason != nil)
        .help(action.unavailableReason ?? action.help)
        .accessibilityLabel(completionItems.count > 1 ? "Complete session: \(item.record.title)" : "Complete session")
        .accessibilityHint(action.unavailableReason ?? action.help)
        .accessibilityIdentifier("session-action-complete")
        .padding(Spacing.xxl)
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
                onFocus: { store.setFocusedPane(pane.id) }
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
            .buttonStyle(.borderedProminent)
            .tint(Color.loopflowBurgundy)
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
            .buttonStyle(.borderedProminent)
            .tint(Color.loopflowBurgundy)
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
                    .buttonStyle(.borderedProminent)
                    .tint(Color.loopflowBurgundy)
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
            onFocus: { store.setFocusedPane(pane.id) }
        )
        .id(surface.id)
    }

    private var _title: String {
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

/// Sidebar badge: VIEWING means this window shows the terminal in a pane,
/// RUNNING means a live surface is retained without a visible pane, and
/// ELSEWHERE means another client (Warp, another window, SSH) holds it.
func sessionRowStatus(_ item: SessionItem, hasOpenPane: Bool) -> String {
    if hasOpenPane, item.surface != nil { return "VIEWING" }
    return switch item.state {
    case .pending: item.statusLabel
    case .elsewhere: "ELSEWHERE"
    case .opening, .prepared: "OPENING…"
    case .live: item.record.kind == .interactive ? "RUNNING" : item.statusLabel
    case .failed: "RETRY"
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
            if modifiers == [.command], event.charactersIgnoringModifiers == "d" {
                return .splitRight
            }
            if modifiers == [.command, .shift], event.charactersIgnoringModifiers == "d" {
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

private extension PaneColor {
    var color: Color {
        switch self {
        case .blue: Color(hex: 0x69A9E6)
        case .amber: Color(hex: 0xE4B45F)
        case .green: Color(hex: 0x7FB987)
        case .rose: Color(hex: 0xD98291)
        case .violet: Color(hex: 0xA78BD4)
        case .cyan: Color(hex: 0x68BFC1)
        }
    }
}
#endif
