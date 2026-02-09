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
    case failed
    case paused

    public var color: Color {
        switch self {
        case .running: return .statusSuccess
        case .waiting: return .statusWarning
        case .idle: return .gray
        case .failed: return .statusError
        case .paused: return .gray
        }
    }

    public var icon: String {
        switch self {
        case .running: return "circle.fill"
        case .waiting: return "circle.lefthalf.filled"
        case .idle: return "circle"
        case .failed: return "xmark.circle.fill"
        case .paused: return "pause.circle"
        }
    }
}

public enum MergeMode: String, Sendable, Codable {
    case pr
    case land
}

public enum WaitingReason: Sendable, Hashable {
    case prLimitReached(open: Int, limit: Int)

    public var description: String {
        switch self {
        case .prLimitReached(let open, let limit):
            return "\(open)/\(limit) PRs open"
        }
    }

    public var accessibilityDescription: String {
        switch self {
        case .prLimitReached(let open, let limit):
            return "\(open) of \(limit) PRs open"
        }
    }
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

public struct CommitEntry: Sendable, Hashable, Identifiable {
    public let sha: String
    public let message: String

    public var id: String { sha }

    public init(sha: String, message: String) {
        self.sha = sha
        self.message = message
    }
}

public struct Wave: Sendable, Identifiable, Hashable {
    public let id: String
    public var name: String
    public var repo: String
    public var flow: String
    public var direction: [String]
    public var area: [String]
    public var stimulus: Stimulus
    public var status: WaveStatus
    public var iteration: Int
    public var localWorktree: String?
    public var remoteBranch: String?
    public var commits: [CommitEntry]
    public var diffStat: String?
    public var flowSteps: [String]
    public var activeRun: WaveRun?
    public var createdAt: Date?

    public init(
        id: String,
        name: String = "",
        repo: String,
        flow: String = "",
        direction: [String] = [],
        area: [String] = [],
        stimulus: Stimulus = Stimulus(kind: .once),
        status: WaveStatus = .idle,
        iteration: Int = 0,
        localWorktree: String? = nil,
        remoteBranch: String? = nil,
        commits: [CommitEntry] = [],
        diffStat: String? = nil,
        flowSteps: [String] = [],
        activeRun: WaveRun? = nil,
        createdAt: Date? = nil
    ) {
        self.id = id
        self.name = name
        self.repo = repo
        self.flow = flow
        self.direction = direction
        self.area = area
        self.stimulus = stimulus
        self.status = status
        self.iteration = iteration
        self.localWorktree = localWorktree
        self.remoteBranch = remoteBranch
        self.commits = commits
        self.diffStat = diffStat
        self.flowSteps = flowSteps
        self.activeRun = activeRun
        self.createdAt = createdAt
    }
}
