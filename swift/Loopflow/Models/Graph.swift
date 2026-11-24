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
    case waiting
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
    public let error: String?

    public var id: String { lfProcessId }

    enum CodingKeys: String, CodingKey {
        case label, key, iterations, outcome, error
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

/// Contiguous steps at one complete nested loop position. Returning to an earlier
/// position starts another group, so repeated occurrences never disappear.
public struct GraphStepGroup: Identifiable, Sendable {
    public private(set) var steps: [GraphStepProcess]
    public var id: String { steps[0].id }
    public var iterations: [[UInt32]] { steps[0].iterations }

    public var label: String {
        let levels = iterations.enumerated().compactMap { depth, counts -> String? in
            guard !counts.isEmpty else { return nil }
            let passes = counts.enumerated().map { "loop \($0.offset + 1) pass \(UInt64($0.element) + 1)" }
            return "level \(depth + 1): " + passes.joined(separator: ", ")
        }
        return levels.isEmpty ? "Steps" : levels.joined(separator: " · ")
    }

    public static func groups(_ steps: [GraphStepProcess]) -> [Self] {
        var result: [Self] = []
        for step in steps {
            if result.last?.iterations == step.iterations {
                result[result.count - 1].steps.append(step)
            } else {
                result.append(Self(steps: [step]))
            }
        }
        return result
    }
}

public struct TaskRunControl: Codable, Sendable, Hashable {
    public let pausing: Bool
    public let unavailable: String?
}

public struct TaskExecutionSnapshot: Decodable, Sendable, Hashable {
    public let state: TaskExecutionState
    public let reason: String
    public let step: String?
    public let captured: Int64?
    /// Where the run's `lf task run` died before the run stopped.
    public let died: RunDied?
    /// What happens next to a run that died; `reason` already says it.
    public let outlook: SessionOutlook?

    enum CodingKeys: String, CodingKey {
        case state, reason, step, captured, died, outlook
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        state = try values.decode(TaskExecutionState.self, forKey: .state)
        reason = try values.decode(String.self, forKey: .reason)
        step = try values.decodeIfPresent(String.self, forKey: .step)
        captured = try values.decodeIfPresent(Int64.self, forKey: .captured)
        died = try values.decode(RunDied?.self, forKey: .died)
        outlook = try values.decode(SessionOutlook?.self, forKey: .outlook)
    }
}

/// A Task run's carrier died while the run was at a step.
public struct RunDied: Decodable, Sendable, Hashable {
    public let atStep: String
    /// RFC 3339; nil when nobody recorded the death.
    public let at: String?

    enum CodingKeys: String, CodingKey {
        case atStep = "at_step"
        case at
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        atStep = try values.decode(String.self, forKey: .atStep)
        at = try values.decode(String?.self, forKey: .at)
    }
}

/// One autonomous definition, including an invalid local source.
public struct GraphCatalogEntry: Decodable, Sendable, Hashable, Identifiable {
    public let name: String
    /// Its `description:` line; nil when the file has none or does not load.
    public let description: String?
    public let source: String?
    public let compiled: CompiledGraph?
    public let composition: GraphComposition?
    /// The Graph as a Task's workflow line draws it; nil exactly when `compiled` is.
    public let workflow: WorkflowOutline?
    public let unavailable: String?
    /// The Graphs that include this one with `graph:`; empty for a Graph people start with.
    public let includedBy: [String]
    /// What it runs for: a Task, or a Wave's own operation.
    public let runs: GraphSubject
    public var id: String { name }

    enum CodingKeys: String, CodingKey {
        case name, description, source, compiled, composition, workflow, unavailable, runs
        case includedBy = "included_by"
    }

    public init(from decoder: any Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        name = try values.decode(String.self, forKey: .name)
        description = try values.decode(String?.self, forKey: .description)
        source = try values.decode(String?.self, forKey: .source)
        compiled = try values.decode(CompiledGraph?.self, forKey: .compiled)
        composition = try values.decode(GraphComposition?.self, forKey: .composition)
        workflow = try values.decode(WorkflowOutline?.self, forKey: .workflow)
        unavailable = try values.decode(String?.self, forKey: .unavailable)
        includedBy = try values.decode([String].self, forKey: .includedBy)
        runs = try values.decode(GraphSubject.self, forKey: .runs)
    }
}

/// What a Graph is started on.
public enum GraphSubject: String, Decodable, Sendable, Hashable {
    case task, wave
}

/// A Graph as the workflow line shows it before any Task stands on it.
public struct WorkflowOutline: Decodable, Sendable, Hashable {
    /// The Graph's top-level steps in order, as `TaskGraph.steps` carries them.
    public let steps: [TaskGraph.Step]
    /// Every step where a person takes part, XOR paths included.
    public let interactiveSteps: [TaskGraph.InteractiveStep]
    /// "You take part at explore and ship-review." or "Runs on its own."
    public let sentence: String

    enum CodingKeys: String, CodingKey {
        case steps, sentence
        case interactiveSteps = "interactive_steps"
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
