// Compiled graphs, execution details and catalogs projected by Rust.
//
// Topology, captured node IDs, cursor position, return counts, and control
// legality all come from the shared read. Clients draw them; they never parse
// Graph YAML or derive which control is legal.

import Foundation

public struct CompiledGraph: Decodable, Sendable, Hashable {
    public let name: String
    public let steps: [GraphNode]

    public init(name: String, steps: [GraphNode]) {
        self.name = name; self.steps = steps
    }

    /// Locate an exact captured occurrence, including nested XOR paths.
    public func node(_ key: UInt32) -> GraphNode? {
        func find(_ nodes: [GraphNode]) -> GraphNode? {
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

public struct GraphNode: Decodable, Sendable, Hashable {
    /// Captured preorder ID, local to this Graph and including all XOR alternatives.
    public let key: UInt32
    /// Authored occurrence name; never used as graph identity.
    public let id: String?
    /// Literal skill name, operation, or XOR router.
    public let label: String
    public let kind: GraphNodeKind
    /// An interactive step: a person takes part here.
    public let interactive: Bool
    public let returnsTo: UInt32?
    /// Composed Graphs this occurrence came from, outermost first.
    public let sources: [String]
    public let paths: [CompiledGraphPath]

    /// Includes this node and every descendant, without interpreting an ID as a path.
    public func contains(_ key: UInt32) -> Bool {
        self.key == key || paths.contains { $0.steps.contains { $0.contains(key) } }
    }

    enum CodingKeys: String, CodingKey {
        case key, id, label, kind, interactive, sources, paths
        case returnsTo = "returns_to"
    }
}

public enum GraphNodeKind: String, Decodable, Sendable, Hashable {
    case skill
    case op
    case xor
}

public struct CompiledGraphPath: Decodable, Sendable, Hashable {
    public let name: String
    public let description: String
    public let steps: [GraphNode]
}

/// Execution evidence for one authored backward edge in the latest invocation.
/// The edge itself is the deciding node's `returnsTo`.
public struct GraphReturn: Decodable, Sendable, Hashable {
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

/// One launched step and how its process ended.
public struct GraphStepProcess: Decodable, Sendable, Hashable, Identifiable {
    public let lfProcessId: String
    public let label: String
    public let key: UInt32
    public let iterations: [[UInt32]]
    public let startedAt: Int64
    public let completedAt: Int64?
    public let outcome: String?
    public let exitCode: Int32?

    public var id: String { lfProcessId }

    enum CodingKeys: String, CodingKey {
        case label, key, iterations, outcome
        case lfProcessId = "lf_process_id"
        case startedAt = "started_at"
        case completedAt = "completed_at"
        case exitCode = "exit_code"
    }
}

/// One Graph process as it recorded itself: the graph captured at launch and
/// every step it started. Any Graph reads the same way, ad hoc or a Task's edge.
public struct GraphProcessDetail: Decodable, Sendable, Hashable {
    public let entry: GraphProcessInventoryEntry
    public let compiled: CompiledGraph
    public let current: UInt32?
    public let completed: [UInt32]
    public let returns: [GraphReturn]
    public let iterations: [[UInt32]]
    public let cwd: String?
    public let steps: [GraphStepProcess]

    /// Labels derived from recorded exits; these do not establish process liveness.
    public var presentation: (execution: TaskExecutionState, reason: String) {
        let last = steps.last
        switch entry.state {
        case .current:
            return (.running, last.map { "Running \($0.label)" } ?? "Starting")
        case .completed:
            return (.idle, "Completed")
        case .stopped:
            guard let last, let outcome = last.outcome, outcome != "succeeded" else {
                return (.idle, "Stopped before its last step")
            }
            return (.blocked, "\(last.label) · \(outcome)")
        }
    }
}

public struct TaskRunControl: Codable, Sendable, Hashable {
    public let unavailable: String?
}

public struct TaskExecutionSnapshot: Decodable, Sendable, Hashable {
    public let state: TaskExecutionState
    public let reason: String
    public let step: String?
    public let captured: Int64?
}

/// One autonomous definition, including an invalid local source.
public struct GraphCatalogEntry: Decodable, Sendable, Hashable, Identifiable {
    public let name: String
    public let source: String?
    public let compiled: CompiledGraph?
    public let composition: GraphComposition?
    public let unavailable: String?
    public var id: String { name }
}

/// A graph as a Workflow: `start`, its interactive steps in order, `end`, and
/// the runs between them. Not a wire type; derived from what Rust sends.
public struct WorkflowDefinition: Sendable, Hashable {
    /// What a run from `from` carries the Task to; `name` is the step it runs first.
    public struct Run: Sendable, Hashable {
        public let from: String
        public let to: String
        public let name: String
    }

    public let name: String
    public let steps: [TaskGraph.InteractiveStep]
    public let runs: [Run]

    /// A Task's captured graph: each run carries it to the next interactive step.
    public init(_ graph: TaskGraph) {
        name = graph.name
        steps = graph.interactiveSteps
        let columns = [TaskGraph.start] + steps.map(\.name) + [TaskGraph.end]
        runs = zip(columns, columns.dropFirst()).map { Run(from: $0, to: $1, name: "") }
    }

    /// A definition: interactive steps in order, each XOR path's in name order,
    /// and a run back wherever a step returns to an earlier one.
    public init(_ compiled: CompiledGraph) {
        var flat: [GraphNode] = []
        func walk(_ nodes: [GraphNode]) {
            for node in nodes {
                flat.append(node)
                for path in node.paths.sorted(by: { $0.name < $1.name }) { walk(path.steps) }
            }
        }
        walk(compiled.steps)
        var steps: [TaskGraph.InteractiveStep] = []
        // The column each node runs in: 0 after `start`, n after the nth interactive step.
        var segment: [UInt32: Int] = [:]
        var first: [Int: String] = [:]
        for node in flat {
            if node.interactive {
                steps.append(TaskGraph.InteractiveStep(name: node.id ?? node.label, skill: node.label, description: nil))
            } else if first[steps.count] == nil {
                first[steps.count] = node.label
            }
            segment[node.key] = steps.count
        }
        let columns = [TaskGraph.start] + steps.map(\.name) + [TaskGraph.end]
        var runs = zip(columns.indices, columns.indices.dropFirst()).map {
            Run(from: columns[$0], to: columns[$1], name: first[$0] ?? "")
        }
        for node in flat {
            guard let back = node.returnsTo, let from = segment[node.key],
                  let target = flat.first(where: { $0.key == back }), let column = segment[target.key] else { continue }
            // A return into an interactive step arrives there; into any other, at the next one.
            let to = target.interactive ? column : column + 1
            let run = Run(from: columns[from], to: columns[to], name: node.label)
            if to != from + 1, !runs.contains(run) { runs.append(run) }
        }
        name = compiled.name
        self.steps = steps
        self.runs = runs
    }
}

/// A catalog graph as a Workflow, keeping an invalid source editable.
public struct WorkflowCatalogEntry: Sendable, Hashable, Identifiable {
    public let name: String
    public let source: String?
    public let workflow: WorkflowDefinition?
    public let unavailable: String?
    public var id: String { name }

    public init(_ entry: GraphCatalogEntry) {
        name = entry.name
        source = entry.source
        workflow = entry.compiled.map(WorkflowDefinition.init)
        unavailable = entry.unavailable
    }
}

/// Formatting only: order and nesting are supplied by Rust's captured definition.
public func graphIterationLabel(_ levels: [[UInt32]]) -> String? {
    let counts = levels.flatMap { $0 }
    let passes = counts.enumerated().filter { $0.element > 0 }.map { index, count in
        counts.count == 1 ? "pass \(UInt64(count) + 1)" : "loop \(index + 1) pass \(UInt64(count) + 1)"
    }
    return passes.isEmpty ? nil : passes.joined(separator: ", ")
}

/// Template-local disclosure IDs never identify execution or an invocation.
public struct GraphComposition: Decodable, Sendable, Hashable {
    public let revision: String
    public let items: [GraphCompositionItem]
}

public indirect enum GraphCompositionItem: Decodable, Sendable, Hashable, Identifiable {
    case node(key: UInt32, paths: [String: [GraphCompositionItem]])
    case group(id: String, name: String, items: [GraphCompositionItem])

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
                         paths: try value.decode([String: [GraphCompositionItem]].self, forKey: .paths))
        case .group:
            self = .group(id: try value.decode(String.self, forKey: .id),
                          name: try value.decode(String.self, forKey: .name),
                          items: try value.decode([GraphCompositionItem].self, forKey: .items))
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
public struct GraphCompositionProjection {
    public let compiled: CompiledGraph
    public let visibleKeys: [UInt32: UInt32]
    public let groups: [UInt32: String]

    public init(compiled: CompiledGraph, items: [GraphCompositionItem], expanded: Set<String>) {
        var visible: [UInt32: UInt32] = [:]
        var groups: [UInt32: String] = [:]
        var sourceNodes: [UInt32: GraphNode] = [:]
        func index(_ nodes: [GraphNode]) {
            for node in nodes {
                sourceNodes[node.key] = node
                for path in node.paths { index(path.steps) }
            }
        }
        index(compiled.steps)
        var next = (sourceNodes.keys.max() ?? 0) + 1
        func nodes(_ items: [GraphCompositionItem]) -> [GraphNode] {
            items.flatMap { item -> [GraphNode] in
                switch item {
                case .group(let id, let name, let children):
                    if expanded.contains(id) { return nodes(children) }
                    let groupKey = next
                    next += 1
                    groups[groupKey] = id
                    let keys = item.nodeKeys
                    for key in keys { visible[key] = groupKey }
                    return [GraphNode(key: groupKey, id: nil, label: "▸ \(name) · \(keys.count)",
                                     kind: .op, interactive: false, returnsTo: nil, sources: [], paths: [])]
                case .node(let key, let paths):
                    guard let node = sourceNodes[key] else { return [] }
                    visible[key] = key
                    return [GraphNode(key: key, id: node.id, label: node.label, kind: node.kind,
                                     interactive: node.interactive, returnsTo: node.returnsTo, sources: node.sources,
                                     paths: node.paths.map { path in
                        CompiledGraphPath(name: path.name, description: path.description,
                                      steps: nodes(paths[path.name] ?? []))
                    })]
                }
            }
        }
        self.compiled = CompiledGraph(name: compiled.name, steps: nodes(items))
        visibleKeys = visible
        self.groups = groups
    }
}
