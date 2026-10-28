import Foundation

/// Rust owns checkout association; membership grants no execution authority.
public struct TaskWork: Codable, Sendable, Equatable {
    public let sessions: [TaskSession]
    public let graphProcesses: [GraphProcessInventoryEntry]
    public let processes: [LfProcess]
    /// The graph the Task took up; `nil` until its first `lf task run`.
    public let graph: TaskGraph?

    enum CodingKeys: String, CodingKey {
        case sessions, processes, graph
        case graphProcesses = "graph_processes"
    }
}

/// Workspace facts; mechanical Process history is a separate diagnostic read.
public struct TaskOverview: Codable, Sendable, Equatable {
    public let sessions: [TaskSession]
    public let graphProcesses: [GraphProcessInventoryEntry]
    public let graph: TaskGraph?

    enum CodingKeys: String, CodingKey {
        case sessions, graph
        case graphProcesses = "graph_processes"
    }
}

/// The graph a Task took up and where the Task stands on it. An interactive
/// step is where a person takes part in the Task conversation; `start` and
/// `end` are implicit.
public struct TaskGraph: Codable, Sendable, Equatable {
    public struct InteractiveStep: Codable, Sendable, Hashable {
        public let name: String
        public let skill: String
        /// One line saying what the person does here; `nil` when the skill says nothing.
        public let description: String?
    }

    /// `at` waits at `start`, `end` or an interactive step. `run` is a
    /// `lf task run` carrying the Task on from `step`; one no longer running
    /// stopped, and the next `lf task run` starts it again.
    public enum Position: Codable, Sendable, Equatable {
        case at(step: String)
        case run(step: String, lfProcessId: String, running: Bool)

        enum CodingKeys: String, CodingKey {
            case kind, step, running
            case lfProcessId = "lf_process_id"
        }

        public init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            switch try container.decode(String.self, forKey: .kind) {
            case "at":
                self = .at(step: try container.decode(String.self, forKey: .step))
            case "run":
                self = .run(
                    step: try container.decode(String.self, forKey: .step),
                    lfProcessId: try container.decode(String.self, forKey: .lfProcessId),
                    running: try container.decode(Bool.self, forKey: .running))
            case let kind:
                throw DecodingError.dataCorruptedError(
                    forKey: .kind, in: container, debugDescription: "unknown graph position \(kind)")
            }
        }

        public func encode(to encoder: Encoder) throws {
            var container = encoder.container(keyedBy: CodingKeys.self)
            switch self {
            case .at(let step):
                try container.encode("at", forKey: .kind)
                try container.encode(step, forKey: .step)
            case .run(let step, let lfProcessId, let running):
                try container.encode("run", forKey: .kind)
                try container.encode(step, forKey: .step)
                try container.encode(lfProcessId, forKey: .lfProcessId)
                try container.encode(running, forKey: .running)
            }
        }

        /// Where the Task waits, or the step its run left.
        public var step: String {
            switch self {
            case .at(let step), .run(let step, _, _): step
            }
        }
    }

    public static let start = "start"
    public static let end = "end"

    public let name: String
    public let interactiveSteps: [InteractiveStep]
    public let position: Position

    enum CodingKeys: String, CodingKey {
        case name, position
        case interactiveSteps = "interactive_steps"
    }

    /// The interactive step a person takes part at now.
    public var waitingStep: InteractiveStep? {
        guard case .at(let step) = position else { return nil }
        return interactiveSteps.first { $0.name == step }
    }
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
