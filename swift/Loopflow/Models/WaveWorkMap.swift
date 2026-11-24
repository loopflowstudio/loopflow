import Foundation

public struct WaveWorkMap: Sendable, Hashable {
    public let objective: String
    public let projects: WorkEvidence<ProjectPlanningSnapshot>
    public var currentProject: ProjectPlanningSnapshot? { projects.currentProject }
    public let tasks: WorkEvidence<RoadmapTask>

    public init(objective: String, projects: WorkEvidence<ProjectPlanningSnapshot>, tasks: WorkEvidence<RoadmapTask>) {
        self.objective = objective
        self.projects = projects
        self.tasks = tasks
    }
}

public enum ProjectStatus: String, Decodable, Sendable, Hashable {
    case backlog, planned, started, paused, completed, canceled
}

public struct ProjectPlanningSnapshot: Decodable, Sendable, Identifiable, Hashable {
    public let sync: TrackerSync
    public let current: Bool
    public let id: String
    public let trackerId: String?
    public let slug: String
    public let name: String
    public let status: ProjectStatus
    public let metricTargets: [MetricTarget]
    public let krs: [PlanningKeyResult]

    enum CodingKeys: String, CodingKey {
        case id, slug, name, status, krs, current, sync
        case trackerId = "tracker_id"
        case metricTargets = "metric_targets"
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        sync = try values.decode(TrackerSync.self, forKey: .sync)
        current = try values.decode(Bool.self, forKey: .current)
        id = try values.decode(String.self, forKey: .id)
        trackerId = try values.decode(String?.self, forKey: .trackerId)
        slug = try values.decode(String.self, forKey: .slug)
        name = try values.decode(String.self, forKey: .name)
        status = try values.decode(ProjectStatus.self, forKey: .status)
        metricTargets = try values.decode([MetricTarget].self, forKey: .metricTargets)
        krs = try values.decode([PlanningKeyResult].self, forKey: .krs)
    }
}

extension WorkEvidence where Item == ProjectPlanningSnapshot {
    public var currentProject: ProjectPlanningSnapshot? {
        let current = items.filter { $0.current }
        return current.count == 1 ? current[0] : nil
    }
}

public struct PlanningKeyResult: Decodable, Sendable, Identifiable, Hashable {
    public var id: String { text }

    public let text: String
    public let holds: Bool
}

public struct TrackerState: Decodable, Sendable, Hashable {
    public let state: State
    public let syncedAt: Int64?

    public enum State: String, Decodable, Sendable {
        case available, invalid, removed, absent, unavailable
    }

    enum CodingKeys: String, CodingKey {
        case state
        case syncedAt = "synced_at"
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        state = try values.decode(State.self, forKey: .state)
        syncedAt = try values.decode(Int64?.self, forKey: .syncedAt)
    }
}

public struct TaskPlanningSnapshot: Decodable, Sendable, Identifiable, Hashable {
    public let trackerState: TrackerState
    public let sync: TrackerSync
    public let id: String
    public let repositoryId: String
    public let repo: String
    public let projectId: String?
    public internal(set) var waveId: String?
    public let trackerId: String?
    /// The Task this one was filed from.
    public let parentId: String?
    public let identifier: String
    public let name: String
    public let description: String
    public let rank: UInt32
    public let state: String?
    public let completedAt: String?
    public let dueDate: String?
    public let assignee: String?

    enum CodingKeys: String, CodingKey {
        case repositoryId = "repo_id", projectId = "project_id", waveId = "wave_id"
        case repo
        case id, identifier, name, description, rank, state, assignee, sync
        case trackerState = "tracker_state"
        case trackerId = "tracker_id"
        case parentId = "parent_id"
        case completedAt = "completed_at"
        case dueDate = "due_date"
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        trackerState = try values.decode(TrackerState.self, forKey: .trackerState)
        sync = try values.decode(TrackerSync.self, forKey: .sync)
        id = try values.decode(String.self, forKey: .id)
        repositoryId = try values.decode(String.self, forKey: .repositoryId)
        repo = try values.decode(String.self, forKey: .repo)
        projectId = try values.decode(String?.self, forKey: .projectId)
        waveId = try values.decode(String?.self, forKey: .waveId)
        trackerId = try values.decode(String?.self, forKey: .trackerId)
        parentId = try values.decode(String?.self, forKey: .parentId)
        identifier = try values.decode(String.self, forKey: .identifier)
        name = try values.decode(String.self, forKey: .name)
        description = try values.decode(String.self, forKey: .description)
        rank = try values.decode(UInt32.self, forKey: .rank)
        state = try values.decode(String?.self, forKey: .state)
        completedAt = try values.decode(String?.self, forKey: .completedAt)
        dueDate = try values.decode(String?.self, forKey: .dueDate)
        assignee = try values.decode(String?.self, forKey: .assignee)
    }

    public var isSuccessful: Bool { state == "completed" }
}

public struct TaskRuntimeSnapshot: Decodable, Sendable, Hashable {
    public let workId: String
    public let status: TaskState
    /// A durable completion request awaiting settlement.
    public let completionPending: String?
    public let reason: String
    public let updatedAt: String
    public let provider: String?
    /// Who owns the Task on which Machine; absent when it is unowned.
    public internal(set) var owner: TaskOwner?

    enum CodingKeys: String, CodingKey {
        case status, reason, provider, owner
        case workId = "work_id"
        case completionPending = "completion_pending"
        case updatedAt = "updated_at"
    }
}

/// A Machine for display. A missing id is an unknown Machine.
public struct MachineRef: Codable, Sendable, Hashable {
    public let id: String?
    public let label: String?
}

/// The person and Machine that own a Task.
public struct TaskOwner: Codable, Sendable, Hashable {
    public let person: String?
    /// The owner's lowercase Git email: the display fallback when they have no
    /// name, not their identity.
    public let personKey: String?
    /// The owner's `people` id, which says who they are; nil when the claim
    /// named none.
    public let personId: String?
    public let machine: MachineRef

    enum CodingKeys: String, CodingKey {
        case person, machine
        case personKey = "person_key"
        case personId = "person_id"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        person = try container.decodeIfPresent(String.self, forKey: .person)
        personKey = try container.decodeIfPresent(String.self, forKey: .personKey)
        personId = try container.decode(String?.self, forKey: .personId)
        machine = try container.decode(MachineRef.self, forKey: .machine)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encodeIfPresent(person, forKey: .person)
        try container.encodeIfPresent(personKey, forKey: .personKey)
        try container.encode(personId, forKey: .personId)
        try container.encode(machine, forKey: .machine)
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

/// Where a Task sits on the Wave board, derived once in Rust: the board, the
/// sidebar's Working Tasks and every count read it.
public enum TaskSection: String, Decodable, Sendable, Hashable {
    case backlog, working, paused, done, abandoned
}

public struct RoadmapTask: Decodable, Sendable, Identifiable, Hashable {
    public var id: String { task.id }

    public internal(set) var task: TaskPlanningSnapshot
    public let reference: TaskReferenceSnapshot
    /// Effective lifecycle, including planning withdrawal of never-started work.
    public internal(set) var lifecycle: TaskState
    /// The same states Rust uses for Task selections and conversation activity.
    public let selectionStates: Set<WorkSelectionState>
    public internal(set) var runtime: TaskRuntimeSnapshot?
    public let nextMove: WorkNextMove
    public let condition: TaskConditionSnapshot
    public let actions: TaskActionModel
    public let graphName: String?
    public let graphSource: String?
    public let latestGraphProcess: GraphProcessDetail?
    public let execution: TaskExecutionSnapshot?
    public let runControl: TaskRunControl
    public let pr: PrSnapshot?
    public internal(set) var section: TaskSection
    /// Whether its owner is you, decided in Rust; false when nobody owns it.
    public let ownedByYou: Bool

    enum CodingKeys: String, CodingKey {
        case task, reference, runtime, condition, actions, execution, section, pr, lifecycle
        case selectionStates = "selection_states"
        case ownedByYou = "owned_by_you"
        case graphName = "graph_name"
        case graphSource = "graph_source"
        case latestGraphProcess = "latest_graph_process"
        case runControl = "run_control"
        case nextMove = "next_move"
    }

    /// A started Task nobody owns: nothing moves it until someone claims it.
    public var isUnassigned: Bool {
        guard let runtime else { return false }
        return runtime.owner == nil && lifecycle != .unstarted && !lifecycle.isTerminal
    }

    // What a Task's Session slot does follows its state alone, never its owner:
    // unstarted offers Start, working opens by itself, finished opens when asked.

    /// Not yet started and still open: it has no checkout, so no conversation to
    /// open, and its Session slot offers Start instead.
    public var awaitsStart: Bool { lifecycle == .unstarted }

    /// Whether a Graph has run for the Task. A stopped Graph still counts: a
    /// Task parked at a step has run, one that was only opened has not.
    public var graphHasRun: Bool { latestGraphProcess != nil }

    /// Whether the Task needs someone, and how: nil when its condition is clear,
    /// when it waits only for its first run, which its row offers as Start, or
    /// when it is finished and so waits for nobody.
    public var need: TaskConditionState? {
        switch condition.state {
        case .clear: nil
        case .waiting: isCompleted || (!graphHasRun && actions.recommended == .run) ? nil : .waiting
        case .blocked: .blocked
        case .unknown: .unknown
        }
    }

    /// Where a Task that never ran its Graph stands, as the server reads it:
    /// starting that Graph, or why its setup stopped.
    public var setupPosition: String? {
        guard lifecycle != .paused, let execution, [.starting, .blocked].contains(execution.state) else { return nil }
        return execution.reason
    }

    public var isCompleted: Bool { lifecycle.isTerminal }

    /// Working Tasks open their conversation without being asked, owned or not:
    /// opening claims nothing. Unstarted and finished Tasks open one only when a
    /// person asks, or when Desktop already shows its Session.
    public var opensConversationAutomatically: Bool { lifecycle.isWorking || lifecycle == .paused }
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
    case run
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

    /// Whether more items exist than were read.
    public var truncated: Bool {
        if case let .available(_, truncated) = self { return truncated }
        return false
    }

    /// The items that pass `isIncluded`; unavailable evidence stays unavailable.
    public func filter(_ isIncluded: (Item) -> Bool) -> WorkEvidence<Item> {
        guard case let .available(items, truncated) = self else { return self }
        return .available(items: items.filter(isIncluded), truncated: truncated)
    }
}
