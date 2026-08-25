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

    func includes(_ record: AskAttentionRecord) -> Bool {
        switch self {
        case .repo:
            true
        case .wave(_, let id):
            record.ask.origin.work.kind == .wave && record.ask.origin.work.id == id
        case .project(_, let id):
            record.ask.origin.work.kind == .project && record.ask.origin.work.id == id
        case .task(_, let id):
            record.ask.origin.work.kind == .task && record.ask.origin.work.id == id
        }
    }
}

struct SessionItem: Identifiable, Equatable {
    enum State: Equatable {
        case pending
        case opening
        case live(AskSessionRecord)
        case failed(String)
    }

    enum Presentation: Equatable {
        case needed
        case confirming
        case confirmed
    }

    var record: AskAttentionRecord
    var state: State
    var presentation = Presentation.needed

    var id: String { record.id }
    var label: String { record.ask.request.label }
    var work: WorkReference { record.ask.origin.work }

    /// The flow step this session is a gate for (e.g. `review-design`) — an
    /// explicit field, never the task's name.
    var step: String? {
        if case .flowStep(_, _, let skill, _) = record.ask.request { return skill }
        return nil
    }

    /// A free-text intervention's prompt, when the ask is not a flow step.
    var interventionPrompt: String? {
        if case .intervention(let prompt) = record.ask.request { return prompt }
        return nil
    }

    var surface: AskSessionRecord? {
        guard case .live(let surface) = state else { return nil }
        return surface
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
    private var openQueue: [String] = []
    private var openerRunning = false
    private var hasRecordedSessionsLoad = false

    init(
        scope: SessionScope,
        query: RegistryQuery = RegistryQueryLocal.shared,
        initialRecords: [AskAttentionRecord]? = nil
    ) {
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
            let records = try await query.userAskAttention(cwd: scope.repoPath)
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

    func reconcile(_ records: [AskAttentionRecord]) {
        let filtered = records.filter(scope.includes)
        let incoming = Set(filtered.map(\.id))
        sessions.removeAll { !incoming.contains($0.id) }

        for record in filtered {
            if let index = _index(record.id) {
                sessions[index].record = record
                if case .pending = sessions[index].state,
                   let surface = record.surface {
                    sessions[index].state = .live(surface)
                }
                if case .failed = sessions[index].state {
                    sessions[index].state = .pending
                    _enqueue(record.id, priority: record.attention != .queued)
                }
            } else {
                let state = record.surface.map(SessionItem.State.live) ?? .pending
                sessions.append(
                    SessionItem(
                        record: record,
                        state: state
                    )
                )
                if record.surface == nil {
                    _enqueue(record.id, priority: record.attention != .queued)
                }
            }
        }
        _startOpener()
    }

    func prioritize(_ id: String) {
        guard let index = _index(id), sessions[index].surface == nil else { return }
        if case .failed = sessions[index].state {
            sessions[index].state = .pending
        }
        openQueue.removeAll { $0 == id }
        openQueue.insert(id, at: 0)
        _startOpener()
    }

    func open(_ id: String) async {
        guard let index = _index(id) else { return }
        switch sessions[index].state {
        case .pending, .failed:
            sessions[index].state = .opening
        case .opening, .live:
            return
        }
        do {
            let surface = try await query.prepareAskOpen(
                askId: id,
                cwd: scope.repoPath
            )
            guard let latest = _index(id) else { return }
            sessions[latest].state = .live(surface)
        } catch {
            guard let latest = _index(id) else { return }
            sessions[latest].state = .failed(error.localizedDescription)
        }
    }

    func confirmPresented(_ id: String) async {
        guard let index = _index(id),
              let surface = sessions[index].surface,
              sessions[index].presentation == .needed
        else { return }
        sessions[index].presentation = .confirming
        do {
            _ = try await query.confirmAskPresented(
                askId: id,
                runId: surface.runId,
                cwd: scope.repoPath
            )
            if let latest = _index(id) {
                sessions[latest].presentation = .confirmed
            }
        } catch {
            if let latest = _index(id) {
                sessions[latest].presentation = .needed
            }
        }
    }

    func beginPaneLoad(_ id: String) {
        metrics.beginPaneLoad(id)
    }

    func recordPaneLive(_ id: String) {
        metrics.recordPaneLive(id)
    }

    private func _enqueue(_ id: String, priority: Bool) {
        guard !openQueue.contains(id) else { return }
        if priority {
            openQueue.insert(id, at: 0)
        } else {
            openQueue.append(id)
        }
    }

    private func _startOpener() {
        guard !openerRunning, !openQueue.isEmpty else { return }
        openerRunning = true
        Task {
            while let id = _nextPending() {
                await open(id)
            }
            openerRunning = false
        }
    }

    private func _nextPending() -> String? {
        while !openQueue.isEmpty {
            let id = openQueue.removeFirst()
            if let index = _index(id), case .pending = sessions[index].state {
                return id
            }
        }
        return nil
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
    private let onQueueChanged: () async -> Void
    private let onShowWork: () -> Void
    private let query: RegistryQuery

    @StateObject private var store: SessionsStore
    @State private var layoutSnapshot: LayoutNode
    @State private var focusedPaneId: String
    @State private var zoomedPaneId: String?
    @State private var pendingSessionId: String?
    /// Session work id → its Wave/Project/Task, resolved from the roadmap so a
    /// row shows what it *is* instead of an opaque `task_…` id.
    @State private var hierarchy: [String: SessionContext] = [:]
    @Environment(\.palette) private var palette

    init(
        scope: SessionScope,
        query: RegistryQuery = RegistryQueryLocal.shared,
        initialRecords: [AskAttentionRecord]? = nil,
        onQueueChanged: @escaping () async -> Void = {},
        onShowWork: @escaping () -> Void = {}
    ) {
        self.scope = scope
        self.onQueueChanged = onQueueChanged
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
        .onChange(of: store.sessions) { _, sessions in
            guard let pendingSessionId else { return }
            guard let item = sessions.first(where: { $0.id == pendingSessionId }) else {
                self.pendingSessionId = nil
                return
            }
            guard item.surface != nil else { return }
            _load(item)
            self.pendingSessionId = nil
        }
        .task {
            await _loadHierarchy()
            while !Task.isCancelled {
                await store.refresh()
                multiplexer.reconcileSessions(Set(store.sessions.map(\.id)))
                await onQueueChanged()
                // Resolve any newly-seen session whose task we don't know yet.
                if store.sessions.contains(where: { hierarchy[$0.work.id] == nil }) {
                    await _loadHierarchy()
                }
                do {
                    try await Task.sleep(for: .seconds(2))
                } catch {
                    return
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
        let taskName: String
    }

    /// Resolve every session's task to its Wave/Project/Task via the roadmap.
    private func _loadHierarchy() async {
        guard let snapshot = try? await query.roadmap() else { return }
        var index: [String: SessionContext] = [:]
        for wave in snapshot.waves {
            for project in wave.projects.items {
                for task in project.tasks {
                    let context = SessionContext(
                        wave: wave.wave.name,
                        project: project.project.name,
                        identifier: task.task.identifier,
                        taskName: task.task.name
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

    // The work hierarchy filtered to what has a session: session-bearing tasks
    // grouped by their Wave › Project.
    private struct SessionGroup: Identifiable {
        let wave: String
        let project: String
        let items: [SessionItem]
        var id: String { "\(wave)/\(project)" }
    }

    private func _groupedSessions() -> [SessionGroup] {
        var order: [String] = []
        var buckets: [String: (wave: String, project: String, items: [SessionItem])] = [:]
        for item in store.sessions {
            let context = hierarchy[item.work.id]
            let wave = context?.wave ?? "—"
            let project = context?.project ?? "Other"
            let key = "\(wave)/\(project)"
            if buckets[key] == nil {
                buckets[key] = (wave, project, [])
                order.append(key)
            }
            buckets[key]?.items.append(item)
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
                        ForEach(group.items) { item in
                            _sessionRow(item)
                        }
                    }
                }
            }
            .padding(Spacing.sm)
        }
    }

    private func _groupHeader(_ wave: String, project: String) -> some View {
        HStack(spacing: Spacing.xxs) {
            Text(wave)
                .font(Typography.caption(9).weight(.bold))
                .foregroundStyle(Color.loopflowBurgundy)
            Image(systemName: "chevron.right")
                .font(.system(size: 6, weight: .bold))
                .foregroundStyle(palette.textSecondary.opacity(0.5))
            Text(project)
                .font(Typography.caption(9).weight(.semibold))
                .foregroundStyle(palette.textSecondary)
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
                description: Text("Tasks with an open or queued session appear here, grouped by Wave and Project.")
            )
            .frame(maxWidth: .infinity)
            .padding(.top, Spacing.xl)
        }
    }

    private func _sectionTitle(_ title: String, count: Int) -> some View {
        HStack(spacing: Spacing.xxs) {
            Text(title.uppercased())
            Text("\(count)").opacity(0.65)
            Spacer()
        }
        .font(Typography.caption(8).weight(.bold))
        .foregroundStyle(palette.textSecondary)
        .padding(.horizontal, Spacing.xs)
        .padding(.top, Spacing.sm)
    }

    private func _sessionRow(_ item: SessionItem) -> some View {
        let pane = _pane(for: item.id)
        let isFocused = pane?.id == focusedPaneId
        let color = pane.map { multiplexer.color(for: $0.id).color }
        return Button {
            if item.surface != nil {
                _load(item)
            } else {
                store.beginPaneLoad(item.id)
                pendingSessionId = item.id
                store.prioritize(item.id)
            }
        } label: {
            HStack(alignment: .top, spacing: Spacing.sm) {
                Circle()
                    .fill(color ?? _stateColor(item.state))
                    .frame(width: 8, height: 8)
                    .padding(.top, 4)
                VStack(alignment: .leading, spacing: Spacing.xxs) {
                    let context = hierarchy[item.work.id]
                    Text(context?.taskName ?? item.interventionPrompt ?? "Session")
                        .font(Typography.body(11).weight(.semibold))
                        .foregroundStyle(palette.text)
                        .lineLimit(2)
                    HStack(spacing: Spacing.xs) {
                        if let identifier = context?.identifier {
                            Text(identifier)
                                .font(Typography.caption(9))
                                .foregroundStyle(palette.textSecondary)
                        }
                        if let step = item.step {
                            Text(step)
                                .font(Typography.caption(8).weight(.semibold))
                                .foregroundStyle(palette.textSecondary)
                                .padding(.horizontal, Spacing.xxs)
                                .padding(.vertical, 1)
                                .background(palette.textSecondary.opacity(0.14), in: Capsule())
                        }
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
                Text(isFocused ? "ACTIVE" : _status(item, pane: pane))
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
    }

    private func _pane(for sessionId: String) -> PaneState? {
        multiplexer.pane(forSessionId: sessionId)
    }

    private func _status(_ item: SessionItem, pane: PaneState?) -> String {
        if pane != nil { return "OPEN" }
        return switch item.state {
        case .pending: "QUEUED"
        case .opening: "OPENING…"
        case .live: "READY"
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
        if multiplexer.pane(forSessionId: item.id) == nil,
           pendingSessionId != item.id {
            store.beginPaneLoad(item.id)
        }
        multiplexer.load(sessionId: item.id)
    }

    private func _handle(_ shortcut: SessionsShortcut) {
        switch shortcut {
        case .splitRight:
            _ = multiplexer.split(multiplexer.focusedPaneId, axis: .vertical)
        case .splitDown:
            _ = multiplexer.split(multiplexer.focusedPaneId, axis: .horizontal)
        case .close:
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

private struct SessionPaneView: View {
    let pane: PaneState
    let isFocused: Bool
    let scope: SessionScope
    @ObservedObject var sessions: SessionsStore
    let store: MultiplexerStore
    @State private var bellRinging = false
    @State private var terminalTitle: String?

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
        .accessibilityElement(children: .contain)
        .accessibilityLabel("\(_title), \(isFocused ? "active" : "open")")
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
                    .accessibilityLabel("Needs attention")
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
                store.close(pane.id)
            } label: {
                Image(systemName: "xmark")
            }
            .disabled(!store.canClose)
            .help("Close pane (⌘W)")
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
    private var content: some View {
        switch pane.content {
        case .empty:
            ContentUnavailableView(
                "Select a session",
                systemImage: "terminal",
                description: Text("Choose a request from the Sessions list or open a new shell.")
            )
        case .session:
            if let item, let surface = item.surface {
                _terminal(item: item, surface: surface)
            } else {
                ContentUnavailableView(
                    "Session unavailable",
                    systemImage: "terminal",
                    description: Text("The Ask may have settled while this pane was open.")
                )
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

    @ViewBuilder
    private func _terminal(item: SessionItem, surface: AskSessionRecord) -> some View {
        let command = LaunchTargetLauncher.command(for: surface, localCwd: scope.repoPath)
        GhosttyTerminalView(
            workingDirectory: command.cwd,
            argv: command.argv,
            env: command.environment,
            sessionId: _terminalSurfaceId(
                askId: item.id,
                runId: surface.runId
            ),
            isFocused: isFocused,
            onSurfaceCreated: {
                sessions.recordPaneLive(item.id)
                Task { await sessions.confirmPresented(item.id) }
            },
            onFocus: { store.setFocusedPane(pane.id) }
        )
        .id(surface.runId)
    }

    private var _title: String {
        if let terminalTitle, !terminalTitle.isEmpty { return terminalTitle }
        return switch pane.content {
        case .empty: "Session"
        case .session: item?.label ?? "Session"
        case .shell: "Shell"
        }
    }

    private var _surfaceId: String {
        if let item, let surface = item.surface {
            return _terminalSurfaceId(
                askId: item.id,
                runId: surface.runId
            )
        }
        return "shell-pane-\(pane.id)"
    }
}

private func _terminalSurfaceId(askId: String, runId: String) -> String {
    "ask-\(askId)-\(runId)"
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
