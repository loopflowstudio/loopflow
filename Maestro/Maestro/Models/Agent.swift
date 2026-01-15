// Agent model for background agent loops.
// Reads definitions from ~/.lf/agents/*.md (markdown with YAML frontmatter)
// and runtime state from ~/.lf/maestro.db

import Foundation
import SwiftUI

enum AgentStatus: String, Codable {
    case idle
    case running
    case waiting
    case completed
    case error
    case stopped

    var color: Color {
        switch self {
        case .running: return .green
        case .waiting: return .blue
        case .idle: return .gray
        case .completed: return .green
        case .error: return .red
        case .stopped: return .orange
        }
    }
}

enum TriggerKind: String, Codable, CaseIterable {
    case manual
    case mainChanged = "main-changed"
    case loop
    case cron

    var displayName: String {
        switch self {
        case .manual: return "Manual"
        case .mainChanged: return "On main change"
        case .loop: return "Loop"
        case .cron: return "Cron"
        }
    }
}

enum MergeStrategy: String, Codable, CaseIterable {
    case pr
    case auto

    var displayName: String {
        switch self {
        case .pr: return "Open PR"
        case .auto: return "Auto-merge"
        }
    }
}

struct Agent: Identifiable, Hashable {
    let id: String  // filename without .md
    let name: String
    let repo: String
    let pipeline: String
    let triggerKind: TriggerKind
    let context: [String]
    let prompt: String  // Goal text (markdown body after frontmatter)
    let emoji: String
    let cron: String?
    let mergeStrategy: MergeStrategy

    // Runtime state (from DB)
    var status: AgentStatus = .idle
    var lastRunAt: Date?
    var iteration: Int = 0
    var pid: Int?
    var worktree: String?

    var statusText: String {
        switch status {
        case .running:
            return "Running"
        case .waiting:
            return "Waiting"
        case .idle:
            return "Idle"
        case .completed:
            return "Completed"
        case .error:
            return "Error"
        case .stopped:
            return "Stopped"
        }
    }

    var statusColor: String {
        switch status {
        case .running:
            return "green"
        case .waiting:
            return "blue"
        case .idle:
            return "gray"
        case .completed:
            return "green"
        case .error:
            return "red"
        case .stopped:
            return "orange"
        }
    }

    var triggerText: String {
        switch triggerKind {
        case .cron:
            if let cron = cron {
                return "cron(\(cron))"
            }
            return "cron"
        default:
            return triggerKind.displayName
        }
    }

    var displayEmoji: String {
        emoji.isEmpty ? "🤖" : emoji
    }

    var repoName: String {
        URL(fileURLWithPath: repo).lastPathComponent
    }

    var lastRunText: String? {
        guard let date = lastRunAt else { return nil }
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}
