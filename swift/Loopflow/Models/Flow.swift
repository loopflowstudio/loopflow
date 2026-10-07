// Flow graphs, execution details and catalogs projected by Rust.
//
// Topology, captured node IDs, cursor position, return counts, and control
// legality all come from the shared read. Clients draw them; they never parse
// Flow YAML or derive which control is legal.

import Foundation

public struct FlowGraph: Decodable, Sendable, Hashable {
    public let name: String
    public let steps: [FlowNode]
    public let interactions: InteractionGraph

    public init(name: String, steps: [FlowNode], interactions: InteractionGraph) {
        self.name = name; self.steps = steps; self.interactions = interactions
    }

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

public struct InteractionGraph: Decodable, Sendable, Hashable {
    public let stages: [String]
    public let transitions: [InteractionTransition]
}

public struct InteractionTransition: Decodable, Sendable, Hashable {
    public let from: String
    public let to: String
    public let nodes: [String]
    public let routes: [InteractionRoute]
}

public struct InteractionRoute: Decodable, Sendable, Hashable {
    public let from: String
    public let to: String
    public let condition: String?
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
    public let sources: [String]
    public let paths: [FlowGraphPath]

    /// Includes this node and every descendant, without interpreting an ID as a path.
    public func contains(_ key: UInt32) -> Bool {
        self.key == key || paths.contains { $0.steps.contains { $0.contains(key) } }
    }

    enum CodingKeys: String, CodingKey {
        case key, id, label, kind, human, sources, paths
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

/// Execution evidence for one authored backward edge in the latest invocation.
/// The edge itself is the deciding node's `returnsTo`.
public struct FlowReturn: Decodable, Sendable, Hashable {
    /// Key of the deciding occurrence that owns the edge.
    public let decider: UInt32
    public let traversals: UInt32
}

public enum TaskExecutionState: String, Decodable, Sendable, Hashable {
    case idle
    case starting
    case running
    case stalled
    case blocked
    case unknown
}

/// Diagram presentation derived from one execution detail; no separate wire record.
public struct FlowProcessProgress: Sendable, Hashable {
    public let flowProcessLfid: String
    public let graph: FlowGraph
    public let current: UInt32?
    /// Occurrences finished in the current pass only.
    public let completed: [UInt32]
    public let returns: [FlowReturn]
    public let iterations: [[UInt32]]
    public let execution: TaskExecutionState
    public let reason: String


}

/// One launched step and how its process ended.
public struct FlowStepProcess: Decodable, Sendable, Hashable, Identifiable {
    public let processLfid: String
    public let label: String
    public let key: UInt32
    public let iterations: [[UInt32]]
    public let startedAt: Int64
    public let completedAt: Int64?
    public let outcome: String?
    public let exitCode: Int32?

    public var id: String { processLfid }

    enum CodingKeys: String, CodingKey {
        case label, key, iterations, outcome
        case processLfid = "process_lfid"
        case startedAt = "started_at"
        case completedAt = "completed_at"
        case exitCode = "exit_code"
    }
}

/// One Flow process as its driver recorded it: the graph captured at launch and
/// every step it started. Any Flow reads the same way, ad hoc or a Task's edge.
public struct FlowProcessDetail: Decodable, Sendable, Hashable {
    public let entry: FlowProcessInventoryEntry
    public let graph: FlowGraph
    public let current: UInt32?
    public let completed: [UInt32]
    public let returns: [FlowReturn]
    public let iterations: [[UInt32]]
    public let cwd: String?
    public let steps: [FlowStepProcess]

    /// The run in the shape the diagram draws. `current` records no driver
    /// exit, which is not proof of a live process.
    public var progress: FlowProcessProgress {
        let last = steps.last
        let execution: TaskExecutionState
        let reason: String
        switch entry.state {
        case .current:
            execution = .running
            reason = last.map { "Running \($0.label)" } ?? "Starting"
        case .completed:
            execution = .idle
            reason = "Completed"
        case .stopped:
            let failed = last.flatMap { step in step.outcome.flatMap { $0 == "ok" ? nil : "\(step.label) · \($0)" } }
            execution = failed == nil ? .idle : .blocked
            reason = failed ?? "Stopped before its last step"
        }
        return FlowProcessProgress(flowProcessLfid: entry.id, graph: graph, current: current, completed: completed,
                              returns: returns, iterations: iterations, execution: execution, reason: reason)
    }
}

public struct TaskRunControl: Decodable, Sendable, Hashable {
    public let unavailable: String?
}

public struct TaskExecutionSnapshot: Decodable, Sendable, Hashable {
    public let state: TaskExecutionState
    public let reason: String
    public let step: String?
    public let captured: Int64?
}

/// One autonomous definition, including an invalid local source.
public struct FlowCatalogEntry: Decodable, Sendable, Hashable, Identifiable {
    public let name: String
    public let source: String?
    public let graph: FlowGraph?
    public let composition: FlowComposition?
    public let unavailable: String?
    public var id: String { name }
}

public struct WorkflowCatalogEntry: Decodable, Sendable, Hashable, Identifiable {
    public let name: String
    public let source: String?
    public let workflow: WorkflowDefinition?
    public let unavailable: String?
    public var id: String { name }
}

/// An authored workflow before any Task has taken it up.
public struct WorkflowDefinition: Decodable, Sendable, Hashable {
    public let name: String
    public let nodes: [Workflow.Node]
    public let edges: [Workflow.Edge]
}

/// Formatting only: order and nesting are supplied by Rust's captured definition.
public func flowIterationLabel(_ levels: [[UInt32]]) -> String? {
    let counts = levels.flatMap { $0 }
    let passes = counts.enumerated().filter { $0.element > 0 }.map { index, count in
        counts.count == 1 ? "pass \(UInt64(count) + 1)" : "loop \(index + 1) pass \(UInt64(count) + 1)"
    }
    return passes.isEmpty ? nil : passes.joined(separator: ", ")
}

/// Template-local disclosure IDs never identify execution or an invocation.
public struct FlowComposition: Decodable, Sendable, Hashable {
    public let revision: String
    public let items: [FlowCompositionItem]
}

public indirect enum FlowCompositionItem: Decodable, Sendable, Hashable, Identifiable {
    case node(key: UInt32, paths: [String: [FlowCompositionItem]])
    case group(id: String, name: String, items: [FlowCompositionItem])

    public var id: String {
        switch self {
        case .node(let key, _): "node-\(key)"
        case .group(let id, _, _): id
        }
    }

    private enum CodingKeys: String, CodingKey { case kind, key, paths, id, name, items }
    private enum Kind: String, Decodable { case node, group }
    public init(from decoder: Decoder) throws {
        let value = try decoder.container(keyedBy: CodingKeys.self)
        switch try value.decode(Kind.self, forKey: .kind) {
        case .node:
            self = .node(key: try value.decode(UInt32.self, forKey: .key),
                         paths: try value.decode([String: [FlowCompositionItem]].self, forKey: .paths))
        case .group:
            self = .group(id: try value.decode(String.self, forKey: .id),
                          name: try value.decode(String.self, forKey: .name),
                          items: try value.decode([FlowCompositionItem].self, forKey: .items))
        }
    }

    public var nodeKeys: [UInt32] {
        switch self {
        case .node(let key, let paths): [key] + paths.keys.sorted().flatMap { paths[$0]!.flatMap(\.nodeKeys) }
        case .group(_, _, let items): items.flatMap(\.nodeKeys)
        }
    }
}

/// Presentation maps hidden endpoints onto their visible composition boundary.
public struct FlowCompositionProjection {
    public let graph: FlowGraph
    public let visibleKeys: [UInt32: UInt32]
    public let groups: [UInt32: String]

    public init(graph: FlowGraph, items: [FlowCompositionItem], expanded: Set<String>) {
        var visible: [UInt32: UInt32] = [:]
        var groups: [UInt32: String] = [:]
        var sourceNodes: [UInt32: FlowNode] = [:]
        func index(_ nodes: [FlowNode]) {
            for node in nodes {
                sourceNodes[node.key] = node
                for path in node.paths { index(path.steps) }
            }
        }
        index(graph.steps)
        var next = (sourceNodes.keys.max() ?? 0) + 1
        func nodes(_ items: [FlowCompositionItem]) -> [FlowNode] {
            items.flatMap { item -> [FlowNode] in
                switch item {
                case .group(let id, let name, let children):
                    if expanded.contains(id) { return nodes(children) }
                    let groupKey = next
                    next += 1
                    groups[groupKey] = id
                    let keys = item.nodeKeys
                    for key in keys { visible[key] = groupKey }
                    return [FlowNode(key: groupKey, id: nil, label: "▸ \(name) · \(keys.count)",
                                     kind: .op, human: false, returnsTo: nil, sources: [], paths: [])]
                case .node(let key, let paths):
                    guard let node = sourceNodes[key] else { return [] }
                    visible[key] = key
                    return [FlowNode(key: key, id: node.id, label: node.label, kind: node.kind,
                                     human: node.human, returnsTo: node.returnsTo, sources: node.sources,
                                     paths: node.paths.map { path in
                        FlowGraphPath(name: path.name, description: path.description,
                                      steps: nodes(paths[path.name] ?? []))
                    })]
                }
            }
        }
        self.graph = FlowGraph(name: graph.name, steps: nodes(items), interactions: graph.interactions)
        visibleKeys = visible
        self.groups = groups
    }
}
