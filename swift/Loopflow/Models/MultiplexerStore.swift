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
    public private(set) var shellCommands: [String: [String]] = [:]

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
        layout.allPanes.first { pane in
            pane.sessionId == sessionId
        }
    }

    /// A fresh record changes the captured location without selecting a pane.
    /// Renew the occurrence so targets captured before a move cannot send input.
    public func updateSessionLocation(_ sessionId: String, location: WorkspaceIdentity?, execution: TaskLocationObservation? = nil) {
        func update(_ node: LayoutNode) -> LayoutNode {
            guard let pane = node.allPanes.first(where: { $0.sessionId == sessionId }) else { return node }
            let retained = pane.sessionLocation == location ? pane.executionLocation : nil
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

    @discardableResult
    public func split(_ paneId: String, axis: SplitAxis, command: [String] = [], location: WorkspaceIdentity? = nil, execution: TaskLocationObservation? = nil, focusNewPane: Bool = true) -> PaneState? {
        _insert(.shell(location: location, execution: execution), beside: paneId, focus: focusNewPane, splitAxis: axis, shellCommand: command)
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
        closedState = ClosedState(
            layout: layout.replacingContent(of: paneId, with: pane.content),
            focusedPaneId: focusedPaneId,
            zoomedPaneId: zoomedPaneId,
            collapsedPaneIds: collapsedPaneIds
        )
        // Undo may restore a shell pane, but must never replay a completed or
        // interrupted conversation's initial launch command.
        collapsedPaneIds.remove(paneId)
        shellCommands.removeValue(forKey: paneId)
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

    @discardableResult
    public func newShell(command: [String] = [], location: WorkspaceIdentity? = nil, execution: TaskLocationObservation? = nil, beside paneId: String? = nil, focus: Bool = true) -> PaneState? {
        _insert(.shell(location: location, execution: execution), beside: paneId ?? focusedPaneId, focus: focus, shellCommand: command)
    }

    /// Reveal a Task's Flow process log or files beside existing terminals,
    /// never replacing them. Explicit targets do not follow later focus.
    @discardableResult
    public func show(_ content: PaneContent, beside paneId: String? = nil, focus: Bool = true) -> PaneState? {
        let existing = layout.allPanes.first { pane in
            switch (pane.content, content) {
            case (.files(let current, _, _), .files(let requested, _, _)),
                 (.flowLog(let current, _), .flowLog(let requested, _)):
                return current == requested
            default: return pane.content == content
            }
        }
        if let pane = existing {
            // A fresh observation can replace the captured owner, but missing
            // evidence cannot discard the retained remote location or draft.
            if content.executionLocation != nil || content.fileLocation != nil, pane.content != content {
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

    /// Fill empty targets unless an explicit split was requested. Update the
    /// occurrence, launch command, focus and Undo together on MainActor.
    @discardableResult
    private func _insert(
        _ content: PaneContent,
        beside paneId: String,
        focus: Bool = true,
        splitAxis: SplitAxis? = nil,
        shellCommand: [String]? = nil
    ) -> PaneState? {
        guard let target = layout.pane(for: paneId) else { return nil }
        let axis = splitAxis ?? (target.content == .empty ? nil : .vertical)
        let insertedId: String
        if let axis {
            let pane = PaneState(content: content)
            insertedId = pane.id
            layout = layout.splitting(paneId, axis: axis, newPane: pane)
        } else {
            insertedId = paneId
            layout = layout.replacingContent(of: paneId, with: content)
            collapsedPaneIds.remove(paneId)
        }
        if let shellCommand { shellCommands[insertedId] = shellCommand }
        if focus {
            focusedPaneId = insertedId
            zoomedPaneId = axis == nil && zoomedPaneId != nil ? insertedId : nil
        }
        closedState = nil
        return layout.pane(for: insertedId)
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
