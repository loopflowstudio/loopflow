#if os(macOS)
import AppKit
import Combine
import Loopflow
import OSLog
import SwiftUI

extension Notification.Name {
    static let openSessions = Notification.Name("loopflow.openSessions")
}

enum SessionScope: Hashable, Sendable {
    case repo(String)
    case wave(repo: String, id: String)
    case project(repo: String, id: String)
    case task(repo: String, id: String)

    var repoPath: String {
        switch self {
        case .repo(let path): path
        case .wave(let path, _), .project(let path, _), .task(let path, _): path
        }
    }

    var label: String {
        switch self {
        case .repo(let path): URL(fileURLWithPath: path).lastPathComponent
        case .wave(_, let id): id
        case .project(_, let id): id
        case .task(_, let id): id
        }
    }

    func resolvingRepository() -> SessionScope {
        let path = WaveOrigin.resolve(repoPath)
        return switch self {
        case .repo:
            .repo(path)
        case .wave(_, let id):
            .wave(repo: path, id: id)
        case .project(_, let id):
            .project(repo: path, id: id)
        case .task(_, let id):
            .task(repo: path, id: id)
        }
    }

    func includes(_ record: SessionRecord) -> Bool {
        switch self {
        case .repo:
            true
        case .wave(_, let id):
            record.work?.kind == .wave && record.work?.id == id
        case .project(_, let id):
            record.work?.kind == .project && record.work?.id == id
        case .task(_, let id):
            record.work?.kind == .task && record.work?.id == id
        }
    }
}

struct SessionItem: Identifiable, Equatable {
    enum State: Equatable {
        case pending
        case opening
        case live
        case failed(String)
    }

    var record: SessionRecord
    var state: State

    var id: String { record.id }
    var label: String { record.title }
    var work: WorkReference? { record.work }

    var statusLabel: String {
        switch record.state {
        case .waiting: "WAITING"
        case .active: "ACTIVE"
        case .ready: "READY"
        case .closed: "CLOSED"
        }
    }

    var step: String { record.detail }

    var surface: SessionRecord? {
        guard case .live = state else { return nil }
        return record
    }

    var error: String? {
        guard case .failed(let message) = state else { return nil }
        return message
    }
}

@MainActor
final class SessionsStore: ObservableObject {
    @Published private(set) var sessions: [SessionItem] = []
    @Published private(set) var hasLoaded = false
    @Published var pollError: String?

    private let scope: SessionScope
    private let query: RegistryQuery
    private let metrics: SessionsLatencyMetrics
    private var hasRecordedSessionsLoad = false
    private var requestedSessionId: String?

    init(
        scope: SessionScope,
        query: RegistryQuery = RegistryQueryLocal.shared,
        initialRecords: [SessionRecord]? = nil
    ) {
        let scope = scope.resolvingRepository()
        self.scope = scope
        self.query = query
        metrics = SessionsLatencyMetrics(scope: scope.label)
        if let initialRecords {
            hasLoaded = true
            reconcile(initialRecords)
            metrics.recordSessionsLoaded(count: sessions.count)
            hasRecordedSessionsLoad = true
        }
    }

    func refresh() async {
        do {
            let records = try await query.sessions(cwd: scope.repoPath)
            pollError = nil
            reconcile(records)
            hasLoaded = true
            if !hasRecordedSessionsLoad {
                metrics.recordSessionsLoaded(count: sessions.count)
                hasRecordedSessionsLoad = true
            }
        } catch {
            pollError = error.localizedDescription
            hasLoaded = true
        }
    }

    func reconcile(_ records: [SessionRecord]) {
        let filtered = records.filter(scope.includes)
        let incoming = Set(filtered.map(\.id))
        sessions.removeAll { !incoming.contains($0.id) }

        for record in filtered {
            if let index = _index(record.id) {
                sessions[index].record = record
                if record.kind != .interactive {
                    if record.state != .waiting {
                        sessions[index].state = .live
                    } else if case .live = sessions[index].state {
                        sessions[index].state = .pending
                    }
                }
            } else {
                sessions.append(
                    SessionItem(
                        record: record,
                        state: record.kind == .interactive || record.state == .waiting
                            ? .pending
                            : .live
                    )
                )
            }
        }
    }

    func recover(_ id: String) async -> SessionRecord? {
        guard let index = _index(id) else { return nil }
        switch sessions[index].state {
        case .pending, .failed:
            sessions[index].state = .opening
        case .opening, .live:
            return nil
        }
        do {
            let surface = try await query.openSession(
                id: id,
                replacing: sessions[index].record.kind == .interactive,
                cwd: scope.repoPath
            )
            guard let latest = _index(id) else { return nil }
            if case .opening = sessions[latest].state {
                sessions[latest].record = surface
                sessions[latest].state = .live
            }
            return surface
        } catch {
            guard let latest = _index(id) else { return nil }
            sessions[latest].state = .failed(error.localizedDescription)
            return nil
        }
    }

    func select(_ id: String) async -> SessionRecord? {
        guard let index = _index(id) else { return nil }
        requestedSessionId = id
        if let surface = sessions[index].surface {
            requestedSessionId = nil
            return surface
        }
        if case .opening = sessions[index].state {
            return nil
        }

        let surface = await recover(id)
        guard requestedSessionId == id else { return nil }
        requestedSessionId = nil
        return surface
    }

    func beginPaneLoad(_ id: String) {
        metrics.beginPaneLoad(id)
    }

    func recordPaneLive(_ id: String) {
        metrics.recordPaneLive(id)
    }

    func decideFlow(_ id: String, approving: Bool, text: String) async -> Bool {
        guard _index(id) != nil else { return false }
        do {
            try await query.resolveFlowSession(
                id: id,
                approving: approving,
                text: text,
                cwd: scope.repoPath
            )
            await refresh()
            return true
        } catch {
            guard let latest = _index(id) else { return false }
            sessions[latest].state = .failed(error.localizedDescription)
            return false
        }
    }

    func complete(_ id: String) async -> Bool {
        guard _index(id) != nil else { return false }
        do {
            try await query.completeSession(id: id, cwd: scope.repoPath)
            await refresh()
            return true
        } catch {
            guard let latest = _index(id) else { return false }
            sessions[latest].state = .failed(error.localizedDescription)
            return false
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
    private let scope: SessionScope
    private let multiplexer: MultiplexerStore
    private let onShowWork: () -> Void
    private let query: RegistryQuery

    @StateObject private var store: SessionsStore
    @State private var layoutSnapshot: LayoutNode
    @State private var focusedPaneId: String
    @State private var zoomedPaneId: String?
    /// Session work id → its Wave/Project/Task, resolved from the roadmap so a
    /// row shows what it *is* instead of an opaque `task_…` id.
    @State private var hierarchy: [String: SessionContext] = [:]
    @Environment(\.palette) private var palette

    init(
        scope: SessionScope,
        query: RegistryQuery = RegistryQueryLocal.shared,
        initialRecords: [SessionRecord]? = nil,
        onShowWork: @escaping () -> Void = {}
    ) {
        let scope = scope.resolvingRepository()
        self.scope = scope
        self.onShowWork = onShowWork
        self.query = query
        let multiplexer = MultiplexerStore()
        self.multiplexer = multiplexer
        _store = StateObject(
            wrappedValue: SessionsStore(
                scope: scope,
                query: query,
                initialRecords: initialRecords
            )
        )
        _layoutSnapshot = State(initialValue: multiplexer.layout)
        _focusedPaneId = State(initialValue: multiplexer.focusedPaneId)
        _zoomedPaneId = State(initialValue: multiplexer.zoomedPaneId)
    }

    var body: some View {
        HSplitView {
            sidebar
                .frame(minWidth: 220, idealWidth: 275, maxWidth: 360)
            MultiplexerView(
                layout: layoutSnapshot,
                focusedPaneId: focusedPaneId,
                zoomedPaneId: zoomedPaneId,
                scope: scope,
                sessions: store,
                store: multiplexer
            )
            .frame(minWidth: 480, maxWidth: .infinity, maxHeight: .infinity)
        }
        .background(palette.background)
        .overlay {
            SessionsShortcutMonitor { shortcut in
                _handle(shortcut)
            }
            .allowsHitTesting(false)
            .frame(width: 0, height: 0)
        }
        .onReceive(
            NotificationCenter.default.publisher(for: .multiplexerStoreDidChange)
        ) { notification in
            guard let source = notification.object as? MultiplexerStore,
                  source === multiplexer else { return }
            layoutSnapshot = multiplexer.layout
            focusedPaneId = multiplexer.focusedPaneId
            zoomedPaneId = multiplexer.zoomedPaneId
        }
        .onChange(of: store.sessions.map(\.id)) { _, ids in
            multiplexer.reconcileSessions(Set(ids))
        }
        .task {
            // Sessions are the primary content. Do not hold the list behind
            // the slower roadmap query used only to enrich its group labels.
            await store.refresh()
            await _loadHierarchy()
            while !Task.isCancelled {
                do {
                    try await Task.sleep(for: .seconds(2))
                } catch {
                    return
                }
                await store.refresh()
                // Resolve any newly-seen session whose task we don't know yet.
                if store.sessions.contains(where: { item in
                    item.work.map { hierarchy[$0.id] == nil } ?? false
                }) {
                    await _loadHierarchy()
                }
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("sessions-surface")
    }

    private struct SessionContext: Equatable {
        let wave: String
        let project: String
        let identifier: String
        let workName: String
    }

    /// Resolve every bound Session to its Wave/Project/Task via the roadmap.
    private func _loadHierarchy() async {
        guard let snapshot = try? await query.roadmap() else { return }
        var index: [String: SessionContext] = [:]
        for wave in snapshot.waves {
            index[wave.wave.id] = SessionContext(
                wave: wave.wave.name,
                project: "Wave",
                identifier: wave.wave.name,
                workName: wave.wave.name
            )
            for project in wave.projects.items {
                let projectContext = SessionContext(
                    wave: wave.wave.name,
                    project: project.project.name,
                    identifier: project.project.slug,
                    workName: project.project.name
                )
                index[project.project.id] = projectContext
                if let workId = project.runtime?.workId { index[workId] = projectContext }
                for task in project.tasks {
                    let context = SessionContext(
                        wave: wave.wave.name,
                        project: project.project.name,
                        identifier: task.task.identifier,
                        workName: task.task.name
                    )
                    // A session's work id is the DURABLE id (`task_…`), which the
                    // roadmap carries on `runtime.work_id` — not `task.id` (a UUID).
                    if let workId = task.runtime?.workId { index[workId] = context }
                    index[task.task.id] = context
                }
            }
        }
        if !index.isEmpty { hierarchy = index }
    }

    private var sidebar: some View {
        VStack(spacing: 0) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: Spacing.xxs) {
                    Text("SESSIONS")
                        .font(Typography.caption(9).weight(.bold))
                        .tracking(1.4)
                        .foregroundStyle(palette.textSecondary)
                    Text(scope.label)
                        .font(Typography.sectionTitle(17))
                        .foregroundStyle(palette.text)
                        .lineLimit(1)
                }
                Spacer()
                if let pollError = store.pollError {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(Color.statusWarning)
                        .help(pollError)
                }
            }
            .padding(Spacing.md)

            Divider()

            if !store.hasLoaded {
                ProgressView("Loading sessions…")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                sessionList
            }

            Divider()
            VStack(spacing: Spacing.xs) {
                Button {
                    multiplexer.newShell()
                } label: {
                    Label("New shell", systemImage: "plus")
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("sessions-new-shell")

                Button(action: onShowWork) {
                    Label("Waves & roadmap", systemImage: "water.waves")
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .buttonStyle(.plain)
            }
            .font(Typography.body(11).weight(.semibold))
            .foregroundStyle(palette.text)
            .padding(Spacing.md)
        }
        .background(palette.surface)
    }

    // Group Work-bound sessions by Wave › Project. Unbound Run sessions stay
    // visible under Other.
    private struct SessionGroup: Identifiable {
        let wave: String
        let project: String
        let items: [SessionRowItem]
        var id: String { "\(wave)/\(project)" }
    }

    private struct SessionRowItem: Identifiable {
        let item: SessionItem
        let context: SessionContext?
        var id: String { "\(item.id)#\(item.record.title)" }
    }

    private func _groupedSessions() -> [SessionGroup] {
        var order: [String] = []
        var buckets: [String: (wave: String, project: String, items: [SessionRowItem])] = [:]
        for item in store.sessions {
            let context = item.work.flatMap { hierarchy[$0.id] }
            let row = SessionRowItem(item: item, context: context)
            let wave = context?.wave ?? "—"
            let project = context?.project ?? "Other"
            let key = "\(wave)/\(project)"
            if buckets[key] == nil {
                buckets[key] = (wave, project, [])
                order.append(key)
            }
            buckets[key]?.items.append(row)
        }
        return order.compactMap { key in
            buckets[key].map { SessionGroup(wave: $0.wave, project: $0.project, items: $0.items) }
        }
    }

    private var sessionList: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: Spacing.xs) {
                if store.sessions.isEmpty {
                    _emptyState
                } else {
                    ForEach(_groupedSessions()) { group in
                        _groupHeader(group.wave, project: group.project)
                        ForEach(group.items) { row in
                            _sessionRow(row)
                        }
                    }
                }
            }
            .padding(Spacing.sm)
        }
    }

    private func _groupHeader(_ wave: String, project: String) -> some View {
        HStack(spacing: Spacing.xxs) {
            if wave == "—" {
                Text(project)
                    .font(Typography.caption(9).weight(.bold))
                    .foregroundStyle(palette.textSecondary)
            } else {
                Text(wave)
                    .font(Typography.caption(9).weight(.bold))
                    .foregroundStyle(Color.loopflowBurgundy)
                Image(systemName: "chevron.right")
                    .font(.system(size: 6, weight: .bold))
                    .foregroundStyle(palette.textSecondary.opacity(0.5))
                Text(project)
                    .font(Typography.caption(9).weight(.semibold))
                    .foregroundStyle(palette.textSecondary)
            }
            Spacer()
        }
        .padding(.horizontal, Spacing.sm)
        .padding(.top, Spacing.sm)
        .padding(.bottom, Spacing.xxs)
    }

    @ViewBuilder
    private var _emptyState: some View {
        if let pollError = store.pollError {
            ContentUnavailableView(
                "Sessions unavailable",
                systemImage: "exclamationmark.triangle",
                description: Text(pollError)
            )
            .frame(maxWidth: .infinity)
            .padding(.top, Spacing.xl)
        } else {
            ContentUnavailableView(
                "No sessions",
                systemImage: "checkmark.circle",
                description: Text("Interactive runs and human handoffs appear here until completed.")
            )
            .frame(maxWidth: .infinity)
            .padding(.top, Spacing.xl)
        }
    }

    private func _sessionRow(_ row: SessionRowItem) -> some View {
        let item = row.item
        let pane = _pane(for: item.id)
        let isFocused = pane?.id == focusedPaneId
        let color = pane.map { multiplexer.color(for: $0.id).color }
        return Button {
            if pane == nil {
                store.beginPaneLoad(item.id)
            }
            Task { @MainActor in
                guard await store.select(item.id) != nil,
                      let selected = store.sessions.first(where: { $0.id == item.id })
                else { return }
                _load(selected)
            }
        } label: {
            HStack(alignment: .top, spacing: Spacing.sm) {
                Circle()
                    .fill(color ?? _stateColor(item.state))
                    .frame(width: 8, height: 8)
                    .padding(.top, 4)
                VStack(alignment: .leading, spacing: Spacing.xxs) {
                    Text(item.record.title)
                        .font(Typography.body(11).weight(.semibold))
                        .foregroundStyle(palette.text)
                        .lineLimit(2)
                    HStack(spacing: Spacing.xs) {
                        Text(row.context?.identifier ?? item.record.work?.id ?? "Run")
                            .font(Typography.caption(9))
                            .foregroundStyle(palette.textSecondary)
                        Text(item.step)
                            .font(Typography.caption(8))
                            .fontWeight(.semibold)
                            .foregroundStyle(palette.textSecondary)
                            .padding(.horizontal, Spacing.xxs)
                            .padding(.vertical, 1)
                            .background(palette.textSecondary.opacity(0.14), in: Capsule())
                    }
                    .lineLimit(1)
                    if let error = item.error {
                        Text(error)
                            .font(Typography.caption(8))
                            .foregroundStyle(Color.statusWarning)
                            .lineLimit(2)
                    }
                }
                Spacer(minLength: Spacing.xs)
                Text(_status(item, pane: pane))
                    .font(Typography.caption(8).weight(.bold))
                    .foregroundStyle(isFocused ? (color ?? palette.textSecondary) : palette.textSecondary)
            }
            .padding(Spacing.sm)
            .background(
                (isFocused ? (color ?? Color.clear).opacity(0.12) : Color.clear),
                in: RoundedRectangle(cornerRadius: 8)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(item.label), \(_status(item, pane: pane))")
        .accessibilityIdentifier("session-row-\(item.id)")
    }

    private func _pane(for sessionId: String) -> PaneState? {
        multiplexer.pane(forSessionId: sessionId)
    }

    private func _status(_ item: SessionItem, pane: PaneState?) -> String {
        if pane != nil { return "OPEN" }
        return switch item.state {
        case .pending: item.statusLabel
        case .opening: "OPENING…"
        case .live: item.statusLabel
        case .failed: "RETRY"
        }
    }

    private func _stateColor(_ state: SessionItem.State) -> Color {
        switch state {
        case .pending, .opening: palette.textSecondary.opacity(0.45)
        case .live: Color.statusSuccess
        case .failed: Color.statusWarning
        }
    }

    private func _load(_ item: SessionItem) {
        if multiplexer.pane(forSessionId: item.id) == nil {
            _destroySurface(in: multiplexer.focusedPane)
        }
        multiplexer.load(sessionId: item.id)
    }

    private func _destroySurface(in pane: PaneState) {
        switch pane.content {
        case .empty:
            return
        case .shell:
            GhosttyManager.shared.destroySession("shell-pane-\(pane.id)")
        case .session(let id):
            guard let item = store.sessions.first(where: { $0.id == id }),
                  let surface = item.surface
            else { return }
            GhosttyManager.shared.destroySession(
                _terminalSurfaceId(sessionId: surface.id)
            )
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

private struct MultiplexerView: View {
    let layout: LayoutNode
    let focusedPaneId: String
    let zoomedPaneId: String?
    let scope: SessionScope
    @ObservedObject var sessions: SessionsStore
    let store: MultiplexerStore

    var body: some View {
        Group {
            if let zoomedPaneId, let pane = layout.pane(for: zoomedPaneId) {
                SessionPaneView(
                    pane: pane,
                    isFocused: true,
                    scope: scope,
                    sessions: sessions,
                    store: store
                )
            } else {
                MultiplexerNodeView(
                    node: layout,
                    focusedPaneId: focusedPaneId,
                    scope: scope,
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
    let scope: SessionScope
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
                    scope: scope,
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
            scope: scope,
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

private enum FlowResolutionAction {
    case approve
    case iterate

    var title: String {
        switch self {
        case .approve: "Approve and continue"
        case .iterate: "Iterate"
        }
    }
}

private struct SessionPaneView: View {
    let pane: PaneState
    let isFocused: Bool
    let scope: SessionScope
    @ObservedObject var sessions: SessionsStore
    let store: MultiplexerStore
    @State private var bellRinging = false
    @State private var terminalTitle: String?
    @State private var resolutionAction: FlowResolutionAction?
    @State private var resolutionText = ""
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
            completionAction
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("\(_title), \(isFocused ? "active" : "open")")
        .accessibilityIdentifier(item.map { "session-pane-\($0.id)" } ?? "session-pane-empty")
        .onReceive(NotificationCenter.default.publisher(for: .ghosttySessionBell)) { notification in
            guard notification.object as? String == _surfaceId else { return }
            bellRinging = true
        }
        .onReceive(NotificationCenter.default.publisher(for: .ghosttySessionTitle)) { notification in
            guard let update = notification.object as? GhosttySessionTitle,
                  update.sessionId == _surfaceId else { return }
            terminalTitle = update.title
        }
        .onChange(of: isFocused) { _, focused in
            if focused { bellRinging = false }
        }
        .onChange(of: _surfaceId) { _, _ in
            bellRinging = false
            terminalTitle = nil
        }
        .alert(
            resolutionAction?.title ?? "Task decision",
            isPresented: Binding(
                get: { resolutionAction != nil },
                set: { if !$0 { resolutionAction = nil } }
            )
        ) {
            TextField(
                resolutionAction == .approve ? "Verified summary" : "Direction",
                text: $resolutionText
            )
            Button(resolutionAction?.title ?? "Continue") {
                guard let action = resolutionAction, let item else { return }
                let text = resolutionText.trimmingCharacters(in: .whitespacesAndNewlines)
                resolutionAction = nil
                Task { @MainActor in
                    let decided = await sessions.decideFlow(
                        item.id,
                        approving: action == .approve,
                        text: text
                    )
                    if decided {
                        GhosttyManager.shared.destroySession(_surfaceId)
                    }
                }
            }
            .disabled(resolutionText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            Button("Cancel", role: .cancel) {
                resolutionAction = nil
            }
        } message: {
            Text("Approve advances the Task. Iterate returns it to autonomous work with your direction. Either action closes this review terminal.")
        }
    }

    private var header: some View {
        HStack(spacing: Spacing.sm) {
            Circle().fill(paneColor).frame(width: 8, height: 8)
            Text(_title)
                .font(Typography.code(10).weight(.semibold))
                .foregroundStyle(.white)
                .lineLimit(1)
            Spacer()
            if bellRinging {
                Image(systemName: "bell.fill")
                    .foregroundStyle(Color.statusWarning)
                    .accessibilityLabel("Session ready")
            }
            if let item, item.record.kind == .flow {
                Button {
                    resolutionText = item.record.readySummary ?? "Approved by User"
                    resolutionAction = .approve
                } label: {
                    Image(systemName: "checkmark.circle")
                }
                .disabled(item.record.state != .ready)
                .help(
                    item.record.state == .ready
                        ? "Approve and continue the Task"
                        : "The session agent has not marked this ready"
                )
                .accessibilityLabel("Approve and continue")
                .accessibilityIdentifier("session-action-approve")
                Button {
                    resolutionText = ""
                    resolutionAction = .iterate
                } label: {
                    Image(systemName: "arrow.uturn.backward.circle")
                }
                .help("Iterate through autonomous Task work")
                .accessibilityLabel("Iterate")
                .accessibilityIdentifier("session-action-iterate")
            }
            Button {
                _ = store.split(pane.id, axis: .vertical)
            } label: {
                Image(systemName: "rectangle.split.2x1")
            }
            .help("Split right (⌘D)")
            Button {
                _ = store.split(pane.id, axis: .horizontal)
            } label: {
                Image(systemName: "rectangle.split.1x2")
            }
            .help("Split down (⌘⇧D)")
            Button {
                GhosttyManager.shared.destroySession(_surfaceId)
                store.close(pane.id)
            } label: {
                Image(systemName: "xmark")
            }
            .disabled(!store.canClose)
            .help("Close this pane; the Session remains resumable (⌘W)")
            Button {
                store.toggleZoom(pane.id)
            } label: {
                Image(systemName: store.zoomedPaneId == pane.id
                    ? "arrow.down.right.and.arrow.up.left"
                    : "arrow.up.left.and.arrow.down.right")
            }
            .help("Toggle pane zoom (⌘⇧Return)")
        }
        .buttonStyle(.plain)
        .foregroundStyle(.white.opacity(0.6))
        .padding(.horizontal, Spacing.sm)
        .frame(height: 34)
        .background(isFocused ? paneColor.opacity(0.12) : Color.clear)
        .contentShape(Rectangle())
        .onTapGesture { store.setFocusedPane(pane.id) }
    }

    @ViewBuilder
    private var completionAction: some View {
        if let item, item.record.kind != .flow {
            Button {
                isCompleting = true
                Task { @MainActor in
                    let completed = await sessions.complete(item.id)
                    guard completed else {
                        isCompleting = false
                        return
                    }
                    GhosttyManager.shared.destroySession(_surfaceId)
                    store.close(pane.id)
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
                    Text("Complete")
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
            .disabled(isCompleting || (item.record.kind == .ask && item.record.state != .ready))
            .help(_completionHelp(item))
            .accessibilityLabel("Complete session")
            .accessibilityHint(_completionHelp(item))
            .accessibilityIdentifier("session-action-complete")
            .padding(Spacing.xxl)
        }
    }

    private func _completionHelp(_ item: SessionItem) -> String {
        switch item.record.kind {
        case .ask where item.record.state != .ready:
            "The session agent has not marked this ready"
        case .ask:
            "Complete the conversation and resume its blocked caller"
        case .interactive:
            "Stop the provider and remove this Session; native history remains resumable"
        case .flow:
            ""
        }
    }

    @ViewBuilder
    private var content: some View {
        switch pane.content {
        case .empty:
            emptyWorkspace
        case .session:
            if let item, let surface = item.surface {
                _terminal(item: item, surface: surface)
            } else {
                emptyWorkspace
            }
        case .shell:
            GhosttyTerminalView(
                workingDirectory: scope.repoPath,
                sessionId: "shell-pane-\(pane.id)",
                isFocused: isFocused,
                onFocus: { store.setFocusedPane(pane.id) }
            )
            .id(pane.id)
        }
    }

    private var emptyWorkspace: some View {
        ContentUnavailableView {
            Label("No session open", systemImage: "terminal")
        } description: {
            Text("Choose a session from the sidebar or start a shell.")
        } actions: {
            Button {
                store.newShell()
            } label: {
                Label("New shell", systemImage: "plus")
            }
            .buttonStyle(.borderedProminent)
            .tint(Color.loopflowBurgundy)
            .accessibilityIdentifier("sessions-empty-new-shell")
        }
    }

    @ViewBuilder
    private func _terminal(item: SessionItem, surface: SessionRecord) -> some View {
        GhosttyTerminalView(
            workingDirectory: scope.repoPath,
            argv: surface.openArgv,
            sessionId: _terminalSurfaceId(sessionId: surface.id),
            isFocused: isFocused,
            onSurfaceCreated: {
                sessions.recordPaneLive(item.id)
            },
            onFocus: { store.setFocusedPane(pane.id) }
        )
        .id(surface.id)
    }

    private var _title: String {
        if let terminalTitle, !terminalTitle.isEmpty { return terminalTitle }
        return switch pane.content {
        case .empty: "Workspace"
        case .session: item?.record.detail ?? "Workspace"
        case .shell: "Shell"
        }
    }

    private var _surfaceId: String {
        if let item, let surface = item.surface {
            return _terminalSurfaceId(sessionId: surface.id)
        }
        return "shell-pane-\(pane.id)"
    }
}

private func _terminalSurfaceId(sessionId: String) -> String {
    "human-session-\(sessionId)"
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
