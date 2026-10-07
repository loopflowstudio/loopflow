import Foundation

/// Rust owns checkout association; membership grants no execution authority.
public struct TaskWork: Codable, Sendable, Equatable {
    public let sessions: [TaskSession]
    public let flowProcesses: [FlowProcessInventoryEntry]
    public let processes: [Process]
    /// The Task's Workflow; `nil` when it runs only ad hoc Flows.
    public let workflow: Workflow?

    enum CodingKeys: String, CodingKey {
        case sessions, processes, workflow
        case flowProcesses = "flow_processes"
    }
}

/// A Task's Workflow: the definition it took up, where the Task stands on it,
/// and how it got there. `start` and `end` are implicit nodes.
public struct Workflow: Codable, Sendable, Equatable {
    public struct Node: Codable, Sendable, Hashable {
        public let name: String
        public let skill: String
        /// One line saying what the person does here; `nil` when neither the
        /// definition nor its skill says.
        public let description: String?
    }

    public struct Edge: Codable, Sendable, Hashable {
        public let from: String
        public let to: String
        public let flow: String?

        /// What `lf task run ISSUE <name>` takes to choose it: its Flow, or
        /// the node it enters when it runs none.
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
        /// The `lf` process that made the move; `nil` when none was registered.
        public let processLfid: String?
        public let actor: Actor
        public let sessionId: String?
        public let note: String?
        public let at: Int64

        enum CodingKeys: String, CodingKey {
            case workflow, kind, from, to, edge, actor, note, at
            case processLfid = "process_lfid"
            case sessionId = "session_id"
        }
    }

    /// At a node the Task waits on a person. On an edge its Flow's driver
    /// Process carries it; one that no longer runs has stopped and holds the Task.
    public enum Position: Codable, Sendable, Equatable {
        case node(String)
        case edge(index: Int, processLfid: String, running: Bool)

        enum CodingKeys: String, CodingKey {
            case kind, node, edge, running
            case processLfid = "process_lfid"
        }

        public init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            switch try container.decode(String.self, forKey: .kind) {
            case "node":
                self = .node(try container.decode(String.self, forKey: .node))
            case "edge":
                self = .edge(
                    index: try container.decode(Int.self, forKey: .edge),
                    processLfid: try container.decode(String.self, forKey: .processLfid),
                    running: try container.decode(Bool.self, forKey: .running))
            case let kind:
                throw DecodingError.dataCorruptedError(
                    forKey: .kind, in: container, debugDescription: "unknown workflow position \(kind)")
            }
        }

        public func encode(to encoder: Encoder) throws {
            var container = encoder.container(keyedBy: CodingKeys.self)
            switch self {
            case .node(let node):
                try container.encode("node", forKey: .kind)
                try container.encode(node, forKey: .node)
            case .edge(let index, let processLfid, let running):
                try container.encode("edge", forKey: .kind)
                try container.encode(index, forKey: .edge)
                try container.encode(processLfid, forKey: .processLfid)
                try container.encode(running, forKey: .running)
            }
        }
    }

    public let name: String
    public let nodes: [Node]
    public let edges: [Edge]
    public let position: Position
    /// Edges `lf task run` can choose now, by their place among `edges`.
    public let outgoing: [Int]
    public let history: [Move]
}

public struct TaskSession: Codable, Sendable, Equatable, Identifiable {
    public let id: String
    public let title: String
    public let interactive: Bool
    /// Driver Process of the Flow whose step opened the current input.
    public let flowProcessLfid: String?
    public let completedAt: Int64?

    enum CodingKeys: String, CodingKey {
        case id, title, interactive
        case flowProcessLfid = "flow_process_lfid"
        case completedAt = "completed_at"
    }
}

/// One Flow as its driver Process records it; `id` is that Process.
public struct FlowProcessInventoryEntry: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let name: String
    public let state: FlowProcessSummaryState
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
public enum FlowProcessSummaryState: String, Codable, Sendable, Equatable {
    case current, completed, stopped
}
