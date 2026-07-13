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
    public let delivery: TaskDeliverySnapshot?
    public let workers: WorkerSummary

    enum CodingKeys: String, CodingKey {
        case task, runtime, directive, delivery, workers
        case nextMove = "next_move"
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
    public let status: String
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
    public let supervisor: WorkController
    public let status: String
    public let reason: String
    public let statusAt: String
    public let worktree: String
    public let branch: String
    public let provider: String
    public let processAlive: Bool

    enum CodingKeys: String, CodingKey {
        case supervisor, status, reason, worktree, branch, provider
        case sessionId = "session_id"
        case statusAt = "status_at"
        case processAlive = "process_alive"
    }
}

public struct WorkController: Decodable, Sendable, Hashable {
    public let kind: String
    public let waveId: String?
    public let sessionId: String?

    enum CodingKeys: String, CodingKey {
        case kind
        case waveId = "wave_id"
        case sessionId = "session_id"
    }
}

public struct WorkDirectiveSnapshot: Decodable, Sendable, Hashable {
    public let version: UInt32
    public let kind: String
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

public struct TaskDeliverySnapshot: Decodable, Sendable, Hashable {
    public let kind: String
    public let base: String
    public let prNumber: UInt32?
    public let prURL: URL?

    enum CodingKeys: String, CodingKey {
        case kind, base
        case prNumber = "pr_number"
        case prURL = "pr_url"
    }
}

public struct WorkerSummary: Decodable, Sendable, Hashable {
    public let active: UInt32
    public let total: UInt32
}

public struct WaveStatusResult: Sendable {
    public let runs: [Run]
    public let workMap: WaveWorkMap
    public let attention: [AttentionItem]
    public let loopState: String?

    public init(
        runs: [Run],
        workMap: WaveWorkMap,
        attention: [AttentionItem],
        loopState: String?
    ) {
        self.runs = runs
        self.workMap = workMap
        self.attention = attention
        self.loopState = loopState
    }
}
