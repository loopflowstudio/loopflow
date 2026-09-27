// A Task's Flow as Rust projects it (`ops/task_flow.rs`, `engine/flow_graph.rs`).
//
// Topology, occurrence keys, cursor position, return counts, and control
// legality all come from the shared read. Clients draw them; they never parse
// Flow YAML or derive which control is legal.

import Foundation

public struct FlowGraph: Decodable, Sendable, Hashable {
    public let name: String
    public let steps: [FlowNode]

    public init(name: String, steps: [FlowNode]) { self.name = name; self.steps = steps }

    /// Locate an exact structural occurrence, including nested XOR paths.
    public func node(_ key: String) -> FlowNode? {
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

public struct FlowNode: Decodable, Sendable, Hashable, Identifiable {
    /// Structural occurrence key (`3`, `6/fix/0`), unique within the definition.
    public let key: String
    public let id: String?
    /// Literal skill name, operation, or XOR router.
    public let label: String
    public let kind: FlowNodeKind
    public let human: Bool
    public let returnsTo: String?
    /// Composed Flows this occurrence came from, outermost first.
    public let parents: [String]
    public let paths: [FlowGraphPath]

    public var identifier: String { key }

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
    public let decider: String
    public let traversals: UInt32
}

public enum TaskFlowExecution: String, Decodable, Sendable, Hashable {
    case idle
    case starting
    case running
    case human
    case blocked
    case unknown
}

public struct PinnedTaskFlow: Decodable, Sendable, Hashable {
    public let invocationId: String
    public let graph: FlowGraph
    public let current: String?
    /// Occurrences finished in the current pass only.
    public let completed: [String]
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
    public let template: FlowTemplate?
    public let unavailable: String?

    public var id: String { name }
}

/// Formatting only: order and nesting are supplied by Rust's captured definition.
public func flowIterationLabel(_ levels: [[UInt32]]) -> String? {
    guard levels.contains(where: { !$0.isEmpty }) else { return nil }
    return levels.map { "(" + $0.map(String.init).joined(separator: ", ") + ")" }
        .joined(separator: " / ")
}

/// Template-local disclosure IDs never identify execution or an invocation.
public struct FlowTemplate: Decodable, Sendable, Hashable {
    public let revision: String
    public let items: [FlowTemplateItem]
}

public indirect enum FlowTemplateItem: Decodable, Sendable, Hashable, Identifiable {
    case node(key: String, paths: [String: [FlowTemplateItem]])
    case group(id: String, name: String, items: [FlowTemplateItem])

    public var id: String {
        switch self {
        case .node(let key, _): key
        case .group(let id, _, _): id
        }
    }

    private enum CodingKeys: String, CodingKey { case kind, key, paths, id, name, items }
    private enum Kind: String, Decodable { case node, group }
    public init(from decoder: Decoder) throws {
        let value = try decoder.container(keyedBy: CodingKeys.self)
        switch try value.decode(Kind.self, forKey: .kind) {
        case .node:
            self = .node(key: try value.decode(String.self, forKey: .key),
                         paths: try value.decode([String: [FlowTemplateItem]].self, forKey: .paths))
        case .group:
            self = .group(id: try value.decode(String.self, forKey: .id),
                          name: try value.decode(String.self, forKey: .name),
                          items: try value.decode([FlowTemplateItem].self, forKey: .items))
        }
    }

    public var groupIDs: Set<String> {
        switch self {
        case .node(_, let paths): Set(paths.values.flatMap { $0.flatMap(\.groupIDs) })
        case .group(let id, _, let items): Set(items.flatMap(\.groupIDs)).union([id])
        }
    }

    public var nodeKeys: [String] {
        switch self {
        case .node(let key, let paths): [key] + paths.keys.sorted().flatMap { paths[$0]!.flatMap(\.nodeKeys) }
        case .group(_, _, let items): items.flatMap(\.nodeKeys)
        }
    }
}

/// Presentation maps hidden endpoints onto their visible composition boundary.
public struct FlowTemplateProjection {
    public let graph: FlowGraph
    public let visibleKeys: [String: String]

    public init(graph: FlowGraph, items: [FlowTemplateItem], expanded: Set<String>) {
        var visible: [String: String] = [:]
        func nodes(_ items: [FlowTemplateItem]) -> [FlowNode] {
            items.flatMap { item -> [FlowNode] in
                switch item {
                case .group(let id, let name, let children):
                    if expanded.contains(id) { return nodes(children) }
                    for key in item.nodeKeys { visible[key] = id }
                    return [FlowNode(key: id, id: nil, label: "▸ \(name) · \(item.nodeKeys.count)",
                                     kind: .op, human: false, returnsTo: nil, parents: [], paths: [])]
                case .node(let key, let paths):
                    guard let node = graph.node(key) else { return [] }
                    visible[key] = key
                    return [FlowNode(key: key, id: node.id, label: node.label, kind: node.kind,
                                     human: node.human, returnsTo: node.returnsTo, parents: node.parents,
                                     paths: node.paths.map { path in
                        FlowGraphPath(name: path.name, description: path.description,
                                      steps: nodes(paths[path.name] ?? []))
                    })]
                }
            }
        }
        self.graph = FlowGraph(name: graph.name, steps: nodes(items))
        visibleKeys = visible
    }
}
