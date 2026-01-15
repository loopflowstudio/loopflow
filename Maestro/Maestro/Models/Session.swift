// Task session model representing a loopflow task execution.

import Foundation

struct TaskSession: Identifiable, Codable, Hashable {
    let id: String
    let task: String
    let repo: String
    let worktree: String
    let status: String
    let startedAt: Date
    let endedAt: Date?
    let model: String
    let runMode: String

    enum CodingKeys: String, CodingKey {
        case id, task, repo, worktree, status, model
        case startedAt = "started_at"
        case endedAt = "ended_at"
        case runMode = "run_mode"
    }

    var isRunning: Bool {
        status == "running" || status == "waiting"
    }

    var isCompleted: Bool {
        status == "completed"
    }

    var isError: Bool {
        status == "error"
    }

    var relativeTime: String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: startedAt, relativeTo: Date())
    }
}
