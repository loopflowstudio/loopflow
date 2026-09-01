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

public enum PaneColor: CaseIterable, Sendable {
    case blue
    case amber
    case green
    case rose
    case violet
    case cyan
}

/// Owns the immutable layout tree outside SwiftUI. Views receive snapshots via
/// a notification and send every mutation back through this reference layer.
@MainActor
public final class MultiplexerStore {
    public private(set) var layout: LayoutNode
    public private(set) var focusedPaneId: String
    public private(set) var zoomedPaneId: String?

    private var paneColors: [String: PaneColor] = [:]
    private var nextColorIndex = 0
    private var closedState: ClosedState?

    public init(layout: LayoutNode = .defaultLayout()) {
        self.layout = layout
        let first = layout.firstPane
        focusedPaneId = first.id
        zoomedPaneId = nil
        for pane in layout.allPanes {
            _ = _assignColor(to: pane.id)
        }
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

    public func color(for paneId: String) -> PaneColor {
        if let color = paneColors[paneId] { return color }
        return _assignColor(to: paneId)
    }

    public func setFocusedPane(_ paneId: String) {
        guard layout.pane(for: paneId) != nil, focusedPaneId != paneId else { return }
        focusedPaneId = paneId
        if zoomedPaneId != nil { zoomedPaneId = paneId }
        _notify()
    }

    @discardableResult
    public func split(_ paneId: String, axis: SplitAxis) -> PaneState? {
        guard layout.pane(for: paneId) != nil else { return nil }
        let pane = PaneState(content: .shell)
        layout = layout.splitting(paneId, axis: axis, newPane: pane)
        focusedPaneId = pane.id
        zoomedPaneId = nil
        _ = _assignColor(to: pane.id)
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
            paneColors: paneColors,
            nextColorIndex: nextColorIndex
        )
        if layout.allPanes.count == 1 {
            layout = layout.replacingContent(of: paneId, with: .empty)
            zoomedPaneId = nil
            _notify()
            return
        }
        guard let updated = layout.removing(paneId) else { return }
        layout = updated
        paneColors.removeValue(forKey: paneId)
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
        paneColors = closedState.paneColors
        nextColorIndex = closedState.nextColorIndex
        self.closedState = nil
        _notify()
    }

    public func load(sessionId: String) {
        if let openPane = pane(forSessionId: sessionId) {
            setFocusedPane(openPane.id)
            return
        }

        layout = layout.replacingContent(
            of: focusedPaneId,
            with: .session(id: sessionId)
        )
        closedState = nil
        _notify()
    }

    public func newShell() {
        if focusedPane.content == .empty {
            layout = layout.replacingContent(of: focusedPaneId, with: .shell)
        } else {
            _ = split(focusedPaneId, axis: .vertical)
            return
        }
        closedState = nil
        _notify()
    }

    public func toggleZoom(_ paneId: String) {
        guard layout.pane(for: paneId) != nil else { return }
        zoomedPaneId = zoomedPaneId == paneId ? nil : paneId
        focusedPaneId = paneId
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

    /// Drops panes for Sessions that left the current Session list. This is
    /// reconciliation, not a user close, so it does not create an undo entry.
    public func reconcileSessions(_ sessionIds: Set<String>) {
        let stale = layout.allPanes.filter { pane in
            guard case .session(let id) = pane.content else { return false }
            return !sessionIds.contains(id)
        }
        guard !stale.isEmpty else { return }

        for pane in stale {
            if layout.allPanes.count == 1 {
                layout = .leaf(PaneState(id: pane.id, content: .empty))
            } else if let updated = layout.removing(pane.id) {
                layout = updated
                paneColors.removeValue(forKey: pane.id)
            }
        }
        if layout.pane(for: focusedPaneId) == nil {
            focusedPaneId = layout.firstPane.id
        }
        if let zoomedPaneId, layout.pane(for: zoomedPaneId) == nil {
            self.zoomedPaneId = nil
        }
        closedState = nil
        _notify()
    }

    private func _assignColor(to paneId: String) -> PaneColor {
        let colors = PaneColor.allCases
        let color = colors[nextColorIndex % colors.count]
        nextColorIndex += 1
        paneColors[paneId] = color
        return color
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
        _paneFrames(for: layout)
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
        let paneColors: [String: PaneColor]
        let nextColorIndex: Int
    }
}
