// Wave - an autonomous AI coding wave.
// Stimulus types: manual (interactive), once (single run), loop (continuous), watch (on file change), cron (scheduled).

import Foundation
import SwiftUI

/// Determines when a wave runs.
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
        case .manual: return "circle"  // Idle
        case .once: return "play.circle"
        case .loop: return "circle.fill"  // Running
        case .watch: return "eye.circle"
        case .cron: return "clock"  // Scheduled
        }
    }
}

public enum WaveStatus: String, Sendable, Codable {
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
        case .running: return "circle.fill"
        case .waiting: return "circle.lefthalf.filled"
        case .idle: return "circle"
        case .completed: return "checkmark.circle.fill"
        case .error: return "xmark.circle.fill"
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
    public let waveId: String
    public let step: String
    public let worktreePath: String
    public let prompt: String?
    public let startedAt: Date

    public init(
        id: String = UUID().uuidString,
        waveId: String,
        step: String,
        worktreePath: String,
        prompt: String? = nil,
        startedAt: Date = Date()
    ) {
        self.id = id
        self.waveId = waveId
        self.step = step
        self.worktreePath = worktreePath
        self.prompt = prompt
        self.startedAt = startedAt
    }

    /// Build the shell command to run this session.
    public var command: String {
        var cmd = "lf \(step)"
        if let prompt = prompt {
            cmd += " \(shellEscape(prompt))"
        }
        return cmd
    }
}

/// Shell-escape a string for bash/zsh by wrapping in single quotes.
public func shellEscape(_ string: String) -> String {
    // For bash/zsh: wrap in single quotes, escape internal single quotes
    // 'foo' -> 'foo'
    // foo's -> 'foo'\''s'
    let escaped = string.replacingOccurrences(of: "'", with: "'\\''")
    return "'\(escaped)'"
}

public struct Wave: Sendable, Identifiable, Hashable {
    public let id: String
    public var name: String  // User-visible name (e.g., "swift-falcon")
    public var area: [String]?  // Optional, validated at run-time
    public var direction: [String]?  // Optional, validated at run-time
    public var flow: String
    public let repo: String

    public var stimulus: Stimulus
    public var paused: Bool  // When true, stimulus doesn't fire (manual mode)
    public var status: WaveStatus
    public var iteration: Int

    // Worktree (waves always have worktrees)
    public var worktreePath: String?  // Path to the worktree
    public var branch: String?  // Git branch (auto-generated)

    // Git status (enriched from daemon)
    public var isDirty: Bool
    public var isRebasing: Bool
    public var isMerging: Bool
    public var hasDiff: Bool
    public var aheadMain: Int
    public var behindMain: Int
    public var aheadRemote: Int
    public var behindRemote: Int

    // PR status
    public var prURL: URL?
    public var prNumber: Int?
    public var prState: PRState?

    // Staleness
    public var staleness: Staleness
    public var recentSteps: [StepRun]

    // Config
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
        direction: [String]? = nil,
        flow: String = "design",
        repo: String,
        stimulus: Stimulus = Stimulus(kind: .once),
        paused: Bool = true,
        status: WaveStatus = .idle,
        iteration: Int = 0,
        worktreePath: String? = nil,
        branch: String? = nil,
        isDirty: Bool = false,
        isRebasing: Bool = false,
        isMerging: Bool = false,
        hasDiff: Bool = false,
        aheadMain: Int = 0,
        behindMain: Int = 0,
        aheadRemote: Int = 0,
        behindRemote: Int = 0,
        prURL: URL? = nil,
        prNumber: Int? = nil,
        prState: PRState? = nil,
        staleness: Staleness = .active,
        recentSteps: [StepRun] = [],
        prLimit: Int = 5,
        mergeMode: MergeMode = .pr,
        pid: Int? = nil,
        createdAt: Date = Date(),
        lastMainSha: String? = nil
    ) {
        self.id = id
        self.name = name
        self.area = area
        self.direction = direction
        self.flow = flow
        self.repo = repo
        self.stimulus = stimulus
        self.paused = paused
        self.status = status
        self.iteration = iteration
        self.worktreePath = worktreePath
        self.branch = branch
        self.isDirty = isDirty
        self.isRebasing = isRebasing
        self.isMerging = isMerging
        self.hasDiff = hasDiff
        self.aheadMain = aheadMain
        self.behindMain = behindMain
        self.aheadRemote = aheadRemote
        self.behindRemote = behindRemote
        self.prURL = prURL
        self.prNumber = prNumber
        self.prState = prState
        self.staleness = staleness
        self.recentSteps = recentSteps
        self.prLimit = prLimit
        self.mergeMode = mergeMode
        self.pid = pid
        self.createdAt = createdAt
        self.lastMainSha = lastMainSha
    }

    /// Check if wave has required config for running.
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
        // Generate a display name from the wave's configuration
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

    public var directionDisplay: String {
        guard let direction = direction else { return "" }
        return direction.isEmpty ? "" : direction.joined(separator: ", ")
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
