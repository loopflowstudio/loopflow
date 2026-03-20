// Recursive binary split tree for the wave multiplexer.
// Every leaf is a pane (shell, markdown viewer, diff viewer, or launchpad).

import Foundation

public enum SplitAxis: String, Codable, Sendable {
    case horizontal
    case vertical
}

public enum PaneType: String, Codable, Sendable {
    case terminal
    case markdown
    case diff
    case launchpad
}

public struct PaneConfig: Codable, Sendable, Equatable {
    public var filePath: String?
    public var terminalSessionName: String?
    public var launchCommand: String?

    public init(
        filePath: String? = nil,
        terminalSessionName: String? = nil,
        launchCommand: String? = nil
    ) {
        self.filePath = filePath
        self.terminalSessionName = terminalSessionName
        self.launchCommand = launchCommand
    }

    public static let empty = PaneConfig()
}

public struct PaneState: Codable, Sendable, Identifiable, Equatable {
    public let id: String
    public let type: PaneType
    public var config: PaneConfig

    public init(id: String = UUID().uuidString, type: PaneType, config: PaneConfig = .empty) {
        self.id = id
        self.type = type
        self.config = config
    }
}

public indirect enum LayoutNode: Codable, Sendable, Equatable {
    case leaf(PaneState)
    case split(SplitAxis, first: LayoutNode, second: LayoutNode, ratio: Double)

    public var id: String {
        switch self {
        case .leaf(let pane): pane.id
        case .split(_, let first, _, _): "split-\(first.id)"
        }
    }

    public static func defaultLayout() -> LayoutNode {
        .leaf(PaneState(type: .terminal))
    }

    // MARK: - Tree operations

    public func pane(for paneId: String) -> PaneState? {
        switch self {
        case .leaf(let pane):
            return pane.id == paneId ? pane : nil
        case .split(_, let first, let second, _):
            return first.pane(for: paneId) ?? second.pane(for: paneId)
        }
    }

    public var allPanes: [PaneState] {
        switch self {
        case .leaf(let pane):
            return [pane]
        case .split(_, let first, let second, _):
            return first.allPanes + second.allPanes
        }
    }

    public var firstPane: PaneState? {
        switch self {
        case .leaf(let pane): pane
        case .split(_, let first, _, _): first.firstPane
        }
    }

    private func containsPane(_ paneId: String) -> Bool {
        pane(for: paneId) != nil
    }

    private func isLeaf(_ paneId: String) -> Bool {
        if case .leaf(let pane) = self {
            return pane.id == paneId
        }
        return false
    }

    /// Replace the leaf with the given paneId with a split containing the original and a new pane.
    public func splitting(_ paneId: String, axis: SplitAxis, newPane: PaneState, ratio: Double = 0.5) -> LayoutNode {
        switch self {
        case .leaf(let pane):
            guard pane.id == paneId else { return self }
            return .split(axis, first: .leaf(pane), second: .leaf(newPane), ratio: ratio)
        case .split(let ax, let first, let second, let r):
            return .split(ax, first: first.splitting(paneId, axis: axis, newPane: newPane, ratio: ratio),
                          second: second.splitting(paneId, axis: axis, newPane: newPane, ratio: ratio),
                          ratio: r)
        }
    }

    /// Remove a pane by ID. Returns nil if this was the only pane.
    public func removing(_ paneId: String) -> LayoutNode? {
        switch self {
        case .leaf(let pane):
            return pane.id == paneId ? nil : self
        case .split(let axis, let first, let second, let ratio):
            let newFirst = first.removing(paneId)
            let newSecond = second.removing(paneId)
            switch (newFirst, newSecond) {
            case (nil, nil): return nil
            case (nil, let remaining?): return remaining
            case (let remaining?, nil): return remaining
            case (let f?, let s?): return .split(axis, first: f, second: s, ratio: ratio)
            }
        }
    }

    /// Update the pane config for a specific pane.
    public func updatingPane(_ paneId: String, config: PaneConfig) -> LayoutNode {
        switch self {
        case .leaf(var pane):
            guard pane.id == paneId else { return self }
            pane.config = config
            return .leaf(pane)
        case .split(let axis, let first, let second, let ratio):
            return .split(axis, first: first.updatingPane(paneId, config: config),
                          second: second.updatingPane(paneId, config: config),
                          ratio: ratio)
        }
    }

    /// Replace a leaf pane while preserving the surrounding tree shape.
    public func replacingPane(_ paneId: String, with pane: PaneState) -> LayoutNode {
        switch self {
        case .leaf(let current):
            guard current.id == paneId else { return self }
            return .leaf(pane)
        case .split(let axis, let first, let second, let ratio):
            return .split(
                axis,
                first: first.replacingPane(paneId, with: pane),
                second: second.replacingPane(paneId, with: pane),
                ratio: ratio
            )
        }
    }

    /// Update the split ratio for the split that directly contains a pane.
    public func updatingRatio(containing paneId: String, ratio: Double) -> LayoutNode {
        switch self {
        case .leaf:
            return self
        case .split(let axis, let first, let second, let currentRatio):
            if first.isLeaf(paneId) || second.isLeaf(paneId) {
                return .split(axis, first: first, second: second, ratio: ratio)
            }
            if first.containsPane(paneId) {
                return .split(axis,
                              first: first.updatingRatio(containing: paneId, ratio: ratio),
                              second: second,
                              ratio: currentRatio)
            }
            if second.containsPane(paneId) {
                return .split(axis,
                              first: first,
                              second: second.updatingRatio(containing: paneId, ratio: ratio),
                              ratio: currentRatio)
            }
            return self
        }
    }

    /// Find the next pane after the given paneId in tree order.
    public func nextPane(after paneId: String) -> PaneState? {
        adjacentPane(to: paneId, offset: 1)
    }

    /// Find the previous pane before the given paneId in tree order.
    public func previousPane(before paneId: String) -> PaneState? {
        adjacentPane(to: paneId, offset: -1)
    }

    private func adjacentPane(to paneId: String, offset: Int) -> PaneState? {
        let panes = allPanes
        guard let index = panes.firstIndex(where: { $0.id == paneId }) else { return nil }
        let neighborIndex = (index + offset + panes.count) % panes.count
        return panes[neighborIndex]
    }

    // MARK: - Codable

    private enum CodingKeys: String, CodingKey {
        case type, pane, axis, first, second, ratio
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "leaf":
            let pane = try container.decode(PaneState.self, forKey: .pane)
            self = .leaf(pane)
        case "split":
            let axis = try container.decode(SplitAxis.self, forKey: .axis)
            let first = try container.decode(LayoutNode.self, forKey: .first)
            let second = try container.decode(LayoutNode.self, forKey: .second)
            let ratio = try container.decode(Double.self, forKey: .ratio)
            self = .split(axis, first: first, second: second, ratio: ratio)
        default:
            throw DecodingError.dataCorruptedError(forKey: .type, in: container, debugDescription: "Unknown layout node type: \(type)")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .leaf(let pane):
            try container.encode("leaf", forKey: .type)
            try container.encode(pane, forKey: .pane)
        case .split(let axis, let first, let second, let ratio):
            try container.encode("split", forKey: .type)
            try container.encode(axis, forKey: .axis)
            try container.encode(first, forKey: .first)
            try container.encode(second, forKey: .second)
            try container.encode(ratio, forKey: .ratio)
        }
    }
}
