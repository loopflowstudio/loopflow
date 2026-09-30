import CoreGraphics
import Foundation

public extension Notification.Name {
    static let multiplexerStoreDidChange = Notification.Name("loopflow.multiplexerStoreDidChange")
}

public enum SpatialDirection: Equatable, Sendable {
    case left
    case right
    case up
    case down
}

/// Owns the immutable layout tree outside SwiftUI. Views receive snapshots via
/// a notification and send every mutation back through this reference layer.
@MainActor
public final class MultiplexerStore {
    public private(set) var layout: LayoutNode
    public private(set) var focusedPaneId: String
    public private(set) var zoomedPaneId: String?
    public private(set) var shellCommands: [String: [String]] = [:]

    public private(set) var collapsedPaneIds: Set<String> = []
    public var visibleLayout: LayoutNode? { layout.visible(excluding: collapsedPaneIds) }

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
            pane.content == .session(id: sessionId)
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
        _notify()
    }

    public func reveal(sessionId: String) {
        if let pane = pane(forSessionId: sessionId) {
            setCollapsed(paneId: pane.id, collapsed: false)
            setFocusedPane(pane.id)
        } else if focusedPane.content == .empty {
            load(sessionId: sessionId)
            setCollapsed(paneId: focusedPaneId, collapsed: false)
        } else {
            _ = _split(focusedPaneId, axis: .vertical, content: .session(id: sessionId))
        }
    }

    public func setFocusedPane(_ paneId: String) {
        guard layout.pane(for: paneId) != nil, focusedPaneId != paneId else { return }
        collapsedPaneIds.remove(paneId)
        focusedPaneId = paneId
        if zoomedPaneId != nil { zoomedPaneId = paneId }
        _notify()
    }

    @discardableResult
    public func split(_ paneId: String, axis: SplitAxis) -> PaneState? {
        _split(paneId, axis: axis, content: .empty)
    }

    private func _split(_ paneId: String, axis: SplitAxis, content: PaneContent) -> PaneState? {
        guard layout.pane(for: paneId) != nil else { return nil }
        let pane = PaneState(content: content)
        layout = layout.splitting(paneId, axis: axis, newPane: pane)
        focusedPaneId = pane.id
        zoomedPaneId = nil
        closedState = nil
        _notify()
        return pane
    }

    public func close(_ paneId: String) {
        guard let pane = layout.pane(for: paneId),
              layout.allPanes.count > 1 || pane.content != .empty
        else { return }

        closedState = ClosedState(
            layout: layout,
            focusedPaneId: focusedPaneId,
            zoomedPaneId: zoomedPaneId,
            collapsedPaneIds: collapsedPaneIds
        )
        // Undo may restore a shell pane, but must never replay a completed or
        // interrupted conversation's initial launch command.
        collapsedPaneIds.remove(paneId)
        shellCommands.removeValue(forKey: paneId)
        if layout.allPanes.count == 1 {
            layout = layout.replacingContent(of: paneId, with: .empty)
            zoomedPaneId = nil
            _notify()
            return
        }
        guard let updated = layout.removing(paneId) else { return }
        layout = updated
        if zoomedPaneId == paneId { zoomedPaneId = nil }
        if focusedPaneId == paneId {
            focusedPaneId = _nearestPane(to: paneId, in: closedState?.layout)
                ?? updated.firstPane.id
        }
        _notify()
    }

    public func undoClose() {
        guard let closedState else { return }
        layout = closedState.layout
        focusedPaneId = closedState.focusedPaneId
        zoomedPaneId = closedState.zoomedPaneId
        collapsedPaneIds = closedState.collapsedPaneIds
        self.closedState = nil
        _notify()
    }

    public func load(sessionId: String) {
        if let openPane = pane(forSessionId: sessionId) {
            setFocusedPane(openPane.id)
            return
        }

        switch focusedPane.content {
        case .shell, .monitor:
            _ = _split(focusedPaneId, axis: .vertical, content: .session(id: sessionId))
            return
        case .empty, .session:
            break
        }

        layout = layout.replacingContent(
            of: focusedPaneId,
            with: .session(id: sessionId)
        )
        closedState = nil
        _notify()
    }

    public func newShell(command: [String] = []) {
        if focusedPane.content == .empty {
            shellCommands[focusedPaneId] = command
            layout = layout.replacingContent(of: focusedPaneId, with: .shell)
        } else {
            if let pane = _split(focusedPaneId, axis: .vertical, content: .shell) {
                shellCommands[pane.id] = command
                _notify()
            }
            return
        }
        closedState = nil
        _notify()
    }

    /// Reveal one Task's observation beside existing terminals, never replacing them.
    public func showMonitor(taskId: String) {
        let content = PaneContent.monitor(taskId: taskId)
        if let pane = layout.allPanes.first(where: { $0.content == content }) {
            setFocusedPane(pane.id)
        } else if focusedPane.content == .empty {
            layout = layout.replacingContent(of: focusedPaneId, with: content)
            closedState = nil
            _notify()
        } else {
            _ = _split(focusedPaneId, axis: .vertical, content: content)
        }
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
        _notify()
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
        _notify()
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
            _notify()
        }
    }

    /// Removes confirmed resolved or moved Sessions without creating an undo entry.
    public func removeSessions(_ sessionIds: Set<String>) {
        let stale = layout.allPanes.filter { pane in
            guard case .session(let id) = pane.content else { return false }
            return sessionIds.contains(id)
        }
        let undoIsStale = closedState?.layout.allPanes.contains { pane in
            guard case .session(let id) = pane.content else { return false }
            return sessionIds.contains(id)
        } == true
        guard !stale.isEmpty || undoIsStale else { return }

        for pane in stale {
            collapsedPaneIds.remove(pane.id)
            if zoomedPaneId == pane.id { zoomedPaneId = nil }
            if layout.allPanes.count == 1 {
                layout = .leaf(PaneState(id: pane.id, content: .empty))
            } else if let updated = layout.removing(pane.id) {
                layout = updated
            }
        }
        if layout.pane(for: focusedPaneId) == nil {
            focusedPaneId = layout.firstPane.id
        }
        closedState = nil
        _notify()
    }

    private func _notify() {
        NotificationCenter.default.post(name: .multiplexerStoreDidChange, object: self)
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
            switch axis {
            case .vertical:
                let firstWidth = rect.width * ratio
                _collectFrames(
                    first,
                    rect: CGRect(
                        x: rect.minX,
                        y: rect.minY,
                        width: firstWidth,
                        height: rect.height
                    ),
                    into: &frames
                )
                _collectFrames(
                    second,
                    rect: CGRect(
                        x: rect.minX + firstWidth,
                        y: rect.minY,
                        width: rect.width - firstWidth,
                        height: rect.height
                    ),
                    into: &frames
                )
            case .horizontal:
                let firstHeight = rect.height * ratio
                _collectFrames(
                    first,
                    rect: CGRect(
                        x: rect.minX,
                        y: rect.minY,
                        width: rect.width,
                        height: firstHeight
                    ),
                    into: &frames
                )
                _collectFrames(
                    second,
                    rect: CGRect(
                        x: rect.minX,
                        y: rect.minY + firstHeight,
                        width: rect.width,
                        height: rect.height - firstHeight
                    ),
                    into: &frames
                )
            }
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
        let layout: LayoutNode
        let focusedPaneId: String
        let zoomedPaneId: String?
        let collapsedPaneIds: Set<String>
    }
}
