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
    public let flowSessionId: String?
    public let completedAt: Int64?

    enum CodingKeys: String, CodingKey {
        case id, title, kind, interactive
        case flowSessionId = "flow_session_id"
        case completedAt = "completed_at"
    }
}

public struct TaskFlowMember: Codable, Sendable, Equatable, Identifiable {
    public let id: String
    public let name: String?
    public let state: String
    public let currentCapture: Int64?
    public let pendingSession: String?
    public let taskId: String?
    public let waveId: String?
    public let updatedAt: Int64
    public let repo: String?
    public let endedAt: Int64?

    enum CodingKeys: String, CodingKey {
        case id, name, state, repo
        case currentCapture = "current_capture"
        case pendingSession = "pending_session"
        case taskId = "task_id"
        case waveId = "wave_id"
        case updatedAt = "updated_at"
        case endedAt = "ended_at"
    }
}
