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
    public let reference: TaskReferenceSnapshot
    public let runtime: TaskRuntimeSnapshot?
    public let directive: WorkDirectiveSnapshot?
    public let nextMove: WorkNextMove
    public let condition: TaskConditionSnapshot
    public let actions: TaskActionModel
    public let prs: [PrSnapshot]
    public let activePr: String?

    enum CodingKeys: String, CodingKey {
        case task, reference, runtime, directive, condition, actions, prs
        case nextMove = "next_move"
        case activePr = "active_pr"
    }
}

public struct ProjectPlanningSnapshot: Decodable, Sendable, Identifiable, Hashable {
    public let id: String
    public let slug: String
    public let name: String
    public let summary: String
    public let definition: String
    public let flows: ProjectFlowPlanSnapshot
    public let krs: [PlanningKeyResult]
}

public struct ProjectFlowPlanSnapshot: Decodable, Sendable, Hashable {
    public let first: String?
    public let loopFlow: String?
    public let finallyFlow: String?

    enum CodingKeys: String, CodingKey {
        case first
        case loopFlow = "loop"
        case finallyFlow = "finally"
    }
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

public struct HistoricalFailure: Codable, Sendable, Hashable {
    public let message: String
    public let occurredAt: String

    enum CodingKeys: String, CodingKey {
        case message
        case occurredAt = "occurred_at"
    }
}

public struct ProjectRuntimeSnapshot: Decodable, Sendable, Hashable {
    public let workId: String
    public let status: WorkStatus
    public let reason: String
    public let updatedAt: String
    public let iteration: UInt32
    public let pendingObservations: UInt32
    public let provider: String
    public let lastFailure: HistoricalFailure?

    enum CodingKeys: String, CodingKey {
        case status, reason, iteration, provider
        case workId = "work_id"
        case updatedAt = "updated_at"
        case pendingObservations = "pending_observations"
        case lastFailure = "last_failure"
    }
}

public struct TaskRuntimeSnapshot: Decodable, Sendable, Hashable {
    public let workId: String
    public let projectId: String
    public let routingProjectId: String?
    public let status: WorkStatus
    public let reason: String
    public let updatedAt: String
    public let provider: String

    enum CodingKeys: String, CodingKey {
        case status, reason, provider
        case workId = "work_id"
        case projectId = "project_id"
        case routingProjectId = "routing_project_id"
        case updatedAt = "updated_at"
    }
}

/// Stable Task references shared by `lf status` and `lf roadmap`. The issue URL
/// comes from the cached PM snapshot; workspace evidence comes from durable
/// Task Work and remains after execution finishes.
public struct TaskReferenceSnapshot: Decodable, Sendable, Hashable {
    public let issueUrl: URL?
    public let workspace: TaskWorkspaceSnapshot?

    enum CodingKeys: String, CodingKey {
        case workspace
        case issueUrl = "issue_url"
    }
}

public struct TaskWorkspaceSnapshot: Decodable, Sendable, Hashable {
    public let slug: String
    public let branch: String?
    public let worktree: String
}

public enum RoadmapSection: String, Decodable, Sendable, Hashable {
    case now
    case waiting
    case available
    case later
}

public struct RoadmapProject: Decodable, Sendable, Identifiable, Hashable {
    public var id: String { project.id }

    public let project: ProjectPlanningSnapshot
    public let runtime: ProjectRuntimeSnapshot?
    public let nextMove: WorkNextMove
    public let section: RoadmapSection
    public let tasks: [RoadmapTask]

    enum CodingKeys: String, CodingKey {
        case project, runtime, section, tasks
        case nextMove = "next_move"
    }
}

public struct RoadmapTask: Decodable, Sendable, Identifiable, Hashable {
    public var id: String { task.id }

    public let task: TaskPlanningSnapshot
    public let reference: TaskReferenceSnapshot
    public let runtime: TaskRuntimeSnapshot?
    public let nextMove: WorkNextMove
    public let condition: TaskConditionSnapshot
    public let actions: TaskActionModel
    public let activePr: PrSnapshot?
    public let section: RoadmapSection

    enum CodingKeys: String, CodingKey {
        case task, reference, runtime, condition, actions, section
        case nextMove = "next_move"
        case activePr = "active_pr"
    }
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
    case user
    case wave
    case project
    case task
    case ci
    case external
}

public struct WorkNextMove: Decodable, Sendable, Hashable {
    public let owner: WorkNextMoveOwner
    public let reason: String
}

public enum TaskConditionState: String, Decodable, Sendable, Hashable {
    case waiting, blocked, clear, unknown
}

/// The lifecycle actions Task Work can take. Mirrors the Rust
/// `TaskAction`; the server computes which are legal, clients never re-derive.
public enum TaskAction: String, Decodable, Sendable, Hashable {
    case resume
    case openPr = "open_pr"
    case startNextPr = "start_next_pr"
    case complete
    case noAction = "no_action"
}

/// The next legal action and why. A nil recommendation means Task Work has not
/// started; callers do not reconstruct a matrix of blocked alternatives.
public struct TaskActionModel: Decodable, Sendable, Hashable {
    public let recommended: TaskAction?
    public let reason: String
}

public enum LocalProgressEvidenceState: String, Decodable, Sendable, Hashable {
    case observed, missing
    case notApplicable = "not_applicable"
    case unavailable
}

public struct LocalProgressEvidence: Decodable, Sendable, Hashable {
    public let state: LocalProgressEvidenceState
    public let unsettled: Bool?
    public let dirty: Bool?
    public let authoredCommits: Bool?
    public let recoveryRequired: Bool?
    public let reason: String?

    enum CodingKeys: String, CodingKey {
        case state, unsettled, dirty, reason
        case authoredCommits = "authored_commits"
        case recoveryRequired = "recovery_required"
    }
}

public struct TaskConditionSnapshot: Decodable, Sendable, Hashable {
    public let state: TaskConditionState
    public let reason: String
    public let observedAt: String
    public let evidenceAgeSeconds: Int?
    public let localProgress: LocalProgressEvidence

    enum CodingKeys: String, CodingKey {
        case state, reason
        case observedAt = "observed_at"
        case evidenceAgeSeconds = "evidence_age_secs"
        case localProgress = "local_progress"
    }
}

public enum PrPhase: String, Decodable, Sendable, Hashable {
    case working, publishing, open, merged, abandoned
}

public struct PrSnapshot: Decodable, Sendable, Identifiable, Hashable {
    public let id: String
    public let sequence: UInt32
    public let slug: String
    public let branch: String
    public let baseCommit: String
    public let phase: PrPhase
    public let empty: Bool?
    public let publication: PrPublicationSnapshot?
    public let mergeCommit: String?
    public let abandonedAt: String?

    enum CodingKeys: String, CodingKey {
        case id, sequence, slug, branch, phase, empty, publication
        case baseCommit = "base_commit"
        case mergeCommit = "merge_commit"
        case abandonedAt = "abandoned_at"
    }
}

public struct PrPublicationSnapshot: Decodable, Sendable, Hashable {
    public let requestedAt: String
    public let presentation: PrPresentationSnapshot?
    public let github: GithubPrSnapshot?
    public let merge: PrMergeRequestSnapshot?

    enum CodingKeys: String, CodingKey {
        case presentation, github, merge
        case requestedAt = "requested_at"
    }
}

public struct PrPresentationSnapshot: Decodable, Sendable, Hashable {
    public let title: String
    public let body: String
    public let headSha: String

    enum CodingKeys: String, CodingKey {
        case title, body
        case headSha = "head_sha"
    }
}

public enum PrMergeMode: String, Decodable, Sendable, Hashable {
    case user, auto
}

public struct PrMergeRequestSnapshot: Decodable, Sendable, Hashable {
    public let mode: PrMergeMode
    public let requestedAt: String
    public let headSha: String
    public let afterMerge: PrAfterMerge
    public let nextSlug: String?

    enum CodingKeys: String, CodingKey {
        case mode
        case requestedAt = "requested_at"
        case headSha = "head_sha"
        case afterMerge = "after_merge"
        case nextSlug = "next_slug"
    }
}

public enum PrAfterMerge: String, Decodable, Sendable, Hashable {
    case continueTask = "continue_task"
    case completeTask = "complete_task"
}

public struct GithubPrSnapshot: Decodable, Sendable, Hashable {
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
