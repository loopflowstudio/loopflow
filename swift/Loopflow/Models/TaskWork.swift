import Foundation

/// Rust owns checkout association; membership grants no execution authority.
public struct TaskWork: Codable, Sendable, Equatable {
    public let sessions: [TaskSession]
    public let graphProcesses: [GraphProcessInventoryEntry]
    public let processes: [LfProcess]
    /// The Task's Workflow; `nil` when it runs only ad hoc Graphs.
    public let workflow: Workflow?

    enum CodingKeys: String, CodingKey {
        case sessions, processes, workflow
        case graphProcesses = "graph_processes"
    }
}

/// Workspace facts; mechanical Process history is a separate diagnostic read.
public struct TaskOverview: Codable, Sendable, Equatable {
    public let sessions: [TaskSession]
    public let graphProcesses: [GraphProcessInventoryEntry]
    public let workflow: Workflow?

    enum CodingKeys: String, CodingKey {
        case sessions, workflow
        case graphProcesses = "graph_processes"
    }
}

/// A Task's Workflow: the graph it took up, where the Task stands on it, and
/// how it got there. Nodes are stages where a person takes part; edges are the
/// routes between them. `start` and `end` are implicit.
public struct Workflow: Codable, Sendable, Equatable {
    public struct Node: Codable, Sendable, Hashable {
        public let name: String
        public let skill: String
        /// One line saying what the person does here; `nil` when the skill says nothing.
        public let description: String?
    }

    public struct Edge: Codable, Sendable, Hashable {
        public let from: String
        public let to: String
        /// What `lf task run ISSUE <name>` takes to choose it: the graph or
        /// skill it runs first, or `end` when it runs nothing.
        public let name: String
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
        public let lfProcessId: String?
        public let actor: Actor
        public let sessionId: String?
        public let note: String?
        public let at: Int64

        enum CodingKeys: String, CodingKey {
            case workflow, kind, from, to, edge, actor, note, at
            case lfProcessId = "lf_process_id"
            case sessionId = "session_id"
        }
    }

    /// At a node the Task waits on a person. On an edge its Graph
    /// process carries it; one that no longer runs has stopped and holds the Task.
    public enum Position: Codable, Sendable, Equatable {
        case node(String)
        case edge(index: Int, lfProcessId: String, running: Bool)

        enum CodingKeys: String, CodingKey {
            case kind, node, edge, running
            case lfProcessId = "lf_process_id"
        }

        public init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            switch try container.decode(String.self, forKey: .kind) {
            case "node":
                self = .node(try container.decode(String.self, forKey: .node))
            case "edge":
                self = .edge(
                    index: try container.decode(Int.self, forKey: .edge),
                    lfProcessId: try container.decode(String.self, forKey: .lfProcessId),
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
            case .edge(let index, let lfProcessId, let running):
                try container.encode("edge", forKey: .kind)
                try container.encode(index, forKey: .edge)
                try container.encode(lfProcessId, forKey: .lfProcessId)
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
    /// Graph process of the Graph whose step opened the current input.
    public let graphLfProcessId: String?
    public let completedAt: Int64?

    enum CodingKeys: String, CodingKey {
        case id, title, interactive
        case graphLfProcessId = "graph_lf_process_id"
        case completedAt = "completed_at"
    }
}

/// One Graph as its Graph process records it; `id` is that Process.
public struct GraphProcessInventoryEntry: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let name: String
    public let state: GraphProcessSummaryState
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
public enum GraphProcessSummaryState: String, Codable, Sendable, Equatable {
    case current, completed, stopped
}

extension TaskSession {
    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = StructuredIdentity.session(try values.decode(String.self, forKey: .id))
        title = try values.decode(String.self, forKey: .title)
        interactive = try values.decode(Bool.self, forKey: .interactive)
        graphLfProcessId = (try values.decodeIfPresent(String.self, forKey: .graphLfProcessId)).map { StructuredIdentity.uuid($0) }
        completedAt = try values.decodeIfPresent(Int64.self, forKey: .completedAt)
    }
}
