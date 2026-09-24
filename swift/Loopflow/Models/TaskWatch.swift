import Foundation

/// Rust projects persisted flow facts. These describe recorded progress, never
/// provider liveness or Session actions. Output availability comes from taskOutput.
public struct TaskWatchSnapshot: Decodable, Sendable {
    public let taskId: String
    public let activeStage: TaskFlowStage?
    public let invocations: [TaskWatchInvocation]
    public let runs: [TaskWatchRun]
    public let gaps: [OutputGap]

    enum CodingKeys: String, CodingKey {
        case taskId = "task_id"
        case activeStage = "active_stage"
        case invocations, runs, gaps
    }
}

public struct TaskWatchInvocation: Decodable, Sendable, Identifiable {
    public let id: String
    public let flow: String
    public let stages: [TaskWatchStage]
    public let transitions: [TaskWatchTransition]
    public let settlement: TaskFlowSettlement?
}

public enum TaskFlowSettlement: String, Decodable, Sendable {
    case completed, approved, replaced, reopened
}

public struct TaskWatchStage: Decodable, Sendable {
    public let stepIndex: UInt32
    public let name: String
    public let kind: PlayheadStepKind
    public let nodeId: String?
    public let human: Bool
    public let flowParents: [String]
    public let attempts: [TaskWatchAttempt]

    enum CodingKeys: String, CodingKey {
        case stepIndex = "step_index"
        case nodeId = "node_id"
        case flowParents = "flow_parents"
        case name, kind, human, attempts
    }
}

public enum TaskWatchAttemptState: String, Decodable, Sendable {
    case entered, bound, ready, blocked, completed, iterated
}

public struct TaskWatchAttempt: Decodable, Sendable {
    public let iteration: UInt32
    public let runId: String?
    public let state: TaskWatchAttemptState
    public let readySummary: String?
    public let failure: TaskFlowBlocker?

    enum CodingKeys: String, CodingKey {
        case runId = "run_id"
        case readySummary = "ready_summary"
        case iteration, state, failure
    }
}

public struct TaskFlowBlocker: Decodable, Sendable {
    public let reason: String
    public let restartRequired: Bool
    public let observedAt: String

    enum CodingKeys: String, CodingKey {
        case restartRequired = "restart_required"
        case observedAt = "observed_at"
        case reason
    }
}

public struct TaskWatchTransition: Decodable, Sendable {
    public let from: TaskFlowStage
    public let to: TaskFlowStage
    public let reason: TaskFlowTransition
}

public enum TaskFlowTransition: String, Decodable, Sendable {
    case advanced, approved, iterated, retried
}

public struct TaskWatchRun: Decodable, Sendable {
    public let runId: String
    public let provider: String?
    public let stage: TaskFlowStage?

    enum CodingKeys: String, CodingKey {
        case runId = "run_id"
        case provider, stage
    }
}
