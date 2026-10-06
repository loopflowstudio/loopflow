import Foundation

/// Rust owns checkout association; membership grants no execution authority.
public struct TaskWork: Codable, Sendable, Equatable {
    public let sessions: [TaskSession]
    public let flows: [TaskFlowMember]
    public let execs: [Exec]
    /// The Task's Workflow; `nil` when it runs only ad hoc Flows.
    public let workflow: Workflow?
}

/// A Task's Workflow: the definition it took up, where the Task stands on it,
/// and how it got there. `start` and `end` are implicit stages.
public struct Workflow: Codable, Sendable, Equatable {
    public struct Stage: Codable, Sendable, Hashable {
        public let name: String
        public let skill: String
    }

    public struct Edge: Codable, Sendable, Hashable {
        public let from: String
        public let to: String
        public let flow: String?

        /// What `lf task run ISSUE <name>` takes to choose it: its Flow, or
        /// the stage it enters when it runs none.
        public var launchName: String { flow ?? to }
    }

    /// One change to the Task's position.
    public struct Move: Codable, Sendable, Equatable {
        public enum Kind: String, Codable, Sendable {
            case tookUp = "took_up"
            case chose, arrived, set
        }

        public enum Actor: String, Codable, Sendable { case person, conversation, edge }

        public let workflow: String
        public let kind: Kind
        public let from: String
        public let to: String
        public let edge: Int?
        public let execId: String
        public let actor: Actor
        public let sessionId: String?
        public let note: String?
        public let at: Int64

        enum CodingKeys: String, CodingKey {
            case workflow, kind, from, to, edge, actor, note, at
            case execId = "exec_id"
            case sessionId = "session_id"
        }
    }

    /// At a stage the Task waits on a person. On an edge its Flow's driver
    /// Exec carries it; one that no longer runs has stopped and holds the Task.
    public enum Position: Codable, Sendable, Equatable {
        case stage(String)
        case edge(index: Int, execId: String, running: Bool)

        enum CodingKeys: String, CodingKey {
            case kind, stage, edge, running
            case execId = "exec_id"
        }

        public init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            switch try container.decode(String.self, forKey: .kind) {
            case "stage":
                self = .stage(try container.decode(String.self, forKey: .stage))
            case "edge":
                self = .edge(
                    index: try container.decode(Int.self, forKey: .edge),
                    execId: try container.decode(String.self, forKey: .execId),
                    running: try container.decode(Bool.self, forKey: .running))
            case let kind:
                throw DecodingError.dataCorruptedError(
                    forKey: .kind, in: container, debugDescription: "unknown workflow position \(kind)")
            }
        }

        public func encode(to encoder: Encoder) throws {
            var container = encoder.container(keyedBy: CodingKeys.self)
            switch self {
            case .stage(let stage):
                try container.encode("stage", forKey: .kind)
                try container.encode(stage, forKey: .stage)
            case .edge(let index, let execId, let running):
                try container.encode("edge", forKey: .kind)
                try container.encode(index, forKey: .edge)
                try container.encode(execId, forKey: .execId)
                try container.encode(running, forKey: .running)
            }
        }
    }

    public let name: String
    public let stages: [Stage]
    public let edges: [Edge]
    public let position: Position
    /// Edges `lf task run` can choose now, by their place among `edges`.
    public let outgoing: [Int]
    public let history: [Move]

    /// The edge the Task is on, and whether its Flow still runs.
    public var onEdge: (index: Int, edge: Edge, running: Bool)? {
        guard case .edge(let index, _, let running) = position, edges.indices.contains(index) else { return nil }
        return (index, edges[index], running)
    }

    /// Edges a person can choose next, as Rust lists them.
    public var choices: [(index: Int, edge: Edge)] {
        outgoing.filter(edges.indices.contains).map { ($0, edges[$0]) }
    }
}

extension Array where Element == Workflow.Edge {
    /// Edges leaving `stage`, each with its place among the workflow's edges.
    public func leaving(_ stage: String) -> [(index: Int, edge: Workflow.Edge)] {
        enumerated().filter { $0.element.from == stage }.map { ($0.offset, $0.element) }
    }
}

public struct TaskSession: Codable, Sendable, Equatable, Identifiable {
    public let id: String
    public let title: String
    public let interactive: Bool
    /// Driver Exec of the Flow whose step opened the current input.
    public let flowId: String?
    public let completedAt: Int64?

    enum CodingKeys: String, CodingKey {
        case id, title, interactive
        case flowId = "flow_id"
        case completedAt = "completed_at"
    }
}

/// One Flow as its driver Exec records it; `id` is that Exec.
public struct TaskFlowMember: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let name: String
    public let state: TaskFlowState
    public let taskId: String?
    public let waveId: String?
    public let updatedAt: Int64
    public let repo: String?
    public let endedAt: Int64?

    enum CodingKeys: String, CodingKey {
        case id, name, state, repo
        case taskId = "task_id"
        case waveId = "wave_id"
        case updatedAt = "updated_at"
        case endedAt = "ended_at"
    }
}

/// `current` says nothing about a live process; `stopped` exited before the last step.
public enum TaskFlowState: String, Codable, Sendable, Equatable {
    case current, completed, stopped
}
