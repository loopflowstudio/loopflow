import Foundation

/// The retaining host process, on the Machine whose kernel can observe its exit.
public struct TaskHostOwner: Codable, Sendable, Equatable {
    public let machineID: String
    public let pid: UInt32
    public let startedAt: Int64
    public let settlesOnExit: Bool

    public init(machineID: String, pid: UInt32, startedAt: Int64, settlesOnExit: Bool) {
        self.machineID = machineID
        self.pid = pid
        self.startedAt = startedAt
        self.settlesOnExit = settlesOnExit
    }

    enum CodingKeys: String, CodingKey {
        case machineID = "machine_id", pid, startedAt = "started_at", settlesOnExit = "settles_on_exit"
    }
}

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
