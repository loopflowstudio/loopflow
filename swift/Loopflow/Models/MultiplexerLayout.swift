// Recursive binary split tree for the Sessions multiplexer. The reference
// store owns this value; SwiftUI only renders immutable snapshots.

import Foundation

/// The divider orientation. Vertical places the second pane to the right;
/// horizontal places it below.
public enum SplitAxis: String, Codable, Sendable, Equatable {
    case horizontal
    case vertical
}

public enum PaneContent: Codable, Sendable, Equatable {
    case empty
    case session(id: String)
    case shell
    /// Every Flow process of a Task.
    case flowLog(taskId: String)
    /// A Task's worktree files.
    case files(taskId: String)
}

public struct PaneState: Codable, Sendable, Identifiable, Equatable {
    public let id: String
    public let content: PaneContent
    /// The content occurrence, distinct from the reusable pane slot.
    public let incarnation: String

    public init(
        id: String = UUID().uuidString,
        content: PaneContent
    ) {
        self.id = id
        self.content = content
        self.incarnation = UUID().uuidString
    }
}

public indirect enum LayoutNode: Codable, Sendable, Equatable {
    case leaf(PaneState)
    case split(SplitAxis, first: LayoutNode, second: LayoutNode, ratio: Double)

    public static func defaultLayout() -> LayoutNode {
        .leaf(PaneState(content: .empty))
    }

    public func visible(excluding collapsed: Set<String>) -> LayoutNode? {
        _filterPanes { !collapsed.contains($0.id) }
    }

    /// Removes a leaf and collapses its parent. Removing the final leaf returns nil.
    public func removing(_ paneId: String) -> LayoutNode? {
        _filterPanes { $0.id != paneId }
    }

    /// Both visibility and removal preserve surviving leaves and divider ratios.
    private func _filterPanes(_ include: (PaneState) -> Bool) -> LayoutNode? {
        switch self {
        case .leaf(let pane): return include(pane) ? self : nil
        case .split(let axis, let first, let second, let ratio):
            let left = first._filterPanes(include)
            let right = second._filterPanes(include)
            switch (left, right) {
            case let (left?, right?): return .split(axis, first: left, second: right, ratio: ratio)
            case let (left?, nil): return left
            case let (nil, right?): return right
            case (nil, nil): return nil
            }
        }
    }

    public func pane(for paneId: String) -> PaneState? {
        switch self {
        case .leaf(let pane):
            pane.id == paneId ? pane : nil
        case .split(_, let first, let second, _):
            first.pane(for: paneId) ?? second.pane(for: paneId)
        }
    }

    public var allPanes: [PaneState] {
        switch self {
        case .leaf(let pane):
            [pane]
        case .split(_, let first, let second, _):
            first.allPanes + second.allPanes
        }
    }

    public var firstPane: PaneState {
        switch self {
        case .leaf(let pane):
            pane
        case .split(_, let first, _, _):
            first.firstPane
        }
    }

    public func splitting(
        _ paneId: String,
        axis: SplitAxis,
        newPane: PaneState,
        ratio: Double = 0.5
    ) -> LayoutNode {
        _replacingLeaf(paneId) { pane in
            .split(
                axis,
                first: .leaf(pane),
                second: .leaf(newPane),
                ratio: ratio.clampedSplitRatio
            )
        }
    }

    public func replacingContent(of paneId: String, with content: PaneContent) -> LayoutNode {
        _replacingLeaf(paneId) { pane in
            .leaf(PaneState(id: pane.id, content: content))
        }
    }

    /// Updates the one split whose opposing subtrees contain the two panes.
    public func updatingRatio(
        between firstPaneId: String,
        and secondPaneId: String,
        ratio: Double
    ) -> LayoutNode {
        switch self {
        case .leaf:
            return self
        case .split(let axis, let first, let second, let currentRatio):
            if first.pane(for: firstPaneId) != nil,
               second.pane(for: secondPaneId) != nil {
                return .split(
                    axis,
                    first: first,
                    second: second,
                    ratio: ratio.clampedSplitRatio
                )
            }
            if second.pane(for: firstPaneId) != nil,
               first.pane(for: secondPaneId) != nil {
                return .split(axis, first: first, second: second, ratio: (1 - ratio).clampedSplitRatio)
            }
            return .split(
                axis,
                first: first.updatingRatio(
                    between: firstPaneId,
                    and: secondPaneId,
                    ratio: ratio
                ),
                second: second.updatingRatio(
                    between: firstPaneId,
                    and: secondPaneId,
                    ratio: ratio
                ),
                ratio: currentRatio
            )
        }
    }

    private func _replacingLeaf(
        _ paneId: String,
        with transform: (PaneState) -> LayoutNode
    ) -> LayoutNode {
        switch self {
        case .leaf(let pane):
            pane.id == paneId ? transform(pane) : self
        case .split(let axis, let first, let second, let ratio):
            .split(
                axis,
                first: first._replacingLeaf(paneId, with: transform),
                second: second._replacingLeaf(paneId, with: transform),
                ratio: ratio
            )
        }
    }

    private enum CodingKeys: String, CodingKey {
        case type, pane, axis, first, second, ratio
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(String.self, forKey: .type) {
        case "leaf":
            self = .leaf(try values.decode(PaneState.self, forKey: .pane))
        case "split":
            self = .split(
                try values.decode(SplitAxis.self, forKey: .axis),
                first: try values.decode(LayoutNode.self, forKey: .first),
                second: try values.decode(LayoutNode.self, forKey: .second),
                ratio: try values.decode(Double.self, forKey: .ratio)
            )
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .type,
                in: values,
                debugDescription: "Unknown layout node type"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .leaf(let pane):
            try values.encode("leaf", forKey: .type)
            try values.encode(pane, forKey: .pane)
        case .split(let axis, let first, let second, let ratio):
            try values.encode("split", forKey: .type)
            try values.encode(axis, forKey: .axis)
            try values.encode(first, forKey: .first)
            try values.encode(second, forKey: .second)
            try values.encode(ratio, forKey: .ratio)
        }
    }
}

private extension Double {
    var clampedSplitRatio: Double { min(max(self, 0.1), 0.9) }
}
