import CoreGraphics
import Foundation
import Observation

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

    private var closedState: ClosedState?
    private var focusBeforeZoom: String?

    public init(layout: LayoutNode = .defaultLayout()) {
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

    public var canClose: Bool {
        layout.allPanes.count > 1 || focusedPane.content != .empty
    }
    public var canUndoClose: Bool { closedState != nil }

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
        if var closed = closedState {
            closed.layout = update(closed.layout)
            closedState = closed
        }
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

    public func reveal(sessionId: String, location: WorkspaceIdentity? = nil) {
        let sessionId = StructuredIdentity.session(sessionId)
        updateSessionLocation(sessionId, location: location)
        if let pane = pane(forSessionId: sessionId) {
            setFocusedPane(pane.id)
        } else if focusedPane.content == .empty {
            load(sessionId: sessionId, location: location)
        } else {
            _insert(.session(id: sessionId, location: location), beside: focusedPaneId)
        }
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
    /// Never use close/load: those invalidate targets and may retire a shell.
    public func move(_ paneId: String, beside destination: String, axis: SplitAxis) {
        guard paneId != destination,
              let pane = layout.pane(for: paneId), layout.pane(for: destination) != nil,
              let remaining = layout.removing([paneId]) else { return }
        layout = remaining.splitting(destination, axis: axis, newPane: pane)
        closedState = nil
    }

    public func close(_ paneId: String) {
        guard let pane = layout.pane(for: paneId) else { return }
        let remaining = layout.removing([paneId])
        guard remaining != nil || pane.content != .empty else { return }

        // Undo restores this pane as a new occurrence, never reviving a stale
        // control target. Other panes retain their existing occurrences.
        let undoContent: PaneContent
        if case .shell(let tabs, let selected) = pane.content {
            undoContent = .shell(tabs: tabs.map {
                ShellTab(id: $0.id, location: $0.location, execution: $0.execution, command: nil, lfProcessID: $0.lfProcessID)
            }, selected: selected)
        } else { undoContent = pane.content }
        closedState = ClosedState(
            layout: layout.replacingContent(of: paneId, with: undoContent),
            focusedPaneId: focusedPaneId,
            zoomedPaneId: zoomedPaneId,
            collapsedPaneIds: collapsedPaneIds
        )
        // Undo may restore a shell pane, but must never replay a completed or
        // interrupted conversation's initial launch command.
        collapsedPaneIds.remove(paneId)
        layout = remaining ?? .leaf(PaneState(id: paneId, content: .empty))
        if zoomedPaneId == paneId { zoomedPaneId = nil }
        if focusedPaneId == paneId, remaining != nil {
            focusedPaneId = _nearestPane(to: paneId, in: closedState?.layout)
                ?? layout.firstPane.id
        }
    }

    public func undoClose() {
        guard let closedState else { return }
        layout = closedState.layout
        focusedPaneId = closedState.focusedPaneId
        zoomedPaneId = closedState.zoomedPaneId
        collapsedPaneIds = closedState.collapsedPaneIds
        self.closedState = nil
    }

    public func load(sessionId: String, location: WorkspaceIdentity? = nil) {
        let sessionId = StructuredIdentity.session(sessionId)
        updateSessionLocation(sessionId, location: location)
        if let openPane = pane(forSessionId: sessionId) {
            if collapsedPaneIds.contains(openPane.id), openPane.id != focusedPaneId,
               case .session = focusedPane.content {
                // Opening in the active slot must not resurrect a hidden split.
                // Keep the displaced Session in the hidden slot for later reveal.
                let previous = focusedPane.content
                layout = layout.replacingContent(of: focusedPaneId, with: openPane.content)
                    .replacingContent(of: openPane.id, with: previous)
                collapsedPaneIds.remove(focusedPaneId)
                closedState = nil
                return
            }
            setFocusedPane(openPane.id)
            return
        }

        switch focusedPane.content {
        case .shell, .flowLog, .files:
            _insert(.session(id: sessionId, location: location), beside: focusedPaneId)
            return
        case .empty, .session:
            break
        }

        layout = layout.replacingContent(
            of: focusedPaneId,
            with: .session(id: sessionId, location: location)
        )
        collapsedPaneIds.remove(focusedPaneId)
        closedState = nil
    }

    /// Change this slot's content without disturbing its companions or split ratios.
    public func replace(_ paneId: String, with content: PaneContent) {
        guard layout.pane(for: paneId) != nil else { return }
        layout = layout.replacingContent(of: paneId, with: content)
        collapsedPaneIds.remove(paneId)
        setFocusedPane(paneId)
        closedState = nil
    }

    @discardableResult
    public func newShell(command: [String] = [], location: WorkspaceIdentity? = nil, execution: TaskLocationObservation? = nil) -> PaneState? {
        let tab = ShellTab(location: location, execution: execution, command: command)
        if let pane = shellPane, case .shell(let tabs, _) = pane.content {
            layout = layout.updatingShellTabs(of: pane.id, tabs: tabs + [tab], selected: tab.id)
            setFocusedPane(pane.id)
            closedState = nil
            return layout.pane(for: pane.id)
        }
        return _insert(.shell(tabs: [tab], selected: tab.id), beside: focusedPaneId)
    }

    public var shellPane: PaneState? {
        layout.allPanes.first { if case .shell = $0.content { true } else { false } }
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

    public func closeShell(_ id: String) {
        guard let pane = shellPane, case .shell(let tabs, let selected) = pane.content,
              let index = tabs.firstIndex(where: { $0.id == id }) else { return }
        var remaining = tabs
        remaining.remove(at: index)
        guard !remaining.isEmpty else { close(pane.id); return }
        let next = selected == id ? remaining[min(index, remaining.count - 1)].id : selected
        layout = layout.updatingShellTabs(of: pane.id, tabs: remaining, selected: next)
        closedState = nil
    }

    /// Reveal a Task's Flow process log or files beside existing terminals,
    /// never replacing them. Explicit targets do not follow later focus.
    @discardableResult
    public func show(_ content: PaneContent, beside paneId: String? = nil, focus: Bool = true) -> PaneState? {
        let content = content.canonicalized
        let existing = layout.allPanes.first { pane in
            switch (pane.content, content) {
            case (.files, .files): return true
            case (.shell, .shell): return true
            case (.flowLog(let current, _, _), .flowLog(let requested, _, _)):
                return current == requested
            default: return pane.content == content
            }
        }
        if let pane = existing {
            // A fresh observation can replace the captured owner, but missing
            // evidence cannot discard the retained remote location or draft.
            if content.selectedShell == nil, content.executionLocation != nil || content.location != nil, pane.content != content {
                layout = layout.replacingContent(of: pane.id, with: content)
                closedState = nil
            }
            if focus { setFocusedPane(pane.id) }
            else { setCollapsed(paneId: pane.id, collapsed: false) }
            return layout.pane(for: pane.id)
        } else {
            return _insert(content, beside: paneId ?? focusedPaneId, focus: focus)
        }
    }

    /// Fill an empty slot or insert beside it, updating focus and Undo together.
    @discardableResult
    private func _insert(
        _ content: PaneContent,
        beside paneId: String,
        focus: Bool = true
    ) -> PaneState? {
        guard let target = layout.pane(for: paneId) else { return nil }
        let axis: SplitAxis? = target.content == .empty ? nil :
            (layout.allPanes.count.isMultiple(of: 2) ? .horizontal : .vertical)
        let insertedId: String
        if let axis {
            let pane = PaneState(content: content)
            insertedId = pane.id
            let ratio: Double
            if case .shell = content { ratio = LayoutNode.goldenRatio }
            else { ratio = 0.5 }
            layout = layout.splitting(paneId, axis: axis, newPane: pane, ratio: ratio)
        } else {
            insertedId = paneId
            layout = layout.replacingContent(of: paneId, with: content)
            collapsedPaneIds.remove(paneId)
        }
        if focus {
            focusedPaneId = insertedId
            zoomedPaneId = axis == nil && zoomedPaneId != nil ? insertedId : nil
        }
        closedState = nil
        return layout.pane(for: insertedId)
    }

    public func paneLayout(work: WorkWorkspaceKey, revision: Int64,
                           files: WorkPaneIntent?, drafts: [WorkFileDraft]) -> WorkPaneLayout {
        func tree(_ node: LayoutNode) -> WorkPaneTree {
            switch node {
            case .split(let axis, let first, let second, let ratio):
                return .split(axis: axis, ratio: ratio,
                              first: tree(first), second: tree(second))
            case .leaf(let pane):
                let location = pane.content.location.map { WorkViewPlan.Location(machineID: $0.machineId, cwd: $0.worktree) }
                let intent: WorkPaneIntent
                switch pane.content {
                case .empty: intent = .empty
                case .session(let id, _, _): intent = .session(id: id, location: location)
                case .shell(let tabs, let selected):
                    intent = .shell(tabs: tabs.map { tab in
                        WorkShellTab(id: tab.id,
                            location: tab.location.map { WorkViewPlan.Location(machineID: $0.machineId, cwd: $0.worktree) },
                            lfProcessID: tab.lfProcessID)
                    }, selected: selected)
                case .files: intent = files ?? .files(location: location, tabs: [], selected: nil)
                case .flowLog: intent = .log(location: location)
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
                case .empty: content = .empty
                case .session(let session, let place): content = .session(id: session, location: location(place))
                case .shell(let tabs, let selected):
                    content = .shell(tabs: tabs.map { tab in
                        self.shell(tab.id) ?? ShellTab(id: tab.id, location: location(tab.location), command: nil, lfProcessID: tab.lfProcessID)
                    }, selected: selected)
                case .files(let place, _, _): content = .files(taskId: saved.work.id, location: location(place))
                case .log(let place): content = .flowLog(taskId: saved.work.id, location: location(place))
                }
                if let retained = layout.pane(for: id), retained.content == content { return .leaf(retained) }
                return .leaf(PaneState(id: id, content: content))
            }
        }
        layout = saved.tree.map(tree) ?? .defaultLayout()
        focusedPaneId = saved.focused ?? layout.firstPane.id
        zoomedPaneId = saved.zoomed
        collapsedPaneIds = Set(saved.hidden)
        closedState = nil
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

    /// Removes confirmed resolved or moved Sessions without creating an undo entry.
    public func removeSessions(_ sessionIds: Set<String>) {
        let removes: (PaneState) -> Bool = { pane in
            guard case .session(let id, _, _) = pane.content else { return false }
            return sessionIds.contains(id)
        }
        let stale = layout.allPanes.filter(removes)
        let undoIsStale = closedState?.layout.allPanes.contains(where: removes) == true
        guard !stale.isEmpty || undoIsStale else { return }

        if let last = stale.last {
            let removed = Set(stale.map(\.id))
            // Keep the final slot when all Sessions leave, as single removals do.
            layout = layout.removing(removed) ?? .leaf(PaneState(id: last.id, content: .empty))
            collapsedPaneIds.subtract(removed)
            if let zoomedPaneId, removed.contains(zoomedPaneId) { self.zoomedPaneId = nil }
        }
        if layout.pane(for: focusedPaneId) == nil {
            focusedPaneId = layout.firstPane.id
        }
        closedState = nil
    }

    private func _nearestPane(to paneId: String, in previousLayout: LayoutNode?) -> String? {
        guard let previousLayout else { return nil }
        let previousFrames = _paneFrames(for: previousLayout)
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

    private struct ClosedState {
        var layout: LayoutNode
        let focusedPaneId: String
        let zoomedPaneId: String?
        let collapsedPaneIds: Set<String>
    }
}
