import CoreGraphics
import Foundation
import Observation

/// What ⌘W last closed. ⇧⌘T shows that same occurrence again: a hidden pane,
/// or a tab put back at its index with its surface or page still alive.
public enum ClosedOccurrence: Equatable, Sendable {
    case pane(String)
    case shellTab(ShellTab, index: Int)
    case browserTab(BrowserTab, index: Int)
}

public enum SpatialDirection: Equatable, Sendable {
    case left
    case right
    case up
    case down
}

/// Owns the retained layout. Views and opening receipts observe this same store;
/// mutations finish on MainActor before either consumer reads the new state.
@MainActor
@Observable
public final class MultiplexerStore {
    public private(set) var layout: LayoutNode
    public private(set) var focusedPaneId: String
    public private(set) var zoomedPaneId: String?

    public private(set) var collapsedPaneIds: Set<String> = []
    public var visibleLayout: LayoutNode? { layout.visible(excluding: collapsedPaneIds) }

    /// Visibility of this exact retained occurrence within its workspace,
    /// independent of window focus or compositor visibility.
    public func isVisible(_ pane: PaneState) -> Bool {
        layout.pane(for: pane.id) == pane && !collapsedPaneIds.contains(pane.id)
            && (zoomedPaneId == nil || zoomedPaneId == pane.id)
    }

    /// The one undoable close. Only another close replaces it.
    public private(set) var lastClosed: ClosedOccurrence?
    private var focusBeforeZoom: String?

    /// A Task workspace's designated panes; Wave and repository workspaces have
    /// only the session pane. Tools start hidden until they have content.
    public convenience init(taskId: String?) {
        self.init(layout: .designated(taskId: taskId))
        collapsedPaneIds = Set(layout.allPanes.filter { $0.kind != .session }.map(\.id))
    }

    public init(layout: LayoutNode) {
        self.layout = layout
        let first = layout.firstPane
        focusedPaneId = first.id
        zoomedPaneId = nil
    }

    public var focusedPane: PaneState {
        guard let pane = layout.pane(for: focusedPaneId) else {
            preconditionFailure("Focused pane must belong to the layout")
        }
        return pane
    }

    /// Closing hides a pane; the last visible pane stays.
    public var canClose: Bool { (visibleLayout?.allPanes.count ?? 0) > 1 }
    public var canUndoClose: Bool { lastClosed != nil }

    public var sessionPane: PaneState? { layout.pane(of: .session) }
    public var shellPane: PaneState? { layout.pane(of: .shell) }
    public var browserPane: PaneState? { layout.pane(of: .browser) }

    public func pane(forSessionId sessionId: String) -> PaneState? {
        let sessionId = StructuredIdentity.session(sessionId)
        return layout.allPanes.first { pane in
            pane.sessionId == sessionId
        }
    }

    /// A fresh record changes the captured location without selecting a pane.
    /// Renew the occurrence so targets captured before a move cannot send input.
    public func updateSessionLocation(_ sessionId: String, location: WorkspaceIdentity?, execution: TaskLocationObservation? = nil) {
        let sessionId = StructuredIdentity.session(sessionId)
        func update(_ node: LayoutNode) -> LayoutNode {
            guard let pane = node.allPanes.first(where: { $0.sessionId == sessionId }) else { return node }
            let retained = pane.content.location == location ? pane.executionLocation : nil
            let content = PaneContent.session(id: sessionId, location: location, execution: execution ?? retained)
            guard pane.content != content else { return node }
            return node.replacingContent(of: pane.id, with: content)
        }
        let updated = update(layout)
        if updated != layout { layout = updated }
    }

    public func setCollapsed(paneId: String, collapsed: Bool) {
        guard layout.pane(for: paneId) != nil else { return }
        if collapsed { collapsedPaneIds.insert(paneId) }
        else { collapsedPaneIds.remove(paneId) }
        if collapsed && zoomedPaneId == paneId { zoomedPaneId = nil }
        if collapsed && focusedPaneId == paneId, let first = visibleLayout?.firstPane {
            focusedPaneId = first.id
        }
    }

    /// Show a conversation in the session pane and focus it.
    public func reveal(sessionId: String, location: WorkspaceIdentity? = nil) {
        load(sessionId: sessionId, location: location)
    }

    public func setFocusedPane(_ paneId: String) {
        guard layout.pane(for: paneId) != nil else { return }
        guard focusedPaneId != paneId || collapsedPaneIds.contains(paneId)
            || (zoomedPaneId != nil && zoomedPaneId != paneId) else { return }
        collapsedPaneIds.remove(paneId)
        focusedPaneId = paneId
        if zoomedPaneId != nil { zoomedPaneId = paneId }
    }

    /// Relocate the leaf itself, retaining content occurrence and surface keys.
    public func move(_ paneId: String, beside destination: String, axis: SplitAxis) {
        guard paneId != destination,
              let pane = layout.pane(for: paneId), layout.pane(for: destination) != nil,
              let remaining = layout.removing([paneId]) else { return }
        layout = remaining.splitting(destination, axis: axis, newPane: pane)
    }

    /// Hide a pane. Its content, tabs and surfaces stay; Undo shows it again.
    /// Returns the close this one replaced, whose retained views the caller frees.
    @discardableResult
    public func close(_ paneId: String) -> ClosedOccurrence? {
        guard layout.pane(for: paneId) != nil, !collapsedPaneIds.contains(paneId), canClose else { return nil }
        let previous = _paneFrames()
        collapsedPaneIds.insert(paneId)
        if zoomedPaneId == paneId { zoomedPaneId = nil }
        if focusedPaneId == paneId {
            focusedPaneId = _nearestPane(to: paneId, in: previous) ?? visibleLayout?.firstPane.id ?? paneId
        }
        return _record(.pane(paneId))
    }

    /// Show the last closed pane, or put its tab back where it was and select it.
    public func undoClose() {
        guard let closed = lastClosed else { return }
        lastClosed = nil
        switch closed {
        case .pane(let id):
            setFocusedPane(id)
        case .shellTab(let tab, let index):
            guard let pane = shellPane, case .shell(var tabs, _) = pane.content else { return }
            tabs.insert(tab, at: min(index, tabs.count))
            layout = layout.updatingShellTabs(of: pane.id, tabs: tabs, selected: tab.id)
            setFocusedPane(pane.id)
        case .browserTab(let tab, let index):
            guard let pane = browserPane,
                  case .browser(let taskId, var tabs, _, let place, let observed) = pane.content else { return }
            if !tabs.contains(where: { $0.id == tab.id }) { tabs.insert(tab, at: min(index, tabs.count)) }
            layout = layout.replacingBrowser(of: pane.id, with: .browser(taskId: taskId, tabs: tabs, selected: tab.id,
                location: place, execution: observed), retainingOccurrence: true)
            setFocusedPane(pane.id)
        }
    }

    private func _record(_ closed: ClosedOccurrence) -> ClosedOccurrence? {
        let replaced = lastClosed
        lastClosed = closed
        return replaced
    }

    /// Switch the session pane to this conversation. The previous one keeps
    /// running; its surface stays in the pool.
    public func load(sessionId: String, location: WorkspaceIdentity? = nil, focus: Bool = true) {
        let sessionId = StructuredIdentity.session(sessionId)
        updateSessionLocation(sessionId, location: location)
        guard let pane = sessionPane else { return }
        if pane.sessionId != sessionId {
            layout = layout.replacingContent(of: pane.id, with: .session(id: sessionId, location: location))
        }
        if focus { setFocusedPane(pane.id) }
    }

    /// Change this slot's content without disturbing its companions or split ratios.
    /// Content keeps the slot's designated kind.
    public func replace(_ paneId: String, with content: PaneContent) {
        // Swift 6.2.3's -O CopyPropagation crashes on a bound `pane` here.
        guard layout.pane(for: paneId)?.kind == content.kind else { return }
        layout = layout.replacingContent(of: paneId, with: content)
        collapsedPaneIds.remove(paneId)
        setFocusedPane(paneId)
    }

    /// Add a shell tab and select it. Focus moves only when asked; scripts keep theirs.
    @discardableResult
    public func newShell(command: [String] = [], location: WorkspaceIdentity? = nil,
                         execution: TaskLocationObservation? = nil, focus: Bool = true) -> PaneState? {
        guard let pane = shellPane, case .shell(let tabs, _) = pane.content else { return nil }
        let tab = ShellTab(location: location, execution: execution, command: command)
        layout = layout.updatingShellTabs(of: pane.id, tabs: tabs + [tab], selected: tab.id)
        if focus { setFocusedPane(pane.id) } else { setCollapsed(paneId: pane.id, collapsed: false) }
        return layout.pane(for: pane.id)
    }

    public func shell(_ id: String) -> ShellTab? {
        guard let pane = shellPane, case .shell(let tabs, _) = pane.content else { return nil }
        return tabs.first { $0.id == id }
    }

    public func selectShell(_ id: String) {
        guard let pane = shellPane, case .shell(let tabs, _) = pane.content,
              tabs.contains(where: { $0.id == id }) else { return }
        layout = layout.updatingShellTabs(of: pane.id, tabs: tabs, selected: id)
        setFocusedPane(pane.id)
    }

    /// Close a shell tab, keeping it for Undo; the last one leaving hides the shell pane.
    /// Returns the close this one replaced, whose retained views the caller frees.
    @discardableResult
    public func closeShell(_ id: String) -> ClosedOccurrence? {
        guard let pane = shellPane, case .shell(let tabs, _) = pane.content,
              let index = tabs.firstIndex(where: { $0.id == id }) else { return nil }
        _removeShell(id)
        return _record(.shellTab(tabs[index], index: index))
    }

    /// Drop a shell tab whose process on `machineId` exited, open or kept for Undo.
    public func removeExitedShell(_ id: String, machineId: String) {
        if case .shellTab(let tab, _) = lastClosed, tab.id == id, tab.location?.machineId == machineId {
            lastClosed = nil
        }
        guard shell(id)?.location?.machineId == machineId else { return }
        _removeShell(id)
    }

    private func _removeShell(_ id: String) {
        guard let pane = shellPane, case .shell(let tabs, let selected) = pane.content,
              let (remaining, next) = Self._closing(id, in: tabs, selected: selected) else { return }
        layout = layout.updatingShellTabs(of: pane.id, tabs: remaining, selected: next)
        if remaining.isEmpty { setCollapsed(paneId: pane.id, collapsed: true) }
    }

    /// The tabs left after closing one, and the selection: unchanged, else the
    /// tab that took the closed one's place.
    private static func _closing<Tab: Identifiable>(_ id: String, in tabs: [Tab], selected: String?)
        -> (remaining: [Tab], selected: String?)? where Tab.ID == String {
        guard let index = tabs.firstIndex(where: { $0.id == id }) else { return nil }
        var remaining = tabs
        remaining.remove(at: index)
        guard selected == id else { return (remaining, selected) }
        return (remaining, remaining.isEmpty ? nil : remaining[min(index, remaining.count - 1)].id)
    }

    /// Select a browser tab, adding it when absent. A fresh owner observation
    /// replaces the recorded one; missing evidence never discards it.
    @discardableResult
    public func showBrowser(_ tab: BrowserTab, location: WorkspaceIdentity? = nil,
                            execution: TaskLocationObservation? = nil, focus: Bool = true) -> PaneState? {
        guard let pane = browserPane,
              case .browser(let taskId, var tabs, _, let place, let observed) = pane.content else { return nil }
        if !tabs.contains(where: { $0.id == tab.id }) { tabs.append(tab) }
        let fresh = execution != nil || location != nil
        let content = PaneContent.browser(taskId: taskId, tabs: tabs, selected: tab.id,
            location: fresh ? location : place, execution: fresh ? execution : observed)
        if content != pane.content {
            layout = layout.replacingBrowser(of: pane.id, with: content, retainingOccurrence: content.location == pane.content.location)
        }
        if focus { setFocusedPane(pane.id) } else { setCollapsed(paneId: pane.id, collapsed: false) }
        return layout.pane(for: pane.id)
    }

    /// Close a browser tab, keeping it for Undo; the last one leaving hides the browser pane.
    /// Returns the close this one replaced, whose retained views the caller frees.
    @discardableResult
    public func closeBrowserTab(_ id: String) -> ClosedOccurrence? {
        guard let pane = browserPane,
              case .browser(let taskId, let tabs, let selected, let place, let observed) = pane.content,
              let index = tabs.firstIndex(where: { $0.id == id }),
              let (remaining, next) = Self._closing(id, in: tabs, selected: selected) else { return nil }
        layout = layout.replacingBrowser(of: pane.id, with: .browser(taskId: taskId, tabs: remaining, selected: next,
            location: place, execution: observed), retainingOccurrence: true)
        if remaining.isEmpty { setCollapsed(paneId: pane.id, collapsed: true) }
        return _record(.browserTab(tabs[index], index: index))
    }

    public func paneLayout(work: WorkWorkspaceKey, revision: Int64,
                           files: WorkBrowserTab?, drafts: [WorkFileDraft]) -> WorkPaneLayout {
        func place(_ value: WorkspaceIdentity?) -> WorkViewPlan.Location? {
            value.map { WorkViewPlan.Location(machineID: $0.machineId, cwd: $0.worktree) }
        }
        func tree(_ node: LayoutNode) -> WorkPaneTree {
            switch node {
            case .split(let axis, let first, let second, let ratio):
                return .split(axis: axis, ratio: ratio,
                              first: tree(first), second: tree(second))
            case .leaf(let pane):
                let intent: WorkPaneIntent
                switch pane.content {
                case .session(let id, let location, _): intent = .session(id: id, location: place(location))
                case .shell(let tabs, let selected):
                    intent = .shell(tabs: tabs.map { tab in
                        WorkShellTab(id: tab.id, location: place(tab.location), lfProcessID: tab.lfProcessID)
                    }, selected: selected)
                case .browser(_, let tabs, let selected, _, _):
                    intent = .browser(location: place(pane.content.location), tabs: tabs.map { tab in
                        switch tab {
                        case .files: files ?? .files(id: tab.id, tabs: [], selected: nil)
                        case .graphLog: .graphLog(id: tab.id)
                        case .url(let id, let url): .url(id: id, url: url.absoluteString)
                        }
                    }, selected: selected)
                }
                return .pane(id: pane.id, content: intent)
            }
        }
        return WorkPaneLayout(work: work, revision: revision, tree: tree(layout), focused: focusedPaneId,
                              zoomed: zoomedPaneId, hidden: collapsedPaneIds.sorted(), drafts: drafts)
    }

    /// Restore portable identity with fresh native occurrences. No command is replayed.
    public func restore(_ saved: WorkPaneLayout) {
        func location(_ value: WorkViewPlan.Location?) -> WorkspaceIdentity? {
            value.map { WorkspaceIdentity(machineId: $0.machineID, worktree: $0.cwd) }
        }
        func tree(_ node: WorkPaneTree) -> LayoutNode {
            switch node {
            case .split(let axis, let ratio, let first, let second):
                return .split(axis,
                              first: tree(first), second: tree(second), ratio: ratio)
            case .pane(let id, let intent):
                let content: PaneContent
                switch intent {
                case .session(let session, let place): content = .session(id: session, location: location(place))
                case .shell(let tabs, let selected):
                    content = .shell(tabs: tabs.map { tab in
                        self.shell(tab.id) ?? ShellTab(id: tab.id, location: location(tab.location), command: nil, lfProcessID: tab.lfProcessID)
                    }, selected: selected)
                case .browser(let place, let tabs, let selected):
                    let restored = tabs.compactMap { tab -> BrowserTab? in
                        switch tab {
                        case .files: .files
                        case .graphLog: .graphLog
                        case .url(let id, let url): URL(string: url).flatMap { BrowserTab.page($0, id: id) }
                        }
                    }
                    content = .browser(taskId: saved.work.id, tabs: restored,
                        selected: restored.contains { $0.id == selected } ? selected : restored.first?.id,
                        location: location(place))
                }
                if let retained = layout.pane(for: id), retained.content == content { return .leaf(retained) }
                return .leaf(PaneState(id: id, content: content))
            }
        }
        layout = tree(saved.tree)
        focusedPaneId = saved.focused ?? visibleLayout?.firstPane.id ?? layout.firstPane.id
        zoomedPaneId = saved.zoomed
        collapsedPaneIds = Set(saved.hidden)
    }

    /// Explicit arrangement changes visibility without selecting another Session.
    public func setZoom(_ paneId: String, enabled: Bool) {
        guard layout.pane(for: paneId) != nil else { return }
        if enabled {
            collapsedPaneIds.remove(paneId)
            zoomedPaneId = paneId
        } else if zoomedPaneId == paneId {
            zoomedPaneId = nil
        } else { return }
        focusBeforeZoom = nil
    }

    public func toggleZoom(_ paneId: String) {
        guard layout.pane(for: paneId) != nil else { return }
        if zoomedPaneId == paneId {
            zoomedPaneId = nil
            if let previous = focusBeforeZoom, layout.pane(for: previous) != nil {
                focusedPaneId = previous
            }
            focusBeforeZoom = nil
        } else {
            if zoomedPaneId == nil { focusBeforeZoom = focusedPaneId }
            zoomedPaneId = paneId
            focusedPaneId = paneId
        }
    }

    public func updateRatio(
        between firstPaneId: String,
        and secondPaneId: String,
        ratio: Double
    ) {
        let updated = layout.updatingRatio(
            between: firstPaneId,
            and: secondPaneId,
            ratio: ratio
        )
        guard updated != layout else { return }
        layout = updated
    }

    public func focus(_ direction: SpatialDirection) {
        let frames = _paneFrames()
        guard let current = frames[focusedPaneId] else { return }
        let origin = CGPoint(x: current.midX, y: current.midY)
        let candidate = frames
            .filter { id, frame in
                guard id != focusedPaneId else { return false }
                return switch direction {
                case .left: frame.midX < origin.x
                case .right: frame.midX > origin.x
                case .up: frame.midY < origin.y
                case .down: frame.midY > origin.y
                }
            }
            .min { lhs, rhs in
                _spatialScore(from: current, to: lhs.value, direction: direction)
                    < _spatialScore(from: current, to: rhs.value, direction: direction)
            }

        if let candidate {
            focusedPaneId = candidate.key
            zoomedPaneId = nil
        }
    }

    /// Clears confirmed resolved or moved Sessions from the session pane without
    /// creating an undo entry; the pane shows the start state.
    public func removeSessions(_ sessionIds: Set<String>) {
        func clear(_ node: LayoutNode) -> LayoutNode {
            guard let pane = node.pane(of: .session), let id = pane.sessionId, sessionIds.contains(id) else { return node }
            return node.replacingContent(of: pane.id, with: .session(id: nil))
        }
        layout = clear(layout)
    }

    private func _nearestPane(to paneId: String, in previousFrames: [String: CGRect]) -> String? {
        guard let removed = previousFrames[paneId] else { return nil }
        let center = CGPoint(x: removed.midX, y: removed.midY)
        return _paneFrames().min { lhs, rhs in
            hypot(lhs.value.midX - center.x, lhs.value.midY - center.y)
                < hypot(rhs.value.midX - center.x, rhs.value.midY - center.y)
        }?.key
    }

    private func _paneFrames() -> [String: CGRect] {
        visibleLayout.map { _paneFrames(for: $0) } ?? [:]
    }

    private func _paneFrames(for layout: LayoutNode) -> [String: CGRect] {
        var frames: [String: CGRect] = [:]
        _collectFrames(
            layout,
            rect: CGRect(x: 0, y: 0, width: 1, height: 1),
            into: &frames
        )
        return frames
    }

    private func _collectFrames(
        _ node: LayoutNode,
        rect: CGRect,
        into frames: inout [String: CGRect]
    ) {
        switch node {
        case .leaf(let pane):
            frames[pane.id] = rect
        case .split(let axis, let first, let second, let ratio):
            let vertical = axis == .vertical
            let (firstRect, secondRect) = rect.divided(
                atDistance: (vertical ? rect.width : rect.height) * ratio,
                from: vertical ? .minXEdge : .minYEdge
            )
            _collectFrames(first, rect: firstRect, into: &frames)
            _collectFrames(second, rect: secondRect, into: &frames)
        }
    }

    private func _spatialScore(
        from current: CGRect,
        to candidate: CGRect,
        direction: SpatialDirection
    ) -> CGFloat {
        let horizontal = direction == .left || direction == .right
        let primary = horizontal
            ? abs(candidate.midX - current.midX)
            : abs(candidate.midY - current.midY)
        let perpendicular = horizontal
            ? abs(candidate.midY - current.midY)
            : abs(candidate.midX - current.midX)
        let overlaps = horizontal
            ? candidate.maxY > current.minY && candidate.minY < current.maxY
            : candidate.maxX > current.minX && candidate.minX < current.maxX
        return primary + perpendicular + (overlaps ? 0 : 10)
    }
}
