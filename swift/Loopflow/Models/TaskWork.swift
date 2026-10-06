import Foundation

/// Rust owns checkout association; membership grants no execution authority.
public struct TaskWork: Codable, Sendable, Equatable {
    public let sessions: [TaskSession]
    public let flows: [TaskFlowMember]
    public let execs: [Exec]
    /// The Task's workflow and position; `nil` when it runs only ad hoc Flows.
    public let workflow: TaskWorkflow?
}

/// Conversation stages joined by edges that are Flows. `start` and `end` are implicit.
public struct TaskWorkflow: Codable, Sendable, Equatable {
    public struct Stage: Codable, Sendable, Equatable {
        public let name: String
        public let skill: String
    }

    public struct Edge: Codable, Sendable, Equatable {
        public let from: String
        public let to: String
        public let flow: String?

        /// What `lf task run ISSUE <name>` takes to traverse it: its Flow, or
        /// the stage it enters when it runs nothing.
        public var launchName: String { flow ?? to }
    }

    public struct Traversal: Codable, Sendable, Equatable {
        public let edge: Int
        public let execId: String

        enum CodingKeys: String, CodingKey {
            case edge
            case execId = "exec_id"
        }
    }

    /// At a stage the Task waits on a person; on an edge its Flow's driver Exec runs.
    public enum Position: Codable, Sendable, Equatable {
        case stage(String)
        case edge(index: Int, execId: String)

        enum CodingKeys: String, CodingKey {
            case kind, stage, edge
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
                    execId: try container.decode(String.self, forKey: .execId))
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
            case .edge(let index, let execId):
                try container.encode("edge", forKey: .kind)
                try container.encode(index, forKey: .edge)
                try container.encode(execId, forKey: .execId)
            }
        }
    }

    public let name: String
    public let stages: [Stage]
    public let edges: [Edge]
    public let position: Position
    public let traversals: [Traversal]

    /// The edge whose Flow runs now, with its place among `edges`.
    public var running: (index: Int, edge: Edge)? {
        guard case .edge(let index, _) = position, edges.indices.contains(index) else { return nil }
        return (index, edges[index])
    }

    /// Edges leaving `stage`, each with its place among `edges`.
    public func edges(from stage: String) -> [(index: Int, edge: Edge)] {
        edges.enumerated().filter { $0.element.from == stage }.map { ($0.offset, $0.element) }
    }

    /// Edges a person can start next: those leaving the stage the Task waits
    /// at, or the one its running edge left.
    public var outgoing: [(index: Int, edge: Edge)] {
        if case .stage(let stage) = position { return edges(from: stage) }
        return edges(from: running?.edge.from ?? "start")
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
