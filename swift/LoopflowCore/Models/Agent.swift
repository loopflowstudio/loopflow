// Agent - an autonomous AI coding agent.
// Stimulus types: manual (interactive), once (single run), loop (continuous), watch (on file change), cron (scheduled).

import Foundation
import SwiftUI

/// Determines when an agent runs.
public struct Stimulus: Sendable, Hashable, Codable {
    public enum Kind: String, Sendable, Codable {
        case manual  // User-triggered, interactive work
        case once
        case loop
        case watch
        case cron
    }

    public var kind: Kind
    public var cron: String?

    public init(kind: Kind, cron: String? = nil) {
        self.kind = kind
        self.cron = cron
    }

    public var description: String {
        if kind == .cron, let cronExpr = cron {
            return "cron(\(cronExpr))"
        }
        return kind.rawValue
    }

    public var icon: String {
        switch kind {
        case .manual: return "circle"  // ○ Idle
        case .once: return "play.circle"
        case .loop: return "circle.fill"  // ● Running
        case .watch: return "eye.circle"
        case .cron: return "clock"  // ◷ Scheduled
        }
    }
}

public enum AgentStatus: String, Sendable, Codable {
    case idle
    case running
    case waiting
    case completed
    case error

    public var color: Color {
        switch self {
        case .running: return .green
        case .waiting: return .yellow
        case .idle: return .gray
        case .completed: return .green
        case .error: return .red
        }
    }

    public var icon: String {
        switch self {
        case .running: return "circle.fill"  // ●
        case .waiting: return "circle.lefthalf.filled"  // ◐
        case .idle: return "circle"  // ○
        case .completed: return "checkmark.circle.fill"  // ✓
        case .error: return "xmark.circle.fill"  // ✗
        }
    }
}

public enum MergeMode: String, Sendable, Codable {
    case pr
    case land
}

/// An interactive session running in the embedded terminal.
public struct InteractiveSession: Sendable, Identifiable {
    public let id: String
    public let agentId: String
    public let step: String
    public let worktreePath: String
    public let startedAt: Date

    public init(
        id: String = UUID().uuidString,
        agentId: String,
        step: String,
        worktreePath: String,
        startedAt: Date = Date()
    ) {
        self.id = id
        self.agentId = agentId
        self.step = step
        self.worktreePath = worktreePath
        self.startedAt = startedAt
    }
}

public struct Agent: Sendable, Identifiable, Hashable {
    public let id: String
    public var name: String  // User-visible name (e.g., "swift-falcon")
    public var area: [String]?  // Optional, validated at run-time
    public var goal: [String]?  // Optional, validated at run-time
    public var flow: String
    public let repo: String

    public var stimulus: Stimulus
    public var paused: Bool  // When true, stimulus doesn't fire (manual mode)
    public var status: AgentStatus
    public var iteration: Int

    // Hidden implementation details
    public var worktreePath: String?  // Path to the worktree (renamed from mainBranch)
    public var branch: String?  // Git branch (auto-generated)

    public var prLimit: Int
    public var mergeMode: MergeMode
    public var pid: Int?
    public var createdAt: Date

    // Watch state
    public var lastMainSha: String?

    public init(
        id: String,
        name: String = "",
        area: [String]? = nil,
        goal: [String]? = nil,
        flow: String = "design",
        repo: String,
        stimulus: Stimulus = Stimulus(kind: .once),
        paused: Bool = true,
        status: AgentStatus = .idle,
        iteration: Int = 0,
        worktreePath: String? = nil,
        branch: String? = nil,
        prLimit: Int = 5,
        mergeMode: MergeMode = .pr,
        pid: Int? = nil,
        createdAt: Date = Date(),
        lastMainSha: String? = nil
    ) {
        self.id = id
        self.name = name
        self.area = area
        self.goal = goal
        self.flow = flow
        self.repo = repo
        self.stimulus = stimulus
        self.paused = paused
        self.status = status
        self.iteration = iteration
        self.worktreePath = worktreePath
        self.branch = branch
        self.prLimit = prLimit
        self.mergeMode = mergeMode
        self.pid = pid
        self.createdAt = createdAt
        self.lastMainSha = lastMainSha
    }

    /// Check if agent has required config for running.
    public var isConfigured: Bool {
        area != nil
    }


    public var shortId: String { String(id.prefix(7)) }

    /// User-visible display name. Uses the name if set, otherwise generates from area/flow.
    public var displayName: String {
        if !name.isEmpty { return name }
        return generateNameFromInput()
    }

    private func generateNameFromInput() -> String {
        // Generate a display name from the agent's configuration
        let areaStr: String
        if let firstArea = area?.first {
            areaStr = firstArea == "." ? "root" : firstArea
        } else {
            areaStr = "root"
        }
        return "\(areaStr) · \(flow.isEmpty ? "default" : flow)"
    }

    public var areaDisplay: String {
        guard let area = area else { return "" }
        return area.first == "." ? "." : area.joined(separator: ", ")
    }

    public var goalDisplay: String {
        guard let goal = goal else { return "" }
        return goal.isEmpty ? "" : goal.joined(separator: ", ")
    }

    public var flowDisplay: String {
        flow.isEmpty ? "ship" : flow
    }

    public var statusText: String {
        switch status {
        case .running: return "Running"
        case .waiting: return "Waiting"
        case .idle: return "Idle"
        case .completed: return "Completed"
        case .error: return "Error"
        }
    }

    public var iterationText: String {
        iteration > 0 ? "iter \(iteration)" : ""
    }

    public var detailText: String {
        var parts: [String] = []
        if !areaDisplay.isEmpty { parts.append(areaDisplay) }
        if !flowDisplay.isEmpty { parts.append(flowDisplay) }
        if stimulus.kind != .manual { parts.append(stimulus.kind.rawValue) }
        return parts.joined(separator: " · ")
    }

    public var stimulusText: String {
        stimulus.description
    }

    /// Status indicator combining status and stimulus
    public var statusIndicator: (icon: String, color: Color) {
        switch status {
        case .running:
            return ("circle.fill", .green)
        case .waiting:
            return ("circle.lefthalf.filled", .yellow)
        case .error:
            return ("xmark.circle.fill", .red)
        case .completed:
            return ("checkmark.circle.fill", .green)
        case .idle:
            if stimulus.kind == .cron {
                return ("clock", .gray)
            }
            return ("circle", .gray)
        }
    }
}
