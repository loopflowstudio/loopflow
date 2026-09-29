// A Task's Flow as Rust projects it (`ops/task_flow.rs`, `engine/flow_graph.rs`).
//
// Topology, captured node IDs, cursor position, return counts, and control
// legality all come from the shared read. Clients draw them; they never parse
// Flow YAML or derive which control is legal.

import Foundation

public struct FlowGraph: Decodable, Sendable, Hashable {
    public let name: String
    public let steps: [FlowNode]

    /// Locate an exact captured occurrence, including nested XOR paths.
    public func node(_ key: UInt32) -> FlowNode? {
        func find(_ nodes: [FlowNode]) -> FlowNode? {
            for node in nodes {
                if node.key == key { return node }
                for path in node.paths {
                    if let found = find(path.steps) { return found }
                }
            }
            return nil
        }
        return find(steps)
    }
}

public struct FlowNode: Decodable, Sendable, Hashable {
    /// Captured preorder ID, local to this Flow and including all XOR alternatives.
    public let key: UInt32
    /// Authored occurrence name; never used as graph identity.
    public let id: String?
    /// Literal skill name, operation, or XOR router.
    public let label: String
    public let kind: FlowNodeKind
    public let human: Bool
    public let returnsTo: UInt32?
    /// Composed Flows this occurrence came from, outermost first.
    public let parents: [String]
    public let paths: [FlowGraphPath]

    /// Includes this node and every descendant, without interpreting an ID as a path.
    public func contains(_ key: UInt32) -> Bool {
        self.key == key || paths.contains { $0.steps.contains { $0.contains(key) } }
    }

    enum CodingKeys: String, CodingKey {
        case key, id, label, kind, human, parents, paths
        case returnsTo = "returns_to"
    }
}

public enum FlowNodeKind: String, Decodable, Sendable, Hashable {
    case skill
    case op
    case xor
}

public struct FlowGraphPath: Decodable, Sendable, Hashable {
    public let name: String
    public let description: String
    public let steps: [FlowNode]
}

/// Execution evidence for one authored backward edge in the pinned invocation.
/// The edge itself is the deciding node's `returnsTo`.
public struct FlowReturn: Decodable, Sendable, Hashable {
    /// Key of the deciding occurrence that owns the edge.
    public let decider: UInt32
    public let traversals: UInt32
}

public enum TaskFlowExecution: String, Decodable, Sendable, Hashable {
    case idle
    case starting
    case running
    case stalled
    case human
    case blocked
    case unknown
}

public struct PinnedTaskFlow: Decodable, Sendable, Hashable {
    public let invocationId: String
    public let graph: FlowGraph
    public let current: UInt32?
    /// Occurrences finished in the current pass only.
    public let completed: [UInt32]
    public let returns: [FlowReturn]
    public let iterations: [[UInt32]]
    public let execution: TaskFlowExecution
    public let reason: String
    public let restartRequired: Bool

    enum CodingKeys: String, CodingKey {
        case graph, current, completed, returns, iterations, execution, reason
        case invocationId = "invocation_id"
        case restartRequired = "restart_required"
    }
}

public enum TaskFlowRecord: Decodable, Sendable, Hashable {
    /// No pinned or finished Flow is recorded.
    case none
    case pinned(PinnedTaskFlow)
    /// The Flow finished; its definition was not retained, so nothing is drawn.
    case finished(flow: String)

    private enum Kind: String, Decodable {
        case none, pinned, finished
    }

    private enum CodingKeys: String, CodingKey {
        case kind, flow
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(Kind.self, forKey: .kind) {
        case .none:
            self = .none
        case .pinned:
            self = .pinned(try PinnedTaskFlow(from: decoder))
        case .finished:
            self = .finished(flow: try container.decode(String.self, forKey: .flow))
        }
    }
}

public enum TaskFlowControlKind: String, Decodable, Sendable, Hashable {
    case start
    case resume
    case restart
}

public struct TaskFlowControl: Decodable, Sendable, Hashable {
    public let kind: TaskFlowControlKind
    /// Why the control cannot be used now; `nil` when it may be invoked.
    public let unavailable: String?
}

public struct TaskFlowSnapshot: Decodable, Sendable, Hashable {
    /// The Flow a Start without a selection runs.
    public let recommended: String
    public let record: TaskFlowRecord
    public let controls: [TaskFlowControl]

    public func control(_ kind: TaskFlowControlKind) -> TaskFlowControl? {
        controls.first { $0.kind == kind }
    }
}

/// One selectable Flow and the topology it would pin if started now.
public struct FlowCatalogEntry: Decodable, Sendable, Hashable, Identifiable {
    public let name: String
    public let graph: FlowGraph?
    public let unavailable: String?

    public var id: String { name }
}

/// Formatting only: order and nesting are supplied by Rust's captured definition.
public func flowIterationLabel(_ levels: [[UInt32]]) -> String? {
    guard levels.contains(where: { !$0.isEmpty }) else { return nil }
    return levels.map { "(" + $0.map(String.init).joined(separator: ", ") + ")" }
        .joined(separator: " / ")
}
