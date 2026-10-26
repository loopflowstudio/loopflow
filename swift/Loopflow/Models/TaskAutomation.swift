import Foundation

public struct TaskAutomation: Codable, Sendable, Identifiable {
    public var id: String { taskId }
    public let taskId: String
    public let issue: String
    public let enabled: Bool?
    enum CodingKeys: String, CodingKey {
        case taskId = "task_id", issue, enabled
    }
}

public struct AutomationStatus: Codable, Sendable {
    public let tasks: [TaskAutomation]
    public let deliveries: [DeliveryStatus]
}

public struct DeliveryStatus: Codable, Sendable {
    public let prNumber: UInt32
    public let taskId: String?
    public let state: String
    public let detail: String?

    enum CodingKeys: String, CodingKey {
        case prNumber = "pr_number", taskId = "task_id", state, detail
    }
}
