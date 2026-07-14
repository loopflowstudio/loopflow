import Foundation

public struct WaveWorkMap: Sendable, Hashable {
    public let objective: String
    public let projects: [WaveProjectWork]

    public init(objective: String, projects: [WaveProjectWork]) {
        self.objective = objective
        self.projects = projects
    }
}

public struct WaveProjectWork: Decodable, Sendable, Identifiable, Hashable {
    public var id: String { project.id }

    public let project: ProjectPlanningSnapshot
    public let runtime: ProjectRuntimeSnapshot?
    public let directive: WorkDirectiveSnapshot?
    public let nextMove: WorkNextMove
    public let tasks: [WaveTaskWork]

    enum CodingKeys: String, CodingKey {
        case project, runtime, directive, tasks
        case nextMove = "next_move"
    }
}

public struct WaveTaskWork: Decodable, Sendable, Identifiable, Hashable {
    public var id: String { task.id }

    public let task: TaskPlanningSnapshot
    public let runtime: TaskRuntimeSnapshot?
    public let directive: WorkDirectiveSnapshot?
    public let nextMove: WorkNextMove
    public let pullRequests: [PullRequestSnapshot]

    enum CodingKeys: String, CodingKey {
        case task, runtime, directive
        case nextMove = "next_move"
        case pullRequests = "pull_requests"
    }
}

public struct ProjectPlanningSnapshot: Decodable, Sendable, Identifiable, Hashable {
    public let id: String
    public let slug: String
    public let name: String
    public let summary: String
    public let definition: String
    public let krs: [PlanningKeyResult]
}

public struct PlanningKeyResult: Decodable, Sendable, Identifiable, Hashable {
    public var id: String { text }

    public let text: String
    public let holds: Bool
}

public struct TaskPlanningSnapshot: Decodable, Sendable, Identifiable, Hashable {
    public let id: String
    public let identifier: String
    public let name: String
    public let description: String
    public let rank: UInt32
    public let completed: Bool
    public let assignee: String?
}

public struct ProjectRuntimeSnapshot: Decodable, Sendable, Hashable {
    public let sessionId: String
    public let status: ProjectSessionStatus
    public let reason: String
    public let statusAt: String
    public let iteration: UInt32
    public let pendingObservations: UInt32
    public let provider: String
    public let processAlive: Bool

    enum CodingKeys: String, CodingKey {
        case status, reason, iteration, provider
        case sessionId = "session_id"
        case statusAt = "status_at"
        case pendingObservations = "pending_observations"
        case processAlive = "process_alive"
    }
}

public struct TaskRuntimeSnapshot: Decodable, Sendable, Hashable {
    public let sessionId: String
    public let projectSessionId: String
    public let status: TaskSessionStatus
    public let reason: String
    public let statusAt: String
    public let worktree: String
    public let branch: String?
    public let provider: String
    public let processAlive: Bool

    enum CodingKeys: String, CodingKey {
        case status, reason, worktree, branch, provider
        case sessionId = "session_id"
        case projectSessionId = "project_session_id"
        case statusAt = "status_at"
        case processAlive = "process_alive"
    }
}

public enum ProjectSessionStatus: String, Decodable, Sendable, Hashable {
    case created, starting, running, waiting, blocked, failed, completed, abandoned
}

public enum TaskSessionStatus: String, Decodable, Sendable, Hashable {
    case created, starting, running, waiting, blocked, failed, completed, abandoned
}

public struct WorkDirectiveSnapshot: Decodable, Sendable, Hashable {
    public let version: UInt32
    public let kind: WorkDirectiveKind
    public let text: String
    public let appliedAt: String?
    public let incorporatedAt: String?
    public let incorporatedSummary: String?

    enum CodingKeys: String, CodingKey {
        case version, kind, text
        case appliedAt = "applied_at"
        case incorporatedAt = "incorporated_at"
        case incorporatedSummary = "incorporated_summary"
    }
}

public enum WorkDirectiveKind: String, Decodable, Sendable, Hashable {
    case initial
    case replacement
    case workRevised = "work_revised"
}

public enum WorkNextMoveOwner: String, Decodable, Sendable, Hashable {
    case human
    case wave
    case project
    case task
    case review
    case ci
    case external
}

public struct WorkNextMove: Decodable, Sendable, Hashable {
    public let owner: WorkNextMoveOwner
    public let reason: String
}

public struct PullRequestSnapshot: Decodable, Sendable, Hashable {
    public let number: UInt32
    public let url: URL
}

/// A reading from `lf status`, or the reason there is none. Mirrors Rust
/// `Evidence<T>` (`lf/commands/waves.rs`): "we looked and found nothing" and "we
/// could not look" are different facts, and a surface that renders them the same
/// is lying. `truncated` says a cap hid older items.
public enum WorkEvidence<Item: Decodable & Sendable & Hashable>: Decodable, Sendable, Hashable {
    case available(items: [Item], truncated: Bool)
    case unavailable(reason: String)

    enum CodingKeys: String, CodingKey {
        case state, items, truncated, reason
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let state = try container.decode(String.self, forKey: .state)
        switch state {
        case "ok":
            self = .available(
                items: try container.decode([Item].self, forKey: .items),
                truncated: try container.decode(Bool.self, forKey: .truncated)
            )
        case "unavailable":
            self = .unavailable(reason: try container.decode(String.self, forKey: .reason))
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .state,
                in: container,
                debugDescription: "unknown evidence state '\(state)'"
            )
        }
    }

    /// What was read. Empty for `unavailable` — callers that render must check
    /// the case, not this, or they turn a broken source into a quiet zero.
    public var items: [Item] {
        if case let .available(items, _) = self { return items }
        return []
    }

    public var unavailableReason: String? {
        if case let .unavailable(reason) = self { return reason }
        return nil
    }
}

public enum WaveAttentionKind: String, Decodable, Sendable, Hashable {
    case project
    case task
}

/// One Session waiting on somebody. Mirrors Rust `AttentionItem`. `ageSeconds`
/// is nil when the Session's timestamp cannot be read — an unknown age is never
/// a zero one.
public struct WaveAttentionItem: Decodable, Sendable, Hashable, Identifiable {
    public let kind: WaveAttentionKind
    public let id: String
    public let subject: String
    public let owner: WorkNextMoveOwner
    public let reason: String
    public let since: String
    public let ageSeconds: Int?

    enum CodingKeys: String, CodingKey {
        case kind, id, subject, owner, reason, since
        case ageSeconds = "age_secs"
    }
}

public struct WaveStatusResult: Sendable {
    public let workMap: WaveWorkMap
    public let loopState: String?
    public let runs: WorkEvidence<RunLedgerEntry>
    public let attention: WorkEvidence<WaveAttentionItem>

    public init(
        workMap: WaveWorkMap,
        loopState: String?,
        runs: WorkEvidence<RunLedgerEntry>,
        attention: WorkEvidence<WaveAttentionItem>
    ) {
        self.workMap = workMap
        self.loopState = loopState
        self.runs = runs
        self.attention = attention
    }
}
