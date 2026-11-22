import AppKit
import Foundation
import Loopflow
import SwiftUI

/// Render a Work’s retained pane layout and content stores.
struct WorkspacePanesView: View {
    @Environment(\.palette) private var palette
    let workspace: SessionsWorkspace
    let isFocused: Bool
    let sessions: SessionsStore
    let model: WorkModel
    let onFocus: (String, String?) -> Void
    var face: PaneState? = nil

    var body: some View {
        let store = workspace.multiplexer
        Group {
            if let face { panes(.leaf(face)) }
            else if let zoom = store.zoomedPaneId, let pane = store.layout.pane(for: zoom) {
                panes(.leaf(pane))
            } else if let layout = store.visibleLayout {
                panes(layout)
            } else {
                EmptyState(title: "Panes collapsed", sentence: "Your panes are kept. Show a pane to continue.") {
                    if !workspace.showsConversation(held: Set(sessions.sessions.map(\.id))) {
                        ConversationOpenButton(workspace: workspace, model: model, sessions: sessions)
                    }
                    Button("Show pane") {
                        let pane = store.focusedPane
                        onFocus(pane.id, pane.sessionId)
                    }
                }
            }
        }
        .environment(workspace)
        .onChange(of: store.zoomedPaneId) { _, _ in endWorkspaceAction() }
        .onChange(of: store.collapsedPaneIds) { _, _ in endWorkspaceAction() }
    }

    private func panes(_ layout: LayoutNode) -> some View {
        MultiplexerNodeView(node: layout,
            focusedPaneId: isFocused ? workspace.multiplexer.focusedPaneId : "",
            sessions: sessions, workspace: workspace, model: model,
            onFocus: onFocus)
            .padding(PaneGeometry.spacing)
            .background(palette.surfaceMuted)
            .accessibilityIdentifier("sessions-multiplexer")
    }

    private func endWorkspaceAction() {
        Perf.stateObserved(Perf.retainedWorkspaceAction, id: "workspace", owner: workspace.performanceOwner)
    }
}

/// The one action of a Work whose conversation is not open here: Retry after a
/// failure, else Open. The model routes it by the Work's kind.
struct ConversationOpenButton: View {
    let workspace: SessionsWorkspace
    let model: WorkModel
    let sessions: SessionsStore

    var body: some View {
        let failed = workspace.conversationError != nil
        Button(failed ? "Retry" : "Open conversation") {
            Task { await model.openConversation(in: workspace, store: sessions) }
        }
        .help(failed ? "Try opening this conversation again" : "Open this conversation here, starting it if it has none")
        .accessibilityIdentifier("open-conversation")
    }
}

/// The Session slot before any conversation is open here, in a pane or a
/// Dashboard tile. Every Work opens its own; this says why it has not and
/// offers the one next action. An unstarted Task has no checkout and so no
/// conversation: it says what Start will run.
struct ConversationPlaceholder: View {
    @Environment(\.theme) private var theme
    let workspace: SessionsWorkspace
    let model: WorkModel
    let sessions: SessionsStore

    var body: some View {
        if model.conversationIsOpening(workspace) {
            SessionOpeningView(screen: nil)
                .foregroundStyle(theme.palette.text)
        } else if workspace.key.kind == .task, let task = model.task(id: workspace.key.id)?.task, task.awaitsStart {
            TaskStartOffer(model: model, task: task)
        } else {
            let error = workspace.conversationError
            EmptyState(title: error == nil ? "Conversation not open" : "Conversation failed to open",
                       sentence: error == nil ? note : nil, machine: error) {
                ConversationOpenButton(workspace: workspace, model: model, sessions: sessions)
            }
        }
    }

    private var note: String? {
        if workspace.conversationClosed { return "Closed here. Select it again to reopen the conversation." }
        return model.showsSavedWork ? WorkCache.savedReason : nil
    }

    /// Whether a pane on screen draws this workspace's failed opening as its
    /// own state, so the line above the panes does not say it a second time.
    @MainActor
    static func namesFailure(in workspace: SessionsWorkspace, model: WorkModel, sessions: SessionsStore) -> Bool {
        if model.conversationIsOpening(workspace) { return false }
        if workspace.key.kind == .task, model.task(id: workspace.key.id)?.task.awaitsStart == true { return false }
        let store = workspace.multiplexer
        let panes = store.zoomedPaneId.flatMap { store.layout.pane(for: $0) }.map { [$0] }
            ?? store.visibleLayout?.allPanes ?? []
        return panes.contains { pane in
            guard case .session(let id, _, _) = pane.content else { return false }
            return !workspace.canOpenTerminals || !sessions.sessions.contains { $0.id == id }
        }
    }
}

/// Opening keeps available history readable until the terminal takes its place.
struct SessionOpeningView: View {
    let screen: String?
    @Environment(\.palette) private var palette

    var body: some View {
        ZStack(alignment: .topLeading) {
            Color.clear
            if let screen {
                ScrollView {
                    Text(screen).font(Typography.mono)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            ProgressView(ServerConnectionReading.openingTitle)
                .controlSize(.small).padding(Spacing.sm).background(palette.background)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// Inputs compared without `onFocus`: a closure never compares equal, so it
/// would re-run every pane on each parent render. It captures only references.
extension WorkspacePanesView: Equatable {
    nonisolated static func == (lhs: Self, rhs: Self) -> Bool {
        MainActor.assumeIsolated {
            lhs.workspace === rhs.workspace && lhs.isFocused == rhs.isFocused && lhs.sessions === rhs.sessions
                && lhs.model === rhs.model && lhs.face == rhs.face
        }
    }
}

enum PaneGeometry {
    // Keep pane outlines inside the native window's larger lower corners.
    static let windowInset: CGFloat = 10
    static let spacing: CGFloat = 6
    static let cornerRadius = CornerRadius.pane
    /// Pane chrome and content keep clear of the corner arc on every edge.
    static let inset = cornerRadius
    // Nested curves share a center at the workspace's lower corners.
    static let workspaceCornerRadius = cornerRadius + spacing
    /// Square across the titlebar; rounded where the workspace nests in the window.
    static let workspaceShape = UnevenRoundedRectangle(
        topLeadingRadius: 0, bottomLeadingRadius: workspaceCornerRadius,
        bottomTrailingRadius: workspaceCornerRadius, topTrailingRadius: 0)
}

private struct MultiplexerNodeView: View {
    let node: LayoutNode
    let focusedPaneId: String
    let sessions: SessionsStore
    let workspace: SessionsWorkspace
    let model: WorkModel
    let onFocus: (String, String?) -> Void

    private var store: MultiplexerStore { workspace.multiplexer }

    // Structural, not AnyView: SwiftUI diffs panes and splits by their types.
    @ViewBuilder var body: some View {
        switch node {
        case .leaf(let pane):
            SessionPaneView(
                pane: pane,
                isFocused: pane.id == focusedPaneId,
                sessions: sessions,
                workspace: workspace, model: model, onFocus: onFocus
            )
            .id(pane.id)
        case .split(let axis, let first, let second, let ratio):
            GeometryReader { geometry in
                let vertical = axis == .vertical
                let extent = vertical ? geometry.size.width : geometry.size.height
                let firstSize = first.minimumPaneSize
                let secondSize = second.minimumPaneSize
                let sizing = SplitSizing(extent: extent,
                    firstMinimum: vertical ? firstSize.width : firstSize.height,
                    secondMinimum: vertical ? secondSize.width : secondSize.height)
                let displayedRatio = sizing.constrain(ratio)
                let firstExtent = max(0, extent * displayedRatio - sizing.divider / 2)
                let secondExtent = max(0, extent - firstExtent - sizing.divider)
                let stack = vertical ? AnyLayout(HStackLayout(spacing: 0)) : AnyLayout(VStackLayout(spacing: 0))
                stack {
                    _child(first)
                        .frame(width: vertical ? firstExtent : nil, height: vertical ? nil : firstExtent)
                        .clipped()
                    SplitDivider(axis: axis, ratio: displayedRatio, sizing: sizing,
                        update: { store.updateRatio(between: first.firstPane.id, and: second.firstPane.id, ratio: $0) })
                    _child(second)
                        .frame(width: vertical ? secondExtent : nil, height: vertical ? nil : secondExtent)
                        .clipped()
                }
            }
        }
    }

    private func _child(_ child: LayoutNode) -> MultiplexerNodeView {
        MultiplexerNodeView(
            node: child,
            focusedPaneId: focusedPaneId,
            sessions: sessions,
            workspace: workspace, model: model, onFocus: onFocus
        )
    }
}

/// Minimums follow the visible split tree; short windows shrink them together.
extension LayoutNode {
    var minimumPaneSize: CGSize {
        switch self {
        case .leaf: return CGSize(width: 240, height: 140)
        case .split(let axis, let first, let second, _):
            let a = first.minimumPaneSize, b = second.minimumPaneSize
            if axis == .vertical {
                return CGSize(width: a.width + b.width + PaneGeometry.spacing, height: max(a.height, b.height))
            } else {
                return CGSize(width: max(a.width, b.width), height: a.height + b.height + PaneGeometry.spacing)
            }
        }
    }
}

struct SplitSizing {
    let extent: CGFloat
    let firstMinimum: CGFloat
    let secondMinimum: CGFloat
    var divider: CGFloat { min(PaneGeometry.spacing, max(0, extent)) }

    func constrain(_ ratio: Double) -> Double {
        guard extent > 0 else { return 0.5 }
        let available = max(0, extent - divider)
        let scale = min(1, available / max(1, firstMinimum + secondMinimum))
        let lower = (firstMinimum * scale + divider / 2) / extent
        let upper = 1 - (secondMinimum * scale + divider / 2) / extent
        return min(max(ratio, lower), max(lower, upper))
    }

    func snapped(_ ratio: Double) -> Double {
        let bounded = constrain(ratio)
        for target in [1 - LayoutNode.goldenRatio, LayoutNode.goldenRatio] {
            if abs(bounded - target) * extent <= 10, abs(constrain(target) - target) < 0.000001 {
                return target
            }
        }
        return bounded
    }
}

struct SplitDivider: View {
    let axis: SplitAxis
    let ratio: Double
    let sizing: SplitSizing
    let update: (Double) -> Void
    var commit: ((Double) -> Void)? = nil
    @Environment(\.palette) private var palette
    @State private var dragStart: Double?
    @State private var hovering = false

    var body: some View {
        Rectangle()
            .fill(hovering || dragStart != nil ? palette.borderActive : .clear)
            .frame(width: axis == .vertical ? sizing.divider : nil,
                   height: axis == .horizontal ? sizing.divider : nil)
            .contentShape(Rectangle())
            .onHover { inside in
                guard hovering != inside else { return }
                hovering = inside
                if inside { (axis == .vertical ? NSCursor.resizeLeftRight : NSCursor.resizeUpDown).push() }
                else { NSCursor.pop() }
            }
            .onDisappear { if hovering { NSCursor.pop(); hovering = false } }
            .gesture(
                DragGesture(minimumDistance: 0, coordinateSpace: .global)
                    .onChanged { value in
                        let start = dragStart ?? ratio
                        if dragStart == nil { dragStart = ratio }
                        let delta = axis == .vertical ? value.translation.width : value.translation.height
                        guard sizing.extent > 0 else { return }
                        update(sizing.snapped(start + Double(delta / sizing.extent)))
                    }
                    .onEnded { value in
                        let delta = axis == .vertical ? value.translation.width : value.translation.height
                        if sizing.extent > 0 { commit?(sizing.snapped((dragStart ?? ratio) + Double(delta / sizing.extent))) }
                        dragStart = nil
                    }
            )
            .accessibilityLabel(axis == .vertical ? "Resize panes horizontally" : "Resize panes vertically")
            .accessibilityValue("\(Int(ratio * 100)) percent")
            .accessibilityAdjustableAction { direction in
                let value = sizing.constrain(ratio + (direction == .increment ? 0.02 : -0.02))
                update(value)
                commit?(value)
            }
    }
}

/// How this Session's conversation relates to the one before it, on its own line above the terminal.
struct ProviderTransitionRow: View {
    @Environment(\.theme) private var theme
    let transition: ProviderTransition
    var body: some View {
        Text(transition.explanation)
            .font(Typography.meta)
            .foregroundStyle(theme.palette.textSecondary)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, PaneGeometry.inset)
            .padding(.vertical, Spacing.xs)
            .help("Observed at " + Date(timeIntervalSince1970: TimeInterval(transition.observedAt)).formatted())
    }
}

struct SessionPaneView: View {
    @Environment(\.theme) private var theme
    @State private var changingSession = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let pane: PaneState
    let isFocused: Bool
    let sessions: SessionsStore
    let workspace: SessionsWorkspace
    let model: WorkModel
    let onFocus: (String, String?) -> Void

    private var store: MultiplexerStore { workspace.multiplexer }
    private var hover: HoverFlag { workspace.stripHover(pane.id) }
    @State private var bellRinging = false
    @State private var terminalTitle: String?
    @State private var copyConfirmation: String?

    private var item: SessionItem? {
        guard case .session(let id?, _, _) = pane.content else { return nil }
        return sessions.sessions.first { $0.id == id }
    }

    private var showsStrip: Bool { item != nil || (store.visibleLayout?.allPanes.count ?? 0) > 1 }

    var body: some View {
        VStack(spacing: 0) {
            if showsStrip {
                strip
                Rectangle().fill(theme.palette.border).frame(height: 1)
            }
            if let transition = item?.record.transition {
                ProviderTransitionRow(transition: transition)
            }
            content
                // Reserve the rounded edge before Ghostty measures its surface.
                .padding(pane.terminal == nil ? 0 : PaneGeometry.inset)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme.terminal.background.color)
        .clipShape(RoundedRectangle(cornerRadius: PaneGeometry.cornerRadius))
        // Retained pane selection is not proof of native keyboard focus. Only the
        // frame animates: an animated pane would animate its terminal's layout too.
        .overlay {
            RoundedRectangle(cornerRadius: PaneGeometry.cornerRadius)
                .strokeBorder(isFocused ? theme.palette.borderStrong : theme.palette.border, lineWidth: 1)
                .animation(DesignAnimation.standard(reduceMotion), value: isFocused)
                .allowsHitTesting(false)
        }
        .background {
            RoundedRectangle(cornerRadius: PaneGeometry.cornerRadius)
                .fill(theme.terminal.background.color)
                .shadow(color: theme.palette.shadow.opacity(isFocused ? 0.08 : 0), radius: 2, y: 1)
                .animation(DesignAnimation.standard(reduceMotion), value: isFocused)
        }
        .renderFlash()
        .accessibilityElement(children: .contain)
        .accessibilityLabel("\(_title), \(isFocused ? "active" : "open")")
        .accessibilityIdentifier(item.map { "session-pane-\($0.id)" } ?? "session-pane-empty")
        .modifier(CreationPresentation(recovery: .shared, resource: .session(item?.id ?? ""), visible: item != nil))
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
        .onReceive(NotificationCenter.default.publisher(for: .ghosttyCopyConfirmation)) { notification in
            guard let terminal = _terminalIdentity,
                  sessions.surfaces.ownsCopyConfirmation(notification, terminal: terminal) else { return }
            copyConfirmation = notification.userInfo?["message"] as? String
        }
        .onChange(of: isFocused) { _, focused in
            if focused { bellRinging = false }
        }
        .onChange(of: pane.content.selectedShell?.incarnation ?? pane.incarnation) { _, _ in
            bellRinging = false
            terminalTitle = nil
            copyConfirmation = nil
        }
    }

    /// Program identity and a Close control on hover; zoom stays in the context menu.
    private var strip: some View {
        HStack(spacing: Spacing.sm) {
            Menu {
                paneContentChoices
            } label: {
                Text(programHeading.name)
                    .font(Typography.textStrong)
                    .foregroundStyle(theme.palette.text)
            }
            .menuStyle(.borderlessButton).fixedSize()
            .disabled(changingSession)
            .help("Choose this Session's agent or open a companion pane\n\(programHeading.detail)")
            .accessibilityIdentifier("pane-program-\(pane.id)")
            if let item {
                stripButton("arrow.clockwise", label: "Refresh session", id: "pane-refresh-session",
                    help: "Reconnect this pane to the existing conversation.",
                    disabled: changingSession) {
                    workspace.visit()
                    changingSession = true
                    Perf.userAction(.reconnect, scenario: "refresh_to_typing", id: item.id,
                                    repository: sessions.repoPath, liveSessions: sessions.sessions.count,
                                    owner: workspace.performanceOwner) {
                        Perf.acknowledged(.reconnect, id: item.id, owner: workspace.performanceOwner)
                        Task { @MainActor in
                            defer { changingSession = false }
                            await sessions.refresh(item.id)
                            Perf.reconnected(id: item.id, owner: workspace.performanceOwner)
                        }
                    }
                }
            }
            if let session = primarySession {
                stripButton("plus.bubble", label: "Reset session", id: "pane-reset-session",
                    help: "Start fresh with current Work context. Keeps history; discards unsent input.",
                    disabled: changingSession) { changeSession(session, agent: nil) }
            }
            if case .shell = pane.content { shellTabs }
            if case .browser = pane.content { browserTabs }
            if workspace.key.kind != .task || pane.content.location != workspace.location {
                Text(programHeading.context)
                    .font(Typography.mono).foregroundStyle(theme.palette.textSecondary)
                    .lineLimit(1).truncationMode(.middle)
                    .help(programHeading.detail)
                    .accessibilityIdentifier("pane-context-\(pane.id)")
            }
            if let status = _programStatus, let mark = StatusMark(status: status) {
                mark.frame(maxWidth: 340, alignment: .leading)
            } else if let tone = programTone {
                StatusMark(nil, tone: tone)
            }
            if let error = _programStatusSurface?.observationError {
                Image(systemName: "exclamationmark.circle")
                    .help(error).accessibilityLabel(error)
            }
            Spacer(minLength: 8)
            if let copyConfirmation {
                Text(copyConfirmation)
                    .font(Typography.meta)
                    .foregroundStyle(theme.palette.text)
                    .lineLimit(1)
                    .allowsHitTesting(false)
                    .accessibilityIdentifier("terminal-copy-confirmation")
            }
            if bellRinging {
                Image(systemName: "bell.fill")
                    .font(Typography.meta)
                    .foregroundStyle(theme.palette.available)
                    .accessibilityLabel("Session ready")
            }
            // Every pane zooms and hides on hover; the breadcrumb zooms out and the
            // view header's Show… brings a hidden pane back.
            HoverRevealed(flag: hover) {
                HStack(spacing: Spacing.xxs) {
                    let zoomed = store.zoomedPaneId == pane.id
                    stripButton(zoomed ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right",
                                label: zoomed ? "Zoom out" : "Zoom pane", id: "pane-zoom",
                                help: zoomed ? "Return to the layout (⌘⇧↩)" : "Fill the view with this pane (⌘⇧↩)",
                                disabled: !store.canClose) {
                        workspace.toggleFocus(pane.id)
                    }
                    if isFocused, pane.content.selectedTabID != nil {
                        stripButton("xmark", label: "Close tab", id: "pane-close", help: closeHelp, disabled: !store.canClose) {
                            closePane()
                        }
                    }
                    stripButton("eye.slash", label: "Hide pane", id: "pane-hide",
                                help: "Hide this pane; Show… in the header brings it back", disabled: !store.canClose) {
                        workspace.setCollapsed(paneId: pane.id, collapsed: true)
                    }
                }
                .accessibilityIdentifier("pane-actions")
            }
        }
        .padding(.horizontal, PaneGeometry.inset)
        .frame(height: 28)
        .contentShape(Rectangle())
        .background(PaneHoverRegion(identifier: "pane-hover-\(pane.id)") { [hover] inside in
            if hover.isHovered != inside { hover.isHovered = inside }
        })
        .renderFlash()
        .onTapGesture { focusPane() }
        .contextMenu { paneActions }
        .accessibilityIdentifier("pane-strip-\(pane.id)")
    }

    @ViewBuilder
    private var paneContentChoices: some View {
        if let session = primarySession {
            Section("Session agent") {
                ForEach(["claude", "codex", "opencode", "pi", "agy"], id: \.self) { agent in
                    Button {
                        changeSession(session, agent: agent)
                    } label: {
                        Label(agent == "opencode" ? "OpenCode" : agent.capitalized,
                              systemImage: session.presentationProvider == agent ? "checkmark" : "bubble.left")
                    }
                    .disabled(session.presentationProvider == agent)
                    .help("Continue this Session with \(agent). Replaces the current process; unsent input is lost.")
                }
            }
            Divider()
        }
        if let taskId = paneTaskId {
            // The Task's own pane already has these as its tabs.
            if pane.content.kind != .browser {
                ForEach(TaskFilesStore.Page.allCases, id: \.self) { page in
                    Button(page.rawValue, systemImage: page.symbol) {
                        guard let browser = workspace.showBrowser(.files, from: pane) else { return }
                        do {
                            let files = try workspace.files(for: browser.content, task: model.task(id: taskId)?.task, query: sessions.query)
                            files.page = page
                        } catch { paneFailed(error) }
                    }
                }
            }
            Button("Task & workflow", systemImage: "list.bullet") {
                workspace.showBrowser(.task, from: pane)
            }
            Button("lf process", systemImage: "list.bullet.rectangle") {
                workspace.showBrowser(.graphLog, from: pane)
            }
        }
        if workspace.key.kind == .task {
            Button("Shell", systemImage: "terminal") {
                if let shell = store.shellPane, shell.content.selectedShell != nil { store.setFocusedPane(shell.id) }
                else { do { try workspace.newShell() } catch { paneFailed(error) } }
            }
        }
    }

    private var primarySession: SessionItem? {
        guard paneSessions.count == 1, let session = paneSessions.first,
              session.record.taskPrimary || session.record.primaryScope != nil else { return nil }
        return session
    }

    private var paneTaskId: String? {
        switch pane.content {
        case .browser(let task, _, _, _, _): return task
        default:
            if let work = primarySession?.record.work, work.kind == .task { return work.id }
            return workspace.key.kind == .task ? workspace.key.id : nil
        }
    }

    private func changeSession(_ session: SessionItem, agent: String?) {
        workspace.visit()
        changingSession = true
        let notices = model.navigation.notices
        Task { @MainActor in
            defer { changingSession = false }
            do {
                let record: SessionRecord
                if let agent {
                    record = try await sessions.switchProvider(session.id, to: agent)
                } else {
                    record = try await sessions.reset(session.id)
                }
                guard store.layout.pane(for: pane.id)?.incarnation == pane.incarnation else { return }
                if case .shell = pane.content {
                    store.reveal(sessionId: record.id, location: record.workspace?.identity)
                } else {
                    store.replace(pane.id, with: .session(id: record.id, location: record.workspace?.identity,
                        execution: pane.executionLocation))
                }
                workspace.presentation.selectedSessionId = record.id
                if model.workspaceKey(for: model.selection, repository: workspace.key.repository) == workspace.key {
                    model.navigation.selectedSessionId = record.id
                }
            } catch { paneFailed(error, in: notices) }
        }
    }

    /// The asking window's Notice; a late reply keeps the window it started in.
    private func paneFailed(_ error: Error, in notices: NoticeCenter? = nil) {
        (notices ?? model.navigation.notices).post(Notice("Could not change pane: \(error.localizedDescription)"))
    }

    @ViewBuilder
    private var paneActions: some View {
        Button(store.zoomedPaneId == pane.id ? "Exit zoom (⌘⇧↩)" : "Zoom pane (⌘⇧↩)") { store.toggleZoom(pane.id) }
        Divider()
        Button("Close view (⌘W)") { closePane() }.disabled(!store.canClose)
    }

    private var closeHelp: String {
        if case .shell = pane.content { return "Close this shell (⌘W)" }
        return "Close this tab (⌘W)"
    }

    private func closePane() { workspace.closePane(pane.id) }

    /// One strip glyph with a hit target larger than its symbol.
    private func stripButton(
        _ systemImage: String, label: String, id: String, help: String,
        disabled: Bool = false, action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(Typography.meta)
                .foregroundStyle(theme.palette.textSecondary)
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
        SessionProgramStatus(sessions: paneSessions.map(\.record), unavailable: nil).tone
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

    /// Types into this workspace's conversation and focuses it; you send.
    /// The result says whether the text reached the input.
    private func conversationInput() -> ((String) -> Bool)? {
        guard let session = store.layout.pane(of: .session), let terminal = session.terminal,
              sessions.surfaces.surfaceIncarnation(for: terminal) != nil else { return nil }
        return { text in
            do {
                guard let surface = sessions.surfaces.surfaceIncarnation(for: terminal) else {
                    throw RegistryQueryError("The conversation has no terminal surface; nothing was typed.")
                }
                try sessions.surfaces.insertText(text, terminal: terminal, surface: surface)
                store.setFocusedPane(session.id)
                sessions.surfaces.focus(terminal)
                return true
            } catch {
                paneFailed(error)
                return false
            }
        }
    }

    @ViewBuilder private var shellTabs: some View {
        if case .shell(let tabs, let selected) = pane.content {
            ScrollView(.horizontal) {
                HStack(spacing: Spacing.xs) {
                    ForEach(Array(tabs.enumerated()), id: \.element.id) { index, tab in
                        Button("\(index + 1)") { store.selectShell(tab.id) }
                            .accessibilityLabel("Shell \(index + 1)")
                            .padding(.horizontal, Spacing.xs).padding(.vertical, Spacing.xxs)
                            .background(tab.id == selected ? theme.palette.surfaceElement : .clear,
                                        in: RoundedRectangle(cornerRadius: CornerRadius.control))
                    }
                    Button { do { try workspace.newShell() } catch { paneFailed(error) } }
                        label: { Image(systemName: "plus") }
                        .accessibilityLabel("New shell tab")
                }.buttonStyle(.plain).font(Typography.mono)
            }.fixedSize(horizontal: false, vertical: true)
        }
    }

    /// One row names everything the pane can show: the Task's Brief, Diff and
    /// Files, then whatever else is open beside them.
    @ViewBuilder private var browserTabs: some View {
        if case .browser(let taskId, let tabs, let selected, _, _) = pane.content {
            let task = model.task(id: taskId)?.task
            let files = try? workspace.files(for: pane.content, task: task, query: sessions.query)
            ScrollView(.horizontal) {
                HStack(spacing: Spacing.xs) {
                    if let files {
                        ForEach(task == nil ? [.changes, .files] : TaskFilesStore.Page.allCases, id: \.self) { page in
                            browserTab(page.rawValue,
                                       selected: selected == BrowserTab.files.id && files.shownPage(hasTask: task != nil) == page,
                                       id: "page-\(page.rawValue.lowercased())") {
                                workspace.showBrowser(.files, from: pane)
                                files.page = page
                                files.noteInteraction()
                            }
                        }
                    }
                    ForEach(tabs.filter { $0 != .files || files == nil }) { tab in
                        browserTab(tab.title, selected: tab.id == selected, id: tab.id) {
                            workspace.showBrowser(tab, from: pane)
                        }
                    }
                }.buttonStyle(.plain).font(Typography.meta)
            }.fixedSize(horizontal: false, vertical: true)
            .help("Brief ⌥⌘1 · Diff ⌥⌘2 · Files ⌥⌘3")
        }
    }

    private func browserTab(_ title: String, selected: Bool, id: String, action: @escaping () -> Void) -> some View {
        Button(title, action: action)
            .lineLimit(1)
            .fontWeight(selected ? .semibold : .regular)
            .foregroundStyle(selected ? theme.palette.text : theme.palette.textSecondary)
            .padding(.horizontal, Spacing.sm).padding(.vertical, Spacing.xxs)
            .background(selected ? theme.palette.surfaceElement : .clear, in: RoundedRectangle(cornerRadius: CornerRadius.control))
            .accessibilityIdentifier("browser-tab-\(id)")
            .accessibilityAddTraits(selected ? .isSelected : [])
    }

    @ViewBuilder
    private var content: some View {
        switch pane.content {
        case .session where !workspace.canOpenTerminals:
            ConversationPlaceholder(workspace: workspace, model: model, sessions: sessions)
        case .shell where !workspace.canOpenTerminals && pane.content.selectedShell != nil:
            Button("Open shell") {
                focusPane()
                workspace.visit()
            }
        case .session:
            if let item {
                if let surface = item.surface {
                    _terminal(item: item, surface: surface)
                } else {
                    _sessionPlaceholder(item)
                }
            } else {
                ConversationPlaceholder(workspace: workspace, model: model, sessions: sessions)
            }
        case .shell:
            VStack(spacing: 0) {
                if !showsStrip { shellTabs.padding(PaneGeometry.inset) }
                if let tab = pane.content.selectedShell,
                          tab.location != nil, let terminal = tab.terminal {
                    if workspace.disconnectedShells.contains(tab.id) {
                        VStack {
                            if let screen = workspace.shellScreens[tab.id] {
                                ScrollView { Text(screen).font(Typography.mono).textSelection(.enabled) }
                            }
                            Text("Shell attachment ended. Reconnect checks the same execution; New Shell starts fresh.")
                            Button("Reconnect") { workspace.reconnectShell(tab) }
                            Button("New Shell") { do { try workspace.newShell() } catch { paneFailed(error) } }
                        }
                    } else if let command = try? workspace.shellAttachment(tab) {
                        GhosttyTerminalView(
                repository: sessions.repoPath,
                            workingDirectory: NSHomeDirectory(),
                            argv: command,
                            terminal: terminal,
                            surfacePool: sessions.surfaces,
                            isFocused: isFocused,
                            onFocus: focusPane
                        )
                        .id(tab.id)
                    } else {
                        Text("Retained shell is not attached. Add a new shell to start another process.")
                    }
                } else {
                    Text("Shell location is unavailable. Add a new shell in this Work.")
                }
            }
        case .browser(let taskId, _, _, let location, let execution):
            VStack(spacing: 0) {
                if !showsStrip { browserTabs.padding(PaneGeometry.inset) }
                switch pane.content.selectedBrowserTab {
                case .graphLog:
                    GraphProcessLog(model: model, taskId: taskId, location: location, execution: execution,
                        query: sessions.query, openFiles: { workspace.showBrowser(.files, from: pane) })
                        .id(pane.incarnation)
                        .background(PaneFocusTarget(isFocused: isFocused))
                        .simultaneousGesture(TapGesture().onEnded { focusPane() })
                case .files, .changes:
                    let task = model.task(id: taskId)?.task
                    switch Result(catching: { try workspace.files(for: pane.content, task: task, query: sessions.query) }) {
                    case .success(let files):
                        TaskFilesView(store: files, checkoutBase: task?.reference.workspace?.baseCommit, task: task,
                            graph: task.flatMap { model.taskWork[$0.id].value?.graph }, isFocused: isFocused, addToConversation: conversationInput())
                            .task(id: pane.content.selectedBrowserTab?.id) {
                                // A tab names its own comparison, never one Graph step's.
                                files.showStep(nil)
                                if case .changes(let comparison) = pane.content.selectedBrowserTab {
                                    files.comparison = comparison
                                    files.page = .changes
                                    await files.refresh()
                                } else if files.comparison != .workingTree(base: "parent") {
                                    files.comparison = .workingTree(base: "parent")
                                    await files.refresh()
                                    await files.loadDiff()
                                }
                            }
                            .simultaneousGesture(TapGesture().onEnded { focusPane() })
                    case .failure(let error):
                        EmptyState(title: "Files did not open", machine: error.localizedDescription)
                    }
                case .task:
                    if let task = model.task(id: taskId)?.task {
                        ScrollView {
                            VStack(alignment: .leading, spacing: Spacing.md) {
                                Text(task.task.name).font(Typography.textStrong)
                                TaskPlanningFactsView(task: task)
                                TaskWorkflowHeader(model: model, task: task, work: model.taskWork[taskId])
                                MarkdownBlocks(source: task.task.description).equatable()
                                TaskWorkView(model: model, task: task)
                                GraphProcessLog(model: model, taskId: taskId, location: location,
                                    execution: execution, query: sessions.query, showsPosition: false)
                            }.padding(Spacing.md)
                        }.textSelection(.enabled)
                    } else { EmptyState(title: "Task is unavailable") }
                case .url(let id, let url):
                    BrowserPageView(pages: workspace.pages, id: id, url: url).id(id)
                        .simultaneousGesture(TapGesture().onEnded { focusPane() })
                case nil:
                    EmptyState(title: "Nothing open", sentence: "Open this Task’s Files or Graph processes.") {
                        Button("Files") { workspace.showBrowser(.files, from: pane) }
                    }
                }
            }
        }
    }

    /// The pane a Session occupies before its terminal exists here. Each state
    /// explains itself and carries its one explicit next action.
    @ViewBuilder
    func _sessionPlaceholder(_ item: SessionItem) -> some View {
        switch item.state {
        case .opening, .prepared:
            SessionOpeningView(screen: item.lastScreen)
                .foregroundStyle(theme.palette.text)
        case .ended(let output):
            VStack(alignment: .leading, spacing: Spacing.md) {
                Text("No longer open here").foregroundStyle(theme.palette.text)
                ScrollView {
                    Text(output).font(Typography.mono)
                        .foregroundStyle(theme.palette.text).textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                openButton(item)
            }.padding()
        case .failed(let message):
            EmptyState(title: "Session failed to open", machine: message) {
                openButton(item)
            }
        case .pending where model.conversationIsOpening(workspace):
            SessionOpeningView(screen: item.lastScreen)
                .foregroundStyle(theme.palette.text)
        case .pending, .live:
            EmptyState(title: "Not running here", sentence: "This Session has no live terminal in this pane yet.") {
                openButton(item)
            }
        }
    }

    /// Opening is `lf resume`: it views a held terminal, resumes an ended
    /// Session, or takes over a native client; the label says which.
    @ViewBuilder
    private func openButton(_ item: SessionItem) -> some View {
        if let action = item.record.action {
            Button(action.label) {
                Perf.userAction(.taskWorkspaceReady, scenario: "session", id: item.id,
                                repository: model.repoPath, liveSessions: model.liveSessionCount,
                                owner: workspace.performanceOwner) {
                    sessions.beginPaneLoad(item.id)
                    Perf.acknowledged(.taskWorkspaceReady, id: item.id, owner: workspace.performanceOwner)
                    Task { @MainActor in await sessions.select(item.id) }
                }
            }
            .buttonStyle(LoopflowButtonStyle())
            .help(action.help)
            .accessibilityIdentifier("session-open-here-\(item.id)")
        }
    }

    @ViewBuilder
    private func _terminal(item: SessionItem, surface: SessionRecord) -> some View {
        if let location = pane.content.location, surface.workspace?.identity == location,
           let terminal = pane.terminal {
            GhosttyTerminalView(
                repository: sessions.repoPath,
                workingDirectory: sessions.presentationDirectory(for: surface, checkout: location.worktree),
                argv: item.launchArguments,
                terminal: terminal,
                surfacePool: sessions.surfaces,
                isFocused: isFocused,
                onSurfaceCreated: {
                    Perf.agentInput("surface_created", session: item.id)
                    sessions.beginPaneLoad(item.id)
                },
                onSurfaceFailed: { reason in
                    sessions.recordPaneFailure(item, reason: reason)
                },
                onFocus: focusPane
            )
            .id(item.launchId)
        } else {
            EmptyState(title: "Session location unavailable")
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
        case .session: item?.record.detail ?? "Workspace"
        case .shell: "Shell"
        case .browser: pane.content.selectedBrowserTab?.title ?? "Browser"
        }
    }

    private var programHeading: PaneProgramHeading {
        let fallback: String = switch pane.content {
        case .session: "Session"
        case .shell: "Shell"
        case .browser(let task, _, _, _, _): model.task(id: task)?.task.task.identifier ?? "Task"
        }
        return PaneProgramHeading(provider: paneSessions.count == 1 ? paneSessions.first?.presentationProvider : nil,
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

    private var _terminalIdentity: TerminalIdentity? {
        switch pane.content {
        case .session:
            item?.surface.flatMap { record in
                guard record.workspace?.identity == pane.content.location else { return nil }
                return pane.terminal
            }
        case .shell:
            pane.terminal
        case .browser:
            nil
        }
    }
}

/// Hover over a pane strip, tracked by an AppKit tracking area so it stays
/// reliable beside a Metal-backed terminal. It never takes a click.
struct PaneHoverRegion: NSViewRepresentable {
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
