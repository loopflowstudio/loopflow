import Foundation

/// Rust owns checkout association; membership grants no execution authority.
public struct TaskWork: Codable, Sendable, Equatable {
    public let sessions: [TaskSession]
    public let graphProcesses: [GraphProcessInventoryEntry]
    /// The graph the Task took up; `nil` until its first `lf task run`.
    public let graph: TaskGraph?

    enum CodingKeys: String, CodingKey {
        case sessions, graph
        case graphProcesses = "graph_processes"
    }
}

/// `lf task status`'s recorded execution: the Task's work, who started its
/// latest Graph, and its auto setting.
public struct TaskExecution: Decodable, Sendable, Equatable {
    public let work: TaskWork
    public let startedBy: Starter?
    /// On: the run continues through breakpoints. Off: it waits at its
    /// landing step until `lf task auto on`. `nil` when nobody set it.
    public let auto: Auto?
    /// The Task this one is stacked on; `nil` when it builds on the default branch.
    public let stackParent: TaskStackParent?

    enum CodingKeys: String, CodingKey {
        case work
        case startedBy = "started_by", auto
        case stackParent = "stack_parent"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        work = try container.decode(TaskWork.self, forKey: .work)
        startedBy = try container.decode(Starter?.self, forKey: .startedBy)
        auto = try container.decode(Auto?.self, forKey: .auto)
        stackParent = try container.decode(TaskStackParent?.self, forKey: .stackParent)
    }
}

/// The open Task a stacked Task builds on, named by Task rather than PR row.
public struct TaskStackParent: Decodable, Sendable, Hashable {
    public let identifier: String
    public let branch: String
    /// `nil` until the parent opens its PR.
    public let prNumber: UInt64?

    enum CodingKeys: String, CodingKey {
        case identifier, branch
        case prNumber = "pr_number"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        identifier = try container.decode(String.self, forKey: .identifier)
        branch = try container.decode(String.self, forKey: .branch)
        prNumber = try container.decode(UInt64?.self, forKey: .prNumber)
    }
}

/// Who started a Process, derived by Rust from its ancestry: the first Session
/// an agent ran it from, else the person at the root of its chain.
public enum Starter: Decodable, Sendable, Hashable {
    /// `nil` when no participant is configured or the chain is missing rows.
    case person(name: String?)
    case session(id: String, name: String?, requester: String?)
    /// A configured default set it (`auto: on`): "wave", "repository" or "personal".
    case config(layer: String)

    private enum CodingKeys: String, CodingKey { case kind, name, id, requester, layer }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "person": self = .person(name: try container.decode(String?.self, forKey: .name))
        case "session":
            self = .session(
                id: try container.decode(String.self, forKey: .id),
                name: try container.decode(String?.self, forKey: .name),
                requester: try container.decode(String?.self, forKey: .requester)
            )
        case "config": self = .config(layer: try container.decode(String.self, forKey: .layer))
        case let kind:
            throw DecodingError.dataCorruptedError(
                forKey: .kind, in: container, debugDescription: "unknown starter '\(kind)'"
            )
        }
    }

    /// Reads as `lf task status` prints it.
    public var label: String {
        switch self {
        case .person(let name): name ?? "attribution unresolved"
        case .session(let id, let name, let requester):
            (name.map { "Session \"\($0)\"" } ?? "Session \(id)") + (requester.map { " for \($0)" } ?? "")
        case .config(let layer): "\(layer) config"
        }
    }
}

/// The Task's auto setting: on or off, when, and who set it.
public struct Auto: Decodable, Sendable, Hashable {
    public enum State: String, Decodable, Sendable, Hashable { case on, off }

    public let state: State
    public let at: Int64
    public let by: Starter
}

/// Workspace facts without the Task's full history.
public struct TaskOverview: Codable, Sendable, Equatable {
    public let sessions: [TaskSession]
    public let graphProcesses: [GraphProcessInventoryEntry]
    public let graph: TaskGraph?

    enum CodingKeys: String, CodingKey {
        case sessions, graph
        case graphProcesses = "graph_processes"
    }
}

/// The primary workflow button, read with the saved Graph cursor.
public struct WorkflowControl: Codable, Sendable, Hashable {
    public enum Action: String, Codable, Sendable {
        case run, `continue`, pause, pausing, unpause, resume, wait, done
    }

    public let action: Action
    public let label: String
}

/// The graph a Task took up and where the Task stands on it. A breakpoint is
/// where a person takes part in the Task conversation. `start` and `end` are implicit.
public struct TaskGraph: Codable, Sendable, Equatable {
    /// `at` waits at `start`, `end`, a landing wait, or a legacy unconverted stop.
    /// `breakpoint` is parked at a Graph breakpoint for a specific arrival.
    /// `run` is a `lf task run` carrying the Task on from `step`, running `at`; one no
    /// longer running stopped, and the next `lf task run` starts it again at `at`.
    public enum Position: Codable, Sendable, Equatable {
        case at(step: String)
        case breakpoint(step: String, arrival: String, blocker: String?)
        case run(step: String, at: String, lfProcessId: String, running: Bool)

        enum CodingKeys: String, CodingKey {
            case kind, step, at, running, arrival, blocker
            case lfProcessId = "lf_process_id"
        }

        public init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            switch try container.decode(String.self, forKey: .kind) {
            case "at":
                self = .at(step: try container.decode(String.self, forKey: .step))
            case "breakpoint":
                self = .breakpoint(step: try container.decode(String.self, forKey: .step),
                                   arrival: try container.decode(String.self, forKey: .arrival),
                                   blocker: try container.decode(String?.self, forKey: .blocker))
            case "run":
                self = .run(
                    step: try container.decode(String.self, forKey: .step),
                    at: try container.decode(String.self, forKey: .at),
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
            case .breakpoint(let step, let arrival, let blocker):
                try container.encode("breakpoint", forKey: .kind)
                try container.encode(step, forKey: .step)
                try container.encode(arrival, forKey: .arrival)
                try container.encode(blocker, forKey: .blocker)
            case .run(let step, let at, let lfProcessId, let running):
                try container.encode("run", forKey: .kind)
                try container.encode(step, forKey: .step)
                try container.encode(at, forKey: .at)
                try container.encode(lfProcessId, forKey: .lfProcessId)
                try container.encode(running, forKey: .running)
            }
        }

        /// Where the Task waits, or the step its run left.
        public var step: String {
            switch self {
            case .at(let step), .breakpoint(let step, _, _), .run(let step, _, _, _): step
            }
        }
    }

    /// One top-level step of the Graph, as the workflow line shows it.
    public struct Step: Codable, Sendable, Hashable {
        public enum Kind: String, Codable, Sendable, Hashable {
            case stage
            case stageWithBreakpoint = "stage_with_breakpoint"
            case breakpoint
        }

        public let name: String
        public let kind: Kind
    }

    public static let start = "start"
    public static let end = "end"

    public let name: String
    public let control: WorkflowControl
    /// Every breakpoint in order, including composed Graphs and XOR paths.
    public let breakpoints: [String]
    public let position: Position
    /// The Graph's top-level steps in order.
    public let steps: [Step]
    /// The index in `steps` of the step the Task stands in; `nil` at `start` and `end`.
    public let currentStep: Int?
    /// Where the Task stands, in Rust's words: "build is running implement.
    /// You are next needed at after-ship-review."
    public let sentence: String

    enum CodingKeys: String, CodingKey {
        case name, control, breakpoints, position, steps, sentence
        case currentStep = "current_step"
    }

    /// The breakpoint where a person takes part now (or a legacy capture's stop).
    public var waitingStep: String? {
        switch position {
        case .breakpoint(let step, _, _): return step
        case .at(let step): return breakpoints.first { $0 == step }
        case .run: return nil
        }
    }
}

extension TaskGraph {
    // In an extension so the memberwise initializer stays.
    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            name: try container.decode(String.self, forKey: .name),
            control: try container.decode(WorkflowControl.self, forKey: .control),
            breakpoints: try container.decode([String].self, forKey: .breakpoints),
            position: try container.decode(Position.self, forKey: .position),
            steps: try container.decode([Step].self, forKey: .steps),
            currentStep: try container.decode(Int?.self, forKey: .currentStep),
            sentence: try container.decode(String.self, forKey: .sentence))
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
