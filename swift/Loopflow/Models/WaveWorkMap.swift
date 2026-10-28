import Foundation

public struct WaveWorkMap: Sendable, Hashable {
    public let objective: String
    public let projects: WorkEvidence<ProjectPlanningSnapshot>
    public var currentProject: ProjectPlanningSnapshot? { projects.currentProject }
    public let tasks: WorkEvidence<WaveTaskWork>

    public init(objective: String, projects: WorkEvidence<ProjectPlanningSnapshot>, tasks: WorkEvidence<WaveTaskWork>) {
        self.objective = objective
        self.projects = projects
        self.tasks = tasks
    }
}

public enum ProjectStatus: String, Decodable, Sendable, Hashable {
    case backlog, planned, started, paused, completed, canceled
}

public struct ProjectPlanningSnapshot: Decodable, Sendable, Identifiable, Hashable {
    public let sync: PlanningSyncStatus
    public let current: Bool
    public let id: String
    public let linearId: String?
    public let slug: String
    public let name: String
    public let status: ProjectStatus
    public let metricTargets: [MetricTarget]
    public let krs: [PlanningKeyResult]

    enum CodingKeys: String, CodingKey {
        case id, slug, name, status, krs, current, sync
        case linearId = "linear_id"
        case metricTargets = "metric_targets"
    }
}

extension WorkEvidence where Item == ProjectPlanningSnapshot {
    public var currentProject: ProjectPlanningSnapshot? {
        let current = items.filter { $0.current }
        return current.count == 1 ? current[0] : nil
    }
}

public struct WaveTaskWork: Decodable, Sendable, Identifiable, Hashable {
    public var id: String { task.id }

    public let task: TaskPlanningSnapshot
    public let reference: TaskReferenceSnapshot
    public let runtime: TaskRuntimeSnapshot?
    public let graphName: String?
    public let graphSource: String?
    public let latestGraphProcess: GraphProcessDetail?
    public let execution: TaskExecutionSnapshot?
    public let runControl: TaskRunControl
    public let shipControl: TaskRunControl
    public let shipping: ShippingSnapshot?
    public let directive: WorkDirectiveSnapshot?
    public let nextMove: WorkNextMove
    public let condition: TaskConditionSnapshot
    public let actions: TaskActionModel
    public let pr: PrSnapshot?

    enum CodingKeys: String, CodingKey {
        case task, reference, runtime, directive, condition, actions, pr, execution
        case graphName = "graph_name"
        case graphSource = "graph_source"
        case latestGraphProcess = "latest_graph_process"
        case runControl = "run_control"
        case shipControl = "ship_control"
        case shipping
        case nextMove = "next_move"
    }
}

public struct PlanningKeyResult: Decodable, Sendable, Identifiable, Hashable {
    public var id: String { text }

    public let text: String
    public let holds: Bool
}

public struct TaskAcquisition: Decodable, Sendable, Hashable {
    public let state: State
    public let observedAt: Int64?

    public enum State: String, Decodable, Sendable {
        case available, invalid, removed, absent, unavailable
    }

    enum CodingKeys: String, CodingKey {
        case state
        case observedAt = "observed_at"
    }
}

public struct TaskPlanningSnapshot: Decodable, Sendable, Identifiable, Hashable {
    public let acquisition: TaskAcquisition
    public let sync: PlanningSyncStatus
    public let id: String
    public let repositoryId: String
    public let repo: String
    public let projectId: String?
    public let waveId: String?
    public let linearId: String?
    public let identifier: String
    public let name: String
    public let description: String
    public let rank: UInt32
    public let state: String?
    public let completedAt: String?
    public let dueDate: String?
    public let assignee: String?

    enum CodingKeys: String, CodingKey {
        case repositoryId = "repository_id", projectId = "project_id", waveId = "wave_id"
        case repo
        case id, identifier, name, description, rank, state, assignee, sync, acquisition
        case linearId = "linear_id"
        case completedAt = "completed_at"
        case dueDate = "due_date"
    }

    public var terminalLabel: String? {
        switch state {
        case "canceled": "Canceled"
        case "duplicate": "Duplicate"
        default: isSuccessful ? "Completed" : nil
        }
    }

    public var historyLabel: String? {
        isSuccessful && completedAt == nil ? "Completed · date unavailable" : terminalLabel
    }

    public var isTerminal: Bool { isSuccessful || state == "canceled" || state == "duplicate" }
    public var isSuccessful: Bool { state == "completed" }
}

public struct TaskRuntimeSnapshot: Decodable, Sendable, Hashable {
    public let workId: String
    public let status: TaskState
    /// A durable completion request awaiting settlement.
    public let completionPending: String?
    public let reason: String
    public let updatedAt: String
    public let provider: String
    /// Who owns the Task on which Machine; absent when it is unowned.
    public let owner: TaskOwner?

    enum CodingKeys: String, CodingKey {
        case status, reason, provider, owner
        case workId = "work_id"
        case completionPending = "completion_pending"
        case updatedAt = "updated_at"
    }
}

/// A Machine for display. A missing id is an unknown Machine.
public struct MachineRef: Decodable, Sendable, Hashable {
    public let id: String?
    public let label: String?
}

/// The person and Machine that own a Task.
public struct TaskOwner: Decodable, Sendable, Hashable {
    public let person: String?
    /// The owner's lowercase Git email; shown when they have no name.
    public let personKey: String?
    public let machine: MachineRef

    enum CodingKeys: String, CodingKey {
        case person, machine
        case personKey = "person_key"
    }

    /// "Jack Heart on mini"; "Machine unknown" when the Machine was never recorded.
    public var summary: String {
        let person = person ?? personKey ?? "Someone"
        if let label = machine.label, !label.isEmpty { return "\(person) on \(label)" }
        if let id = machine.id { return "\(person) on \(id)" }
        return "\(person) · Machine unknown"
    }
}

/// Stable Task references shared by `lf wave status` and `lf wave show`. The issue URL
/// comes from the cached PM snapshot; workspace evidence comes from durable
/// Task Work and remains after execution finishes.
public struct TaskReferenceSnapshot: Decodable, Sendable, Hashable {
    public let issueUrl: URL?
    public let workspace: TaskWorktreeSnapshot?

    enum CodingKeys: String, CodingKey {
        case workspace
        case issueUrl = "issue_url"
    }
}

public struct TaskWorktreeSnapshot: Decodable, Sendable, Hashable {
    public let machineId: String?
    public let machineLabel: String?
    public var identity: WorkspaceIdentity? { machineId.map { WorkspaceIdentity(machineId: $0, worktree: worktree) } }
    public let slug: String
    public let branch: String?
    public let baseCommit: String
    public let worktree: String
    public let localExists: Bool?

    enum CodingKeys: String, CodingKey {
        case slug, branch, worktree
        case baseCommit = "base_commit"
        case machineId = "machine_id", machineLabel = "machine_label"
        case localExists = "local_exists"
    }
}

public enum RoadmapSection: String, Decodable, Sendable, Hashable {
    case now
    case waiting
    case notStarted = "not_started"
    case later
}

public struct RoadmapTask: Decodable, Sendable, Identifiable, Hashable {
    public var id: String { task.id }

    public let task: TaskPlanningSnapshot
    public let reference: TaskReferenceSnapshot
    public let runtime: TaskRuntimeSnapshot?
    public let nextMove: WorkNextMove
    public let condition: TaskConditionSnapshot
    public let actions: TaskActionModel
    public let graphName: String?
    public let graphSource: String?
    public let latestGraphProcess: GraphProcessDetail?
    public let execution: TaskExecutionSnapshot?
    public let runControl: TaskRunControl
    public let shipControl: TaskRunControl
    public let shipping: ShippingSnapshot?
    public let pr: PrSnapshot?
    public let section: RoadmapSection

    enum CodingKeys: String, CodingKey {
        case task, reference, runtime, condition, actions, execution, section, pr
        case graphName = "graph_name"
        case graphSource = "graph_source"
        case latestGraphProcess = "latest_graph_process"
        case runControl = "run_control"
        case shipControl = "ship_control"
        case shipping
        case nextMove = "next_move"
    }
}

/// What a shipped Task is doing, derived in Rust; nothing here waits on a person.
public struct ShippingSnapshot: Decodable, Sendable, Hashable {
    public let shippedAt: Int64
    public let state: State
    public let reason: String

    public enum State: String, Decodable, Sendable, Hashable {
        case building, landing, repairing, restarting, deferred
    }

    public var label: String {
        switch state {
        case .building: "Building"
        case .landing: "Landing"
        case .repairing: "Repairing"
        case .restarting: "Restarting"
        case .deferred: "Deferred"
        }
    }

    enum CodingKeys: String, CodingKey {
        case state, reason
        case shippedAt = "shipped_at"
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
public enum TaskAction: String, Codable, Sendable, Hashable {
    case resume
    case openPr = "open_pr"
    case noAction = "no_action"
}

/// The next legal action and why. A nil recommendation means Task Work has not
/// started; callers do not reconstruct a matrix of blocked alternatives.
public struct TaskActionModel: Codable, Sendable, Hashable {
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
    /// Started work a finished plan has not settled; keeps a terminal Task current.
    public let unresolvedExecution: Bool

    enum CodingKeys: String, CodingKey {
        case state, reason
        case observedAt = "observed_at"
        case evidenceAgeSeconds = "evidence_age_secs"
        case localProgress = "local_progress"
        case unresolvedExecution = "unresolved_execution"
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

    enum CodingKeys: String, CodingKey {
        case mode
        case requestedAt = "requested_at"
        case headSha = "head_sha"
    }
}

public struct GithubPrSnapshot: Decodable, Sendable, Hashable {
    public let number: UInt32
    public let url: URL
}

/// A reading from `lf wave status`, or the reason there is none. Mirrors Rust
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

    /// The items that pass `isIncluded`; unavailable evidence stays unavailable.
    public func filter(_ isIncluded: (Item) -> Bool) -> WorkEvidence<Item> {
        guard case let .available(items, truncated) = self else { return self }
        return .available(items: items.filter(isIncluded), truncated: truncated)
    }
}
