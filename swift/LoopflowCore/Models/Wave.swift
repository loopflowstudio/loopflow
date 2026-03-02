// Wave - an autonomous AI coding wave.

import Foundation
import SwiftUI

/// Determines when a wave fires. Stored in the triggers table — existence means active.
public struct Trigger: Sendable, Hashable, Codable, Identifiable {
    public enum Signal: String, Sendable, Codable, CaseIterable {
        case repo
        case wave
        case ciFailure = "ci_failure"

        public var icon: String {
            switch self {
            case .repo: return "arrow.triangle.branch"
            case .wave: return "waveform"
            case .ciFailure: return "exclamationmark.triangle"
            }
        }

        public var label: String {
            switch self {
            case .repo: return "Repo"
            case .wave: return "Wave"
            case .ciFailure: return "CI Fix"
            }
        }
    }

    public var id: String
    public var signal: Signal
    public var enabled: Bool
    public var flow: String?
    public var sourceWaveId: String?

    public init(id: String = UUID().uuidString, signal: Signal, enabled: Bool = true, flow: String? = nil, sourceWaveId: String? = nil) {
        self.id = id
        self.signal = signal
        self.enabled = enabled
        self.flow = flow
        self.sourceWaveId = sourceWaveId
    }

    public var description: String {
        if let flow = flow {
            return "\(signal.rawValue) → \(flow)"
        }
        return signal.rawValue
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        signal = try container.decode(Signal.self, forKey: .signal)
        enabled = try container.decodeIfPresent(Bool.self, forKey: .enabled) ?? true
        flow = try container.decodeIfPresent(String.self, forKey: .flow)
        sourceWaveId = try container.decodeIfPresent(String.self, forKey: .sourceWaveId)
    }

    public var icon: String { signal.icon }

    public var label: String { signal.label }
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
        case .idle: return .statusNeutral
        case .failed: return .statusError
        case .paused: return .statusNeutral
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
    /// Runs the step, then auto-commits and pushes when the agent exits.
    public var command: String {
        var cmd = "lf \(step)"
        if let prompt = prompt {
            cmd += " \(shellEscape(prompt))"
        }
        cmd += " && lf ops commit --push"
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
    public var agent: String?
    public var stepAgents: [String: String]?
    public var triggers: [Trigger]
    public var status: WaveStatus
    public var iteration: Int
    public var localWorktree: String?
    public var remoteBranch: String?
    public var commits: [CommitEntry]
    public var diffStat: String?
    public var flowSteps: [String]
    public var openPRCount: Int
    public var activeRun: WaveRun?
    public var createdAt: Date?

    public init(
        id: String,
        name: String = "",
        repo: String,
        flow: String = "",
        direction: [String] = [],
        area: [String] = [],
        agent: String? = nil,
        stepAgents: [String: String]? = nil,
        triggers: [Trigger] = [],
        status: WaveStatus = .idle,
        iteration: Int = 0,
        localWorktree: String? = nil,
        remoteBranch: String? = nil,
        commits: [CommitEntry] = [],
        diffStat: String? = nil,
        flowSteps: [String] = [],
        openPRCount: Int = 0,
        activeRun: WaveRun? = nil,
        createdAt: Date? = nil
    ) {
        self.id = id
        self.name = name
        self.repo = repo
        self.flow = flow
        self.direction = direction
        self.area = area
        self.agent = agent
        self.stepAgents = stepAgents
        self.triggers = triggers
        self.status = status
        self.iteration = iteration
        self.localWorktree = localWorktree
        self.remoteBranch = remoteBranch
        self.commits = commits
        self.diffStat = diffStat
        self.flowSteps = flowSteps
        self.openPRCount = openPRCount
        self.activeRun = activeRun
        self.createdAt = createdAt
    }
}
