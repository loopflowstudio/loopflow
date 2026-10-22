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
    case session(id: String, location: WorkspaceIdentity? = nil, execution: TaskLocationObservation? = nil)
    /// Unknown locations remain unavailable, never a local-shell fallback.
    case shell(tabs: [ShellTab], selected: String)
    /// Every Graph process of a Task.
    case graphLog(taskId: String, location: WorkspaceIdentity? = nil, execution: TaskLocationObservation? = nil)
    /// A Task's worktree files.
    case files(taskId: String, location: WorkspaceIdentity? = nil, execution: TaskLocationObservation? = nil)

    var canonicalized: PaneContent {
        switch self {
        case .session(let id, let location, let execution):
            .session(id: StructuredIdentity.session(id), location: location, execution: execution)
        case .files(let id, let location, let execution):
            .files(taskId: StructuredIdentity.prefixed(id, "task_"), location: location, execution: execution)
        case .graphLog(let id, let location, let execution):
            .graphLog(taskId: StructuredIdentity.prefixed(id, "task_"), location: location, execution: execution)
        case .empty, .shell: self
        }
    }

    public static func shell(location: WorkspaceIdentity?, execution: TaskLocationObservation? = nil) -> Self {
        let tab = ShellTab(location: location, execution: execution)
        return .shell(tabs: [tab], selected: tab.id)
    }

    public var selectedShell: ShellTab? {
        guard case .shell(let tabs, let selected) = self else { return nil }
        return tabs.first { $0.id == selected }
    }

    /// Terminals retain their captured location; Files and logs follow their
    /// recorded owner. Unavailable owner evidence never falls back to a path.
    public var location: WorkspaceIdentity? {
        switch self {
        case .empty: return nil
        case .session(_, let location, _): return location
        case .shell: return selectedShell?.location
        case .files(_, let location, let execution), .graphLog(_, let location, let execution):
            guard let execution else { return location }
            guard case .recorded(_, let checkout?) = execution.location else { return nil }
            return WorkspaceIdentity(machineId: execution.machineID, worktree: checkout)
        }
    }

    /// The exact observed owner travels with the content, never the enclosing workspace.
    public var executionLocation: TaskLocationObservation? {
        switch self {
        case .shell: return selectedShell?.execution
        case .session(_, _, let execution),
             .files(_, _, let execution), .graphLog(_, _, let execution): return execution
        case .empty: return nil
        }
    }
}

/// Runtime shell state. Its identity and incarnation survive tab selection and pane moves.
public struct ShellTab: Codable, Sendable, Identifiable, Equatable {
    public let id: String
    public let incarnation: String
    public let location: WorkspaceIdentity?
    public let execution: TaskLocationObservation?
    public let command: [String]?
    public let lfProcessID: String?
    public init(id: String = UUID().uuidString, location: WorkspaceIdentity?, execution: TaskLocationObservation? = nil, command: [String]? = [], lfProcessID: String? = nil) {
        self.id = id; incarnation = UUID().uuidString; self.location = location
        self.execution = execution; self.command = command; self.lfProcessID = lfProcessID
    }
}

public struct PaneState: Codable, Sendable, Identifiable, Equatable {
    public let id: String
    public let content: PaneContent
    /// The content occurrence, distinct from the reusable pane slot.
    public let incarnation: String
    public var sessionId: String? {
        guard case .session(let id, _, _) = content else { return nil }
        return id
    }

    public var executionLocation: TaskLocationObservation? { content.executionLocation }

    fileprivate init(id: String, content: PaneContent, incarnation: String) {
        self.id = id; self.content = content; self.incarnation = incarnation
    }

    public init(
        id: String = UUID().uuidString,
        content: PaneContent
    ) {
        self.id = id
        self.content = content.canonicalized
        self.incarnation = UUID().uuidString
    }
}

public indirect enum LayoutNode: Codable, Sendable, Equatable {
    public static let goldenRatio = 0.618

    case leaf(PaneState)
    case split(SplitAxis, first: LayoutNode, second: LayoutNode, ratio: Double)

    public static func defaultLayout() -> LayoutNode {
        .leaf(PaneState(content: .empty))
    }

    public func visible(excluding collapsed: Set<String>) -> LayoutNode? {
        removing(collapsed)
    }

    /// Prune leaves in one traversal, preserving surviving occurrences and ratios.
    /// Removing every leaf returns nil; the store owns the empty-workspace choice.
    public func removing(_ paneIds: Set<String>) -> LayoutNode? {
        switch self {
        case .leaf(let pane): return paneIds.contains(pane.id) ? nil : self
        case .split(let axis, let first, let second, let ratio):
            let left = first.removing(paneIds)
            let right = second.removing(paneIds)
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

    public func updatingShellTabs(of paneId: String, tabs: [ShellTab], selected: String) -> LayoutNode {
        _replacingLeaf(paneId) { pane in
            .leaf(PaneState(id: pane.id, content: .shell(tabs: tabs, selected: selected), incarnation: pane.incarnation))
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

extension PaneState {
    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decode(String.self, forKey: .id)
        incarnation = try values.decode(String.self, forKey: .incarnation)
        content = try values.decode(PaneContent.self, forKey: .content).canonicalized
    }
}
