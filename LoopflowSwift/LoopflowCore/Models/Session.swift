// Task session model representing a loopflow task execution.

import Foundation

public struct TaskSession: Sendable, Identifiable, Codable, Hashable {
    public let id: String
    public let task: String
    public let repo: String
    public let worktree: String
    public let status: String
    public let startedAt: Date
    public let endedAt: Date?
    public let model: String
    public let runMode: String

    enum CodingKeys: String, CodingKey {
        case id, task, repo, worktree, status, model
        case startedAt = "started_at"
        case endedAt = "ended_at"
        case runMode = "run_mode"
    }

    public var isRunning: Bool {
        status == "running" || status == "waiting"
    }

    public var isCompleted: Bool {
        status == "completed"
    }

    public var isError: Bool {
        status == "error"
    }

    public var relativeTime: String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: startedAt, relativeTo: Date())
    }
}
