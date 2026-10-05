import Foundation

/// Rust owns checkout association; membership grants no execution authority.
public struct TaskWork: Codable, Sendable, Equatable {
    public let sessions: [TaskSession]
    public let flows: [TaskFlowMember]
    public let execs: [Exec]
}

public struct TaskSession: Codable, Sendable, Equatable, Identifiable {
    public let id: String
    public let title: String
    public let kind: String
    public let interactive: Bool
    /// Driver Exec of the Flow whose step opened the current input.
    public let flowId: String?
    public let completedAt: Int64?

    enum CodingKeys: String, CodingKey {
        case id, title, kind, interactive
        case flowId = "flow_id"
        case completedAt = "completed_at"
    }
}

/// One Flow as its driver Exec records it; `id` is that Exec.
public struct TaskFlowMember: Codable, Sendable, Equatable, Identifiable {
    public let id: String
    public let name: String
    public let state: TaskFlowState
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
public enum TaskFlowState: String, Codable, Sendable, Equatable {
    case current, completed, stopped
}
