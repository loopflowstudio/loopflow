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
                PaneEmptyState(title: "Panes collapsed", systemImage: "rectangle.compress.vertical",
                               description: "Your panes are retained. Show a pane to continue.") {
                    if workspace.key.kind == .task,
                       !workspace.showsConversation(held: Set(sessions.sessions.map(\.id))) {
                        Button("Open conversation") {
                            Task { await model.openTaskConversation(workspace.key.id) }
                        }
                        .accessibilityIdentifier("task-open-conversation")
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
        Perf.endAfterCommit(Perf.retainedWorkspaceAction, id: "workspace")
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

/// The Session slot before a conversation runs here: what that means and its
/// one explicit action. Move here and an unstarted Task's Start share it.
struct SessionSlotOffer<Description: View>: View {
    let title: String
    let systemImage: String
    /// Nil when no action applies; the slot still explains itself.
    let action: String?
    let actionImage: String
    var note: String?
    var unavailable: String?
    var help: String
    var accessibilityLabel: String?
    var accessibilityHint: String?
    let actionIdentifier: String
    let perform: () -> Void
    @ViewBuilder let description: Description
    @Environment(\.theme) private var theme
    @Environment(\.compactPanes) private var compact

    var body: some View {
        PaneEmptyState(title: title, systemImage: systemImage, summary: unavailable ?? note) {
            description
        } actions: {
            if let action {
                VStack(spacing: Spacing.sm) {
                    Button(action: perform) { Label(action, systemImage: actionImage) }
                        .buttonStyle(.bordered)
                        .tint(theme.palette.text)
                        .foregroundStyle(theme.palette.text)
                        .disabled(unavailable != nil)
                        .help(unavailable ?? help)
                        .accessibilityLabel(accessibilityLabel ?? action)
                        .accessibilityHint(accessibilityHint ?? "")
                        .accessibilityIdentifier(actionIdentifier)
                    if !compact, let note = unavailable ?? note {
                        Text(note)
                            .font(Typography.caption(9))
                            .foregroundStyle(theme.palette.textSecondary)
                    }
                }
            }
        }
        .accessibilityElement(children: .contain)
    }
}

extension EnvironmentValues {
    /// Dashboard slots: a pane's empty state reads as one line sized to the slot.
    @Entry var compactPanes = false
}

/// A pane with nothing running in it yet: what that means and its next action.
/// Full panes explain themselves; a compact pane says the state in one line,
/// with `summary` as its help.
struct PaneEmptyState<Description: View, Actions: View>: View {
    let title: String
    let systemImage: String
    let summary: String?
    @ViewBuilder let description: Description
    @ViewBuilder let actions: Actions
    @Environment(\.compactPanes) private var compact
    @Environment(\.theme) private var theme

    var body: some View {
        if compact {
            // A narrow slot keeps the action and the state's icon rather than a clipped title.
            ViewThatFits(in: .horizontal) {
                compactRow(Label(title, systemImage: systemImage).fixedSize())
                compactRow(Label(title, systemImage: systemImage).labelStyle(.iconOnly))
            }
            .font(Typography.body(12))
            .padding(Spacing.sm)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            ContentUnavailableView {
                Label(title, systemImage: systemImage).foregroundStyle(theme.palette.text)
            } description: {
                description.foregroundStyle(theme.palette.textSecondary)
            } actions: { actions }
        }
    }
}

extension PaneEmptyState {
    fileprivate func compactRow(_ label: some View) -> some View {
        HStack(spacing: Spacing.sm) {
            label.foregroundStyle(theme.palette.text).help(summary ?? title)
            actions.controlSize(.small).fixedSize()
        }
    }
}

extension PaneEmptyState where Description == Text? {
    init(title: String, systemImage: String, description: String? = nil, @ViewBuilder actions: () -> Actions) {
        self.init(title: title, systemImage: systemImage, summary: description,
                  description: { description.map { Text($0) } }, actions: actions)
    }
}

extension PaneEmptyState where Description == Text?, Actions == EmptyView {
    init(title: String, systemImage: String) {
        self.init(title: title, systemImage: systemImage) { EmptyView() }
    }
}

enum PaneGeometry {
    // Keep pane outlines inside the native window's larger lower corners.
    static let windowInset: CGFloat = 10
    static let spacing: CGFloat = 6
    static let cornerRadius = CornerRadius.md
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

struct SessionPaneView: View {
    @Environment(\.theme) private var theme
    @State private var shellError: String?
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

    private var showsStrip: Bool { (store.visibleLayout?.allPanes.count ?? 0) > 1 }

    var body: some View {
        VStack(spacing: 0) {
            if showsStrip {
                strip
                Rectangle().fill(theme.palette.border).frame(height: 1)
            }
            content
                // Reserve the rounded edge before Ghostty measures its surface.
                .padding(pane.terminal == nil ? 0 : PaneGeometry.cornerRadius)
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
        .alert("Could not change pane", isPresented: Binding(
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
        HStack(spacing: 8) {
            if let tone = programTone {
                Circle().fill(tone.ink(in: theme.palette)).frame(width: 6, height: 6)
                    .accessibilityHidden(true)
            }
            Menu {
                paneContentChoices
            } label: {
                Label(programHeading.name, systemImage: programHeading.symbol)
                    .font(Typography.body(11)).fontWeight(.semibold)
                    .foregroundStyle(theme.palette.text)
            }
            .menuStyle(.borderlessButton).fixedSize()
            .disabled(changingSession)
            .help("Choose this Session's agent or open a companion pane\n\(programHeading.detail)")
            .accessibilityIdentifier("pane-program-\(pane.id)")
            if let session = primarySession {
                stripButton("arrow.clockwise", label: "Reset session", id: "pane-reset-session",
                    help: "Start fresh with current Work context. Keeps history; discards unsent input.",
                    disabled: changingSession) { changeSession(session, agent: nil) }
            }
            if case .shell = pane.content { shellTabs }
            if case .browser = pane.content { browserTabs }
            if workspace.key.kind != .task || pane.content.location != workspace.location {
                Text(programHeading.context)
                    .font(Typography.code(11)).foregroundStyle(theme.palette.textSecondary)
                    .lineLimit(1).truncationMode(.middle)
                    .help(programHeading.detail)
                    .accessibilityIdentifier("pane-context-\(pane.id)")
            }
            if let status = _programStatus, status.summary != nil {
                ProgramStatusLabel(status: status)
            }
            if let error = _programStatusSurface?.observationError {
                Image(systemName: "exclamationmark.circle")
                    .help(error).accessibilityLabel(error)
            }
            Spacer(minLength: 8)
            if let copyConfirmation {
                Text(copyConfirmation)
                    .font(Typography.body(11))
                    .foregroundStyle(theme.palette.text)
                    .lineLimit(1)
                    .allowsHitTesting(false)
                    .accessibilityIdentifier("terminal-copy-confirmation")
            }
            if bellRinging {
                Image(systemName: "bell.fill")
                    .font(.system(size: 10))
                    .foregroundStyle(theme.palette.available)
                    .accessibilityLabel("Session ready")
            }
            // Every pane zooms and hides on hover; the breadcrumb zooms out and the
            // view header's Show… brings a hidden pane back.
            HoverRevealed(flag: hover) {
                HStack(spacing: 2) {
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
        .padding(.leading, 12)
        .padding(.trailing, 4)
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
                ForEach(["claude", "codex", "opencode", "pi"], id: \.self) { agent in
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
            ForEach(TaskFilesStore.Page.allCases, id: \.self) { page in
                Button(page.rawValue, systemImage: page == .task ? "doc.text" : page == .pullRequest ? "arrow.triangle.pull" : "folder") {
                    guard let browser = workspace.showBrowser(.files, from: pane) else { return }
                    do {
                        let files = try workspace.files(for: browser.content, task: model.task(id: taskId)?.task, query: sessions.query)
                        files.page = page
                    } catch { shellError = error.localizedDescription }
                }
            }
            Button("lf process", systemImage: "list.bullet.rectangle") {
                workspace.showBrowser(.graphLog, from: pane)
            }
        }
        if workspace.key.kind == .task {
            Button("Shell", systemImage: "terminal") {
                if let shell = store.shellPane, shell.content.selectedShell != nil { store.setFocusedPane(shell.id) }
                else { do { try workspace.newShell() } catch { shellError = error.localizedDescription } }
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
        changingSession = true
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
            } catch { shellError = error.localizedDescription }
        }
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
                .font(.system(size: 11))
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

    @ViewBuilder private var shellTabs: some View {
        if case .shell(let tabs, let selected) = pane.content {
            ScrollView(.horizontal) {
                HStack(spacing: 4) {
                    ForEach(Array(tabs.enumerated()), id: \.element.id) { index, tab in
                        Button("\(index + 1)") { store.selectShell(tab.id) }
                            .accessibilityLabel("Shell \(index + 1)")
                            .padding(.horizontal, 6).padding(.vertical, 3)
                            .background(tab.id == selected ? theme.palette.surfaceElement : .clear,
                                        in: RoundedRectangle(cornerRadius: 4))
                    }
                    Button { do { try workspace.newShell() } catch { shellError = error.localizedDescription } }
                        label: { Image(systemName: "plus") }
                        .accessibilityLabel("New shell tab")
                }.buttonStyle(.plain).font(Typography.code(11))
            }.fixedSize(horizontal: false, vertical: true)
        }
    }

    @ViewBuilder private var browserTabs: some View {
        if case .browser(_, let tabs, let selected, _, _) = pane.content {
            ScrollView(.horizontal) {
                HStack(spacing: 4) {
                    ForEach(tabs) { tab in
                        Button(tab.title) { workspace.showBrowser(tab, from: pane) }
                            .lineLimit(1)
                            .padding(.horizontal, 6).padding(.vertical, 3)
                            .background(tab.id == selected ? theme.palette.surfaceElement : .clear,
                                        in: RoundedRectangle(cornerRadius: 4))
                            .accessibilityIdentifier("browser-tab-\(tab.id)")
                    }
                }.buttonStyle(.plain).font(Typography.body(11))
            }.fixedSize(horizontal: false, vertical: true)
        }
    }

    @ViewBuilder
    private var content: some View {
        switch pane.content {
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
            VStack(spacing: 0) {
                if !showsStrip { shellTabs.padding(4) }
                if let tab = pane.content.selectedShell,
                          let location = tab.location, let terminal = tab.terminal {
                    if let command = tab.command {
                        GhosttyTerminalView(
                            workingDirectory: location.machineId == workspace.localMachineId ? location.worktree : NSHomeDirectory(),
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
                if !showsStrip { browserTabs.padding(4) }
                switch pane.content.selectedBrowserTab {
                case .graphLog:
                    GraphProcessLog(model: model, taskId: taskId, location: location, execution: execution,
                        query: sessions.query)
                        .id(pane.incarnation)
                        .background(PaneFocusTarget(isFocused: isFocused))
                        .simultaneousGesture(TapGesture().onEnded { focusPane() })
                case .files:
                    let task = model.task(id: taskId)?.task
                    switch Result(catching: { try workspace.files(for: pane.content, task: task, query: sessions.query) }) {
                    case .success(let files):
                        TaskFilesView(store: files, prURL: task?.pr?.publication?.github?.url,
                            checkoutBase: task?.reference.workspace?.baseCommit, task: task, isFocused: isFocused)
                            .simultaneousGesture(TapGesture().onEnded { focusPane() })
                    case .failure(let error):
                        Text(error.localizedDescription).textSelection(.enabled)
                    }
                case .url(let id, let url):
                    BrowserPageView(pages: workspace.pages, id: id, url: url).id(id)
                        .simultaneousGesture(TapGesture().onEnded { focusPane() })
                case nil:
                    PaneEmptyState(title: "Nothing open", systemImage: "globe",
                                   description: "Open this Task’s Files or Graph processes.") {
                        Button("Files") { workspace.showBrowser(.files, from: pane) }
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var emptyWorkspace: some View {
        if workspace.openingConversation {
            ProgressView("Opening conversation…")
                .foregroundStyle(theme.palette.text)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if workspace.key.kind == .task {
            PaneEmptyState(title: "No Task conversation open", systemImage: "bubble.left",
                description: workspace.conversationError ?? "Open this Task’s recorded conversation.") {
                Button("Open conversation") { Task { await model.openTaskConversation(workspace.key.id) } }
                    .accessibilityIdentifier("task-open-conversation")
            }
        } else {
            PaneEmptyState(title: "No session open", systemImage: "terminal",
                           description: "Open the retained conversation when you’re ready.") {
                Button("Open conversation") {
                    let wave = workspace.key.kind == .wave ? model.wave(id: workspace.key.id)?.wave.name : nil
                    Task { await model.openPrimaryConversation(in: workspace, wave: wave, store: sessions) }
                }.buttonStyle(.bordered)
            }
        }
    }

    /// The pane a Session occupies before its terminal exists here. Each state
    /// explains itself and carries its one explicit next action.
    @ViewBuilder
    func _sessionPlaceholder(_ item: SessionItem) -> some View {
        switch item.state {
        case .elsewhere:
            _elsewherePlaceholder(item)
        case .opening, .prepared:
            ProgressView("Opening conversation…")
                .foregroundStyle(theme.palette.text)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .ended(let output):
            VStack(alignment: .leading, spacing: Spacing.md) {
                Text("No longer open here").foregroundStyle(theme.palette.text)
                ScrollView {
                    Text(output).font(.system(.body, design: .monospaced))
                        .foregroundStyle(theme.palette.text).textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                openButton(item)
            }.padding()
        case .failed(let message):
            PaneEmptyState(title: "Session failed to open", systemImage: "exclamationmark.triangle",
                           description: message) {
                openButton(item)
            }
        case .pending where workspace.openingConversation:
            ProgressView("Opening conversation…")
                .foregroundStyle(theme.palette.text)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .pending, .live:
            PaneEmptyState(title: "Not running here", systemImage: "terminal",
                           description: "This Session has no live terminal in this pane yet.") {
                openButton(item)
            }
        }
    }

    /// The one way back, as `lf resume`: Resume attaches to a running
    /// conversation or reopens its saved history; Move here takes it over.
    @ViewBuilder
    private func openButton(_ item: SessionItem) -> some View {
        if let action = item.record.action(.moveHere) ?? item.record.action(.open) {
            Button(action.kind == .open ? "Resume" : action.label) {
                sessions.beginPaneLoad(item.id)
                Task { @MainActor in
                    if action.kind == .moveHere { await sessions.moveHere(item.id) }
                    else { await sessions.resume(item.id) }
                }
            }
            .buttonStyle(.bordered)
            .tint(theme.palette.text)
            .foregroundStyle(theme.palette.text)
            .disabled(action.kind == .moveHere && action.unavailableReason != nil)
            .help(action.kind == .open ? "Attach to the running conversation, or reopen its saved history" : action.unavailableReason ?? action.help)
            .accessibilityIdentifier("session-open-here-\(item.id)")
        }
    }

    /// A Session whose provider client lives in another terminal. The pane
    /// explains what that means and makes the takeover explicit: nothing is
    /// stopped until Move here.
    private func _elsewherePlaceholder(_ item: SessionItem) -> some View {
        let action = item.record.action(.moveHere)
        return SessionSlotOffer(
            title: "Active in another terminal", systemImage: "rectangle.on.rectangle",
            action: action?.label, actionImage: "arrow.down.forward.square",
            note: "Moving stops the other client. Unsent text typed there is lost.",
            unavailable: action?.unavailableReason, help: action?.help ?? "",
            accessibilityLabel: "Move session here",
            accessibilityHint: "Stops the other client; unsent text typed there is lost",
            actionIdentifier: "session-move-here-\(item.id)",
            perform: {
                sessions.beginPaneLoad(item.id)
                Task { @MainActor in await sessions.moveHere(item.id) }
            }
        ) {
            Text(
                """
                \(item.record.title) is attached to a provider client outside \
                this window — Warp, another Loopflow window, or an SSH session. \
                It keeps running there until you move it.
                """
            )
        }
        .accessibilityIdentifier("session-elsewhere-\(item.id)")
    }

    @ViewBuilder
    private func _terminal(item: SessionItem, surface: SessionRecord) -> some View {
        if let location = pane.content.location, surface.workspace?.identity == location,
           let terminal = pane.terminal {
            GhosttyTerminalView(
                workingDirectory: sessions.presentationDirectory(for: surface, checkout: location.worktree),
                argv: item.launchArguments,
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
            .id(item.launchId)
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
        case .session: item?.record.detail ?? "Workspace"
        case .shell: "Shell"
        case .browser: pane.content.selectedBrowserTab?.title ?? "Browser"
        }
    }

    private var programHeading: PaneProgramHeading {
        let fallback: String = switch pane.content {
        case .session: "Session"
        case .shell: "Shell"
        case .browser: pane.content.selectedBrowserTab?.title ?? "Browser"
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
