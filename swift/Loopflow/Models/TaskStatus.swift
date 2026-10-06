import Foundation

/// Planning remains inspectable before execution has been allocated.
public struct TaskStatus: Decodable, Sendable {
    public let planning: TaskPlanningRecord?
    public let planningError: String?
    public let planningState: PlanningState
    public let planningStale: Bool
    public let execution: TaskStatusExecution?

    enum CodingKeys: String, CodingKey {
        case planning, execution
        case planningError = "planning_error"
        case planningState = "planning_state"
        case planningStale = "planning_stale"
    }
}

public enum PlanningState: String, Decodable, Sendable {
    case available, invalid, removed, absent, unavailable
}

public struct TaskPlanningRecord: Decodable, Sendable {
    public let item: PlanningItem
    public let project: PlanningProject?
    public let observedAt: Int64

    enum CodingKeys: String, CodingKey {
        case item, project
        case observedAt = "observed_at"
    }
}

public struct PlanningItem: Decodable, Sendable {
    public let id: String
    public let revision: String?
    public let identifier: String
    public let branchName: String?
    public let url: String?
    public let name: String
    public let description: String
    public let rank: UInt32
    public let completedAt: String?
    public let completed: Bool
    public let state: String?
    public let projectId: String?
    public let project: String?
    public let teamId: String
    public let assignee: String?

    enum CodingKeys: String, CodingKey {
        case id, revision, identifier, url, name, description, rank, completed, state, project, assignee
        case completedAt = "completed_at"
        case branchName = "branch_name"
        case projectId = "project_id"
        case teamId = "team_id"
    }
}

public struct PlanningProject: Decodable, Sendable {
    public let id: String
    public let revision: String?
    public let slug: String
    public let name: String
    public let summary: String
    public let metricTargets: [ChapterMetricTarget]
    public let workflow: String
    public let status: ProjectStatus
    public let krs: [PlanningKeyResult]
    public let initiativeIds: [String]
    public let teamIds: [String]

    enum CodingKeys: String, CodingKey {
        case id, revision, slug, name, summary, workflow, status, krs
        case metricTargets = "metric_targets"
        case initiativeIds = "initiative_ids"
        case teamIds = "team_ids"
    }
}

/// Execution fields used to locate work and present its next action.
public struct TaskStatusExecution: Decodable, Sendable {
    public let taskId: String
    public let issueId: String
    public let issueIdentifier: String
    public let status: WorkStatus
    public let worktree: String
    public let workspaceSlug: String
    public let actions: TaskActionModel

    enum CodingKeys: String, CodingKey {
        case status, worktree, actions
        case taskId = "task_id"
        case issueId = "issue_id"
        case issueIdentifier = "issue_identifier"
        case workspaceSlug = "workspace_slug"
    }
}
