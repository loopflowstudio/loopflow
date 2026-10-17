#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// Full Work and compact Task faces borrow the same retained panes and content stores.
struct WorkspacePanesView: View {
    enum Presentation {
        case layout
        /// Records must come from this Task's authoritative Work projection.
        case taskTile(sessions: [SessionRecord])
    }

    let workspace: SessionsWorkspace
    let isFocused: Bool
    let sessions: SessionsStore
    let model: WorkModel
    var presentation: Presentation = .layout
    let onFocus: (String, String?) -> Void

    var body: some View {
        let store = workspace.multiplexer
        Group {
            switch presentation {
            case .layout:
                if let layout = store.zoomedPaneId == nil ? store.visibleLayout : store.layout {
                    MultiplexerView(layout: layout,
                        focusedPaneId: isFocused ? store.focusedPaneId : "",
                        zoomedPaneId: store.zoomedPaneId, sessions: sessions,
                        workspace: workspace, model: model, projected: false, onFocus: onFocus)
                } else {
                    ContentUnavailableView("Panes collapsed", systemImage: "rectangle.compress.vertical",
                        description: Text("Open the Task's Session from the outline, or add a shell."))
                }
            case .taskTile(let records):
                switch TaskPaneProjection.resolve(in: store, sessions: records) {
                case .pane(let id):
                    if let pane = store.layout.pane(for: id) {
                        MultiplexerView(layout: .leaf(pane),
                            focusedPaneId: isFocused ? store.focusedPaneId : "",
                            zoomedPaneId: nil, sessions: sessions,
                            workspace: workspace, model: model, projected: true, onFocus: onFocus)
                    }
                case .missingPrimary:
                    Text("Primary Session is unavailable.")
                case .missingPane:
                    Text("Primary Session has no retained pane.")
                case .ambiguousPrimary:
                    Text("Primary Session is ambiguous.")
                }
            }
        }
        .onChange(of: store.zoomedPaneId) { _, _ in endWorkspaceAction() }
        .onChange(of: store.collapsedPaneIds) { _, _ in endWorkspaceAction() }
    }

    private func endWorkspaceAction() {
        Perf.endAfterCommit(Perf.retainedWorkspaceAction, id: "workspace")
    }
}

private struct MultiplexerView: View {
    @Environment(\.palette) private var palette
    let layout: LayoutNode
    let focusedPaneId: String
    let zoomedPaneId: String?
    let sessions: SessionsStore
    let workspace: SessionsWorkspace
    let model: WorkModel
    let projected: Bool
    let onFocus: (String, String?) -> Void

    var body: some View {
        Group {
            if let zoomedPaneId, let pane = layout.pane(for: zoomedPaneId) {
                SessionPaneView(
                    pane: pane,
                    isFocused: pane.id == focusedPaneId,
                    sessions: sessions,
                    workspace: workspace, model: model, projected: projected, onFocus: onFocus
                )
            } else {
                MultiplexerNodeView(
                    node: layout,
                    focusedPaneId: focusedPaneId,
                    sessions: sessions,
                    workspace: workspace, model: model, projected: projected, onFocus: onFocus
                )
            }
        }
        .padding(6)
        .background(palette.surfaceMuted)
        .environment(\.colorScheme, .dark)
        .accessibilityIdentifier("sessions-multiplexer")
    }
}

private struct MultiplexerNodeView: View {
    let node: LayoutNode
    let focusedPaneId: String
    let sessions: SessionsStore
    let workspace: SessionsWorkspace
    let model: WorkModel
    let projected: Bool
    let onFocus: (String, String?) -> Void

    private var store: MultiplexerStore { workspace.multiplexer }

    var body: some View { _content }

    private var _content: AnyView {
        switch node {
        case .leaf(let pane):
            return AnyView(
                SessionPaneView(
                    pane: pane,
                    isFocused: pane.id == focusedPaneId,
                    sessions: sessions,
                    workspace: workspace, model: model, projected: projected, onFocus: onFocus
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
                                update: { store.updateRatio(between: first.firstPane.id, and: second.firstPane.id, ratio: $0) }
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
                                update: { store.updateRatio(between: first.firstPane.id, and: second.firstPane.id, ratio: $0) }
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
            sessions: sessions,
            workspace: workspace, model: model, projected: projected, onFocus: onFocus
        )
    }
}

struct SplitDivider: View {
    let axis: SplitAxis
    let ratio: Double
    let extent: CGFloat
    let update: (Double) -> Void

    @State private var dragStart: Double?

    var body: some View {
        Rectangle()
            .fill(Color.clear)
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
                        update(start + Double(delta / extent))
                    }
                    .onEnded { _ in dragStart = nil }
            )
    }
}

struct SessionPaneView: View {
    @State private var shellError: String?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let pane: PaneState
    let isFocused: Bool
    let sessions: SessionsStore
    let workspace: SessionsWorkspace
    let model: WorkModel
    let projected: Bool
    let onFocus: (String, String?) -> Void

    private var store: MultiplexerStore { workspace.multiplexer }
    private var hover: PaneHover { workspace.hover }
    @State private var bellRinging = false
    @State private var terminalTitle: String?

    private var hovering: Bool { hover.paneId == pane.id }

    private var item: SessionItem? {
        guard case .session(let id, _, _) = pane.content else { return nil }
        return sessions.sessions.first { $0.id == id }
    }

    /// Program identity stays visible even when only one terminal is open.
    private var showsStrip: Bool {
        if case .empty = pane.content { return store.layout.allPanes.count > 1 }
        return true
    }

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
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        .environment(\.colorScheme, .dark)
        // Retained pane selection is not proof of native keyboard focus.
        .overlay {
            RoundedRectangle(cornerRadius: CornerRadius.md)
                .strokeBorder(isFocused ? LoopflowPalette.dark.selectionEdge : TerminalPalette.divider,
                              lineWidth: isFocused ? 1.5 : 1)
                .allowsHitTesting(false)
        }
        .shadow(color: .black.opacity(isFocused ? 0.22 : 0.08), radius: isFocused ? 4 : 1, y: isFocused ? 2 : 1)
        .animation(DesignAnimation.standard(reduceMotion), value: isFocused)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("\(_title), \(isFocused ? "active" : "open")")
        .accessibilityIdentifier(item.map { "session-pane-\($0.id)" } ?? "session-pane-empty")
        .alert("Could not open shell", isPresented: Binding(
            get: { shellError != nil }, set: { if !$0 { shellError = nil } }
        )) {
            Button("OK") { shellError = nil }
        } message: { Text(shellError ?? "") }
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
            if let tone = programTone {
                Circle().fill(tone.ink(in: .dark)).frame(width: 6, height: 6)
                    .accessibilityHidden(true)
            }
            Label(programHeading.name, systemImage: programHeading.symbol)
                .font(Typography.body(11)).fontWeight(.semibold)
                .foregroundStyle(TerminalPalette.foreground)
                .fixedSize()
                .accessibilityIdentifier("pane-program-\(pane.id)")
            Text(programHeading.context)
                .font(Typography.code(11)).foregroundStyle(TerminalPalette.dim)
                .lineLimit(1).truncationMode(.middle)
                .help(programHeading.detail)
                .accessibilityIdentifier("pane-context-\(pane.id)")
            if let status = _programStatus, status.summary != nil {
                ProgramStatusLabel(status: status)
                    .environment(\.palette, LoopflowPalette.dark)
            }
            if let error = _programStatusSurface?.observationError {
                Image(systemName: "exclamationmark.circle")
                    .help(error).accessibilityLabel(error)
            }
            Spacer(minLength: 8)
            if bellRinging {
                Image(systemName: "bell.fill")
                    .font(.system(size: 10))
                    .foregroundStyle(LoopflowPalette.dark.waiting)
                    .accessibilityLabel("Session ready")
            }
            if isFocused && hovering {
                HStack(spacing: 2) {
                    stripButton("rectangle.split.2x1", label: "Split right", id: "pane-split-right", help: "Split right (⌘D)") {
                        split(.vertical)
                    }
                    stripButton("rectangle.split.1x2", label: "Split down", id: "pane-split-down", help: "Split down (⌘⇧D)") {
                        split(.horizontal)
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
        .frame(height: 32)
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
        Button("Split right (⌘D)") { split(.vertical) }
        Button("Split down (⌘⇧D)") { split(.horizontal) }
        Button(store.zoomedPaneId == pane.id ? "Exit zoom (⌘⇧↩)" : "Zoom pane (⌘⇧↩)") { store.toggleZoom(pane.id) }
        Divider()
        Button("Close view (⌘W)") { closePane() }.disabled(!store.canClose)
    }

    private var closeHelp: String {
        if case .shell = pane.content { return "Close this shell (⌘W)" }
        return "Close this view; the Session keeps running (⌘W)"
    }

    private func split(_ axis: SplitAxis) {
        do { try workspace.split(pane.id, axis: axis) }
        catch { shellError = error.localizedDescription }
    }

    private func closePane() {
        if case .shell = pane.content {
            if let terminal = pane.terminal { sessions.surfaces.release(terminal) }
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

    private var programTone: WorkTone? {
        if let report = _programStatus?.summary { return report.state.workTone }
        return SessionProgramStatus(sessions: paneSessions.map(\.record), unavailable: nil).tone
    }

    private var paneSessions: [SessionItem] {
        if case .shell = pane.content {
            guard let terminal = pane.terminal else { return [] }
            return sessions.sessions.filter {
                sessions.localTerminal(for: $0.record) == terminal
            }
        }
        return item.map { [$0] } ?? []
    }

    private func focusPane() {
        onFocus(pane.id, paneSessions.count == 1 ? paneSessions.first?.id : nil)
    }

    @ViewBuilder
    private var content: some View {
        switch pane.content {
        case .empty:
            emptyWorkspace
        case .session:
            if let item {
                if let surface = item.surface, !projected || hasRetainedSurface {
                    _terminal(item: item, surface: surface)
                } else {
                    _sessionPlaceholder(item)
                        .environment(\.colorScheme, .dark)
                }
            } else {
                emptyWorkspace
            }
        case .shell:
            if projected && !hasRetainedSurface {
                Text("Shell surface is not retained here.")
            } else if let location = pane.content.location, let terminal = pane.terminal {
                let isLocal = location.machineId == workspace.localMachineId
                if !isLocal && store.shellCommands[pane.id] == nil {
                    Text("Reopen the Task and add a new remote shell. No local shell was opened.")
                } else {
                    GhosttyTerminalView(
                        workingDirectory: isLocal ? location.worktree : NSHomeDirectory(),
                        argv: store.shellCommands[pane.id] ?? [],
                        terminal: terminal,
                        surfacePool: sessions.surfaces,
                        isFocused: isFocused,
                        onFocus: focusPane
                    )
                    .id(pane.id)
                }
            } else {
                Text("Shell location is unavailable. Add a new shell in this Work.")
            }
        case .flowLog(let taskId, let location, let execution):
            FlowProcessLog(model: model, taskId: taskId, location: location, execution: execution,
                query: sessions.query)
                .id(pane.incarnation)
                .background(PaneFocusTarget(isFocused: isFocused))
                .simultaneousGesture(TapGesture().onEnded { focusPane() })
        case .files(let taskId, _, _):
            let task = model.task(id: taskId)?.task
            switch Result(catching: { try workspace.files(for: pane.content, task: task, query: sessions.query) }) {
            case .success(let files):
                TaskFilesView(store: files, prURL: task?.pr?.publication?.github?.url,
                    checkoutBase: task?.reference.workspace?.baseCommit)
                    .environment(\.colorScheme, .light)
                    .simultaneousGesture(TapGesture().onEnded { focusPane() })
            case .failure(let error):
                Text(error.localizedDescription).textSelection(.enabled)
            }
        }
    }

    @ViewBuilder
    private var emptyWorkspace: some View {
        if workspace.openingConversation {
            ProgressView("Opening conversation…")
                .foregroundStyle(TerminalPalette.foreground)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if workspace.key.kind == .task {
            ContentUnavailableView {
                Label("No Task conversation open", systemImage: "bubble.left")
                    .foregroundStyle(TerminalPalette.foreground)
            } description: {
                Text(workspace.conversationError == nil
                    ? "Open this Task from the sidebar to return to its conversation."
                    : "The conversation could not open. Retry the opening above.")
                    .foregroundStyle(TerminalPalette.dim)
            }
        } else {
            ContentUnavailableView {
                Label("No session open", systemImage: "terminal")
                    .foregroundStyle(TerminalPalette.foreground)
            } description: {
                Text("Open a conversation or a shell in this Work.")
                    .foregroundStyle(TerminalPalette.dim)
            } actions: {
                Button {
                    do { try workspace.newShell() }
                    catch { shellError = error.localizedDescription }
                } label: {
                    Label("New shell", systemImage: "terminal")
                }
                .buttonStyle(.bordered)
                .tint(TerminalPalette.foreground)
                .accessibilityIdentifier("sessions-empty-new-shell")
            }
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
            ProgressView("Opening conversation…")
                .foregroundStyle(TerminalPalette.foreground)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .ended(let output):
            VStack(alignment: .leading, spacing: Spacing.md) {
                Text("Session terminal ended").foregroundStyle(TerminalPalette.foreground)
                ScrollView {
                    Text(output).font(.system(.body, design: .monospaced))
                        .foregroundStyle(TerminalPalette.foreground).textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                openButton(item, retrying: true)
            }.padding()
        case .failed(let message):
            ContentUnavailableView {
                Label("Session failed to open", systemImage: "exclamationmark.triangle")
                    .foregroundStyle(TerminalPalette.foreground)
            } description: {
                Text(message)
                    .foregroundStyle(TerminalPalette.dim)
            } actions: {
                openButton(item, retrying: true)
            }
        case .pending where workspace.openingConversation:
            ProgressView("Opening conversation…")
                .foregroundStyle(TerminalPalette.foreground)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .pending, .live:
            ContentUnavailableView {
                Label("Not running here", systemImage: "terminal")
                    .foregroundStyle(TerminalPalette.foreground)
            } description: {
                Text("This Session has no live terminal in this pane yet.")
                    .foregroundStyle(TerminalPalette.dim)
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
            .tint(TerminalPalette.foreground)
            .foregroundStyle(TerminalPalette.foreground)
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
                .foregroundStyle(TerminalPalette.foreground)
        } description: {
            Text(
                """
                \(item.record.title) is attached to a provider client outside \
                this window — Warp, another Loopflow window, or an SSH session. \
                It keeps running there until you move it.
                """
            )
            .foregroundStyle(TerminalPalette.dim)
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
                    .tint(TerminalPalette.foreground)
                    .foregroundStyle(TerminalPalette.foreground)
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
        if let location = pane.content.location, surface.workspace?.identity == location,
           let terminal = pane.terminal {
            GhosttyTerminalView(
                workingDirectory: sessions.presentationDirectory(for: surface, checkout: location.worktree),
                argv: surface.openArgv,
                terminal: terminal,
                surfacePool: sessions.surfaces,
                isFocused: isFocused,
                onSurfaceCreated: {
                    Perf.agentInput("surface_created", session: item.id)
                    sessions.recordPaneLive(item.id)
                },
                onSurfaceFailed: { reason in sessions.recordPaneFailure(item.id, reason: reason) },
                onFocus: focusPane
            )
            .id(surface.id)
        } else {
            Text("Session location unavailable")
        }
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
        case .flowLog: "Flow processes"
        case .files: "Files"
        }
    }

    private var programHeading: PaneProgramHeading {
        let fallback: String = switch pane.content {
        case .empty: "Workspace"
        case .session: "Session"
        case .shell: "Shell"
        case .flowLog: "Flow processes"
        case .files: "Files"
        }
        return PaneProgramHeading(provider: paneSessions.count == 1 ? paneSessions.first?.record.provider : nil,
            title: paneSessions.count == 1 ? paneSessions.first?.record.title ?? _title : _title,
            directory: pane.content.location?.worktree, fallback: fallback)
    }

    private var _programStatusSurface: ProgramStatusSurface? {
        _terminalIdentity.flatMap { sessions.surfaces.programStatus(for: $0) }
    }

    private var _programStatus: ProgramStatusRecords? {
        if case .session = pane.content { return item?.record.programStatus }
        if let id = _programStatusSurface?.sessionId {
            return sessions.sessions.first { $0.id == id }?.record.programStatus
        }
        return _programStatusSurface?.snapshot
    }

    private var hasRetainedSurface: Bool {
        pane.terminal.flatMap { sessions.surfaces.surfaceIncarnation(for: $0) } != nil
    }

    private var _terminalIdentity: TerminalIdentity? {
        switch pane.content {
        case .session:
            item?.surface.flatMap { record in
                guard record.workspace?.identity == pane.content.location else { return nil }
                return pane.terminal
            }
        case .shell:
            pane.terminal
        case .empty, .flowLog, .files:
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
private struct PaneFocusTarget: NSViewRepresentable {
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

#endif
