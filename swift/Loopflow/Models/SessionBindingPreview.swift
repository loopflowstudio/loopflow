import Foundation

/// Identity returned by `session bind --dry-run`; assignment is validated on commit.
public struct SessionBindingPreview: Codable, Sendable, Hashable {
    public let sessionId: String
    public let taskId: String
    public let identifier: String
    public let title: String

    enum CodingKeys: String, CodingKey {
        case sessionId = "session_id"
        case taskId = "task_id"
        case identifier, title
    }
}
