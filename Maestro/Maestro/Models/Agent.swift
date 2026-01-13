// Agent model for background agent loops.
// Reads definitions from ~/.lf/agents/*.md (markdown with YAML frontmatter)
// and runtime state from ~/.lf/maestro.db

import Foundation

enum AgentStatus: String, Codable {
    case idle
    case running
    case waiting
    case error
    case stopped
}

struct Agent: Identifiable, Hashable {
    let id: String  // filename without .md
    let name: String
    let repo: String
    let pipeline: [String]
    let trigger: String
    let context: [String]
    let prompt: String
    let intervalSeconds: Int?

    // Runtime state (from DB)
    var status: AgentStatus = .idle
    var lastRunAt: Date?
    var iteration: Int = 0
    var pid: Int?

    var statusText: String {
        switch status {
        case .running:
            return "Running"
        case .waiting:
            return "Waiting"
        case .idle:
            return "Idle"
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
        case .error:
            return "red"
        case .stopped:
            return "orange"
        }
    }

    var pipelineText: String {
        pipeline.joined(separator: " -> ")
    }

    var triggerText: String {
        if trigger == "interval", let seconds = intervalSeconds {
            return "interval (\(seconds)s)"
        }
        return trigger
    }

    var lastRunText: String? {
        guard let date = lastRunAt else { return nil }
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}
