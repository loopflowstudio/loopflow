import Foundation

/// Cleanup evidence for a completed Task; its completion remains owned by Work.
public struct TaskRetirement: Codable, Sendable, Equatable {
    public let taskID: String
    public let completion: String
    public let checkout: String
    public let branch: String
    public let dueAt: Int64?
    public let phase: String
    public let reason: String?

    enum CodingKeys: String, CodingKey {
        case taskID = "task_id", completion, checkout, branch, dueAt = "due_at", phase, reason
    }
}
