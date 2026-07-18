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
    public let attention: TaskAttentionSnapshot
    public let prs: [PrSnapshot]
    public let activePr: String?

    enum CodingKeys: String, CodingKey {
        case task, reference, runtime, directive, attention, prs
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

/// The observed state of a Session's current body, distinct from durable intent.
/// `status` records intent; `BodyObservation` records what the current body is
/// observed doing. Working versus Stalled is exactly the difference between a
/// live body that advanced recently and one silent past its deadline.
public enum BodyCategory: String, Decodable, Sendable, Hashable {
    case working, stalled, recovering
    case needsInput = "needs_input"
    case stopped, failed, terminal, unobservable
}

public enum BodyOwner: String, Decodable, Sendable, Hashable {
    case session, loopflow, human, nobody, unknown
}

public enum BodyControl: String, Decodable, Sendable, Hashable {
    case steer, interrupt, stop, extend, resume, decide, abandon, attach
}

public struct BodyObservation: Decodable, Sendable, Hashable {
    public let category: BodyCategory
    public let reason: String
    public let owner: BodyOwner
    public let controls: [BodyControl]
    public let progressAgeSeconds: UInt64?
    public let deadlineInSeconds: Int64?
    public let step: String?

    enum CodingKeys: String, CodingKey {
        case category, reason, owner, controls, step
        case progressAgeSeconds = "progress_age_secs"
        case deadlineInSeconds = "deadline_in_secs"
    }
}

public struct ProjectRuntimeSnapshot: Decodable, Sendable, Hashable {
    public let sessionId: String
    public let status: ProjectStatus
    public let reason: String
    public let statusAt: String
    public let iteration: UInt32
    public let pendingObservations: UInt32
    public let provider: String
    public let processAlive: Bool
    public let observation: BodyObservation

    enum CodingKeys: String, CodingKey {
        case status, reason, iteration, provider, observation
        case sessionId = "session_id"
        case statusAt = "status_at"
        case pendingObservations = "pending_observations"
        case processAlive = "process_alive"
    }
}

public struct TaskRuntimeSnapshot: Decodable, Sendable, Hashable {
    public let sessionId: String
    public let projectSessionId: String
    /// The live Project Session this Task routes to (a successor when the
    /// historical owner is terminal). Nil when the chain is broken.
    public let routingProjectSessionId: String?
    public let status: TaskStatus
    public let reason: String
    public let statusAt: String
    public let provider: String
    public let processAlive: Bool
    public let observation: BodyObservation

    enum CodingKeys: String, CodingKey {
        case status, reason, provider, observation
        case sessionId = "session_id"
        case projectSessionId = "project_session_id"
        case routingProjectSessionId = "routing_project_session_id"
        case statusAt = "status_at"
        case processAlive = "process_alive"
    }
}

/// Stable Task references shared by `lf status` and `lf roadmap`. The issue URL
/// comes from the cached PM snapshot; workspace evidence comes from the durable
/// Task Session and remains after execution finishes.
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
    case needsAttention = "needs_attention"
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
    public let attention: TaskAttentionSnapshot
    public let activePr: PrSnapshot?
    public let section: RoadmapSection

    enum CodingKeys: String, CodingKey {
        case task, reference, runtime, attention, section
        case nextMove = "next_move"
        case activePr = "active_pr"
    }
}

public enum ProjectStatus: String, Decodable, Sendable, Hashable {
    case created, starting, running, waiting, blocked, failed, completed, abandoned
}

public enum TaskStatus: String, Decodable, Sendable, Hashable {
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

public enum TaskAttentionLevel: String, Decodable, Sendable, Hashable {
    case green, red, blue, black, unknown
}

/// The six lifecycle actions a Task Session can take. Mirrors the Rust
/// `TaskAction`; the server computes which are legal, clients never re-derive.
public enum TaskAction: String, Decodable, Sendable, Hashable {
    case recover, resume, review
    case startNextPr = "start_next_pr"
    case complete
    case noAction = "no_action"
}

/// One action's legal status: the reason explains why it is legal when
/// available, and names the blocking fact when it is not.
public struct TaskActionStatus: Decodable, Sendable, Hashable {
    public let action: TaskAction
    public let available: Bool
    public let reason: String
}

/// The complete legal-action model: all six actions in canonical order.
/// `recommended` is always one of the available actions, or nil when no
/// Task Session exists.
public struct TaskActionModel: Decodable, Sendable, Hashable {
    public let recommended: TaskAction?
    public let actions: [TaskActionStatus]

    public func status(_ action: TaskAction) -> TaskActionStatus? {
        actions.first { $0.action == action }
    }
}

public enum TaskProcessEvidenceState: String, Decodable, Sendable, Hashable {
    case observed
    case notExpected = "not_expected"
    case notApplicable = "not_applicable"
    case unavailable
}

public struct TaskProcessEvidence: Decodable, Sendable, Hashable {
    public let state: TaskProcessEvidenceState
    public let alive: Bool?
    public let reason: String?
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

public struct TaskAttentionSnapshot: Decodable, Sendable, Hashable {
    public let level: TaskAttentionLevel
    public let reason: String
    public let observedAt: String
    public let evidenceAgeSeconds: Int?
    public let nextOwner: WorkNextMoveOwner
    public let actions: TaskActionModel
    public let pmCompleted: Bool
    public let sessionStatus: TaskStatus?
    public let process: TaskProcessEvidence
    public let localProgress: LocalProgressEvidence
    public let activePrPhase: PrPhase?

    enum CodingKeys: String, CodingKey {
        case level, reason, actions, process
        case observedAt = "observed_at"
        case evidenceAgeSeconds = "evidence_age_secs"
        case nextOwner = "next_owner"
        case pmCompleted = "pm_completed"
        case sessionStatus = "session_status"
        case localProgress = "local_progress"
        case activePrPhase = "active_pr_phase"
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
    public let afterMerge: PrAfterMerge
    public let nextSlug: String?
    public let github: GithubPrSnapshot?

    enum CodingKeys: String, CodingKey {
        case github
        case requestedAt = "requested_at"
        case afterMerge = "after_merge"
        case nextSlug = "next_slug"
    }
}

public enum PrAfterMerge: String, Decodable, Sendable, Hashable {
    case review
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
