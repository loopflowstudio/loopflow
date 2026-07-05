// Wave - an autonomous AI coding wave.

import Foundation
import SwiftUI

/// Legacy trigger model. The daemon's trigger machinery is gone (webhooks now
/// speak `lf chat` into the wave's thread); this parses leniently to empty and
/// stays only for the retained UI paths.
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
        cmd += " && lf op commit --push"
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

public struct WaveCron: Sendable, Hashable, Codable, Identifiable {
    public let id: String
    public var flow: String
    public var schedule: String
    public var lastTriggeredAt: Date?
    public var createdAt: Date?

    public init(
        id: String = UUID().uuidString,
        flow: String,
        schedule: String,
        lastTriggeredAt: Date? = nil,
        createdAt: Date? = nil
    ) {
        self.id = id
        self.flow = flow
        self.schedule = schedule
        self.lastTriggeredAt = lastTriggeredAt
        self.createdAt = createdAt
    }
}

/// Per-repo execution surface for a wave, mirroring `RepoWorkDto`. One entry
/// per repo the wave runs in. `localWorktree`/`remoteBranch` are git-inferred at
/// build time; `worktree`/`branch` persisted duplicates were dropped from the wire.
public struct RepoWork: Sendable, Hashable {
    public var repo: String
    public var status: WaveStatus
    public var iteration: Int
    public var localWorktree: String?
    public var remoteBranch: String?
    public var commits: [CommitEntry]
    public var diffStat: String?
    public var openPRCount: Int
    public var stackCount: Int
    public var activeRun: Run?
    public var pr: PullRequest?

    public init(
        repo: String,
        status: WaveStatus = .idle,
        iteration: Int = 0,
        localWorktree: String? = nil,
        remoteBranch: String? = nil,
        commits: [CommitEntry] = [],
        diffStat: String? = nil,
        openPRCount: Int = 0,
        stackCount: Int = 0,
        activeRun: Run? = nil,
        pr: PullRequest? = nil
    ) {
        self.repo = repo
        self.status = status
        self.iteration = iteration
        self.localWorktree = localWorktree
        self.remoteBranch = remoteBranch
        self.commits = commits
        self.diffStat = diffStat
        self.openPRCount = openPRCount
        self.stackCount = stackCount
        self.activeRun = activeRun
        self.pr = pr
    }
}

public struct Wave: Sendable, Identifiable, Hashable {
    public let id: String
    public var name: String
    public var flow: String
    public var goal: String
    public var metrics: [String]
    public var direction: [String]
    public var area: [String]
    public var agent: String?
    public var stepAgents: [String: String]?
    public var triggers: [Trigger]
    public var crons: [WaveCron]
    /// Wave-level status rolled up over `repos`.
    public var status: WaveStatus
    public var repos: [RepoWork]
    public var flowSteps: [String]
    public var createdAt: Date?
    /// Parent wave in the chord tree. `nil` for a root wave.
    public var parentWaveId: String?

    public init(
        id: String,
        name: String = "",
        repos: [RepoWork],
        flow: String = "",
        goal: String = "ship-roadmap",
        metrics: [String] = [],
        direction: [String] = [],
        area: [String] = [],
        agent: String? = nil,
        stepAgents: [String: String]? = nil,
        triggers: [Trigger] = [],
        crons: [WaveCron] = [],
        status: WaveStatus = .idle,
        flowSteps: [String] = [],
        createdAt: Date? = nil,
        parentWaveId: String? = nil
    ) {
        self.id = id
        self.name = name
        self.repos = repos
        self.flow = flow
        self.goal = goal
        self.metrics = metrics
        self.direction = direction
        self.area = area
        self.agent = agent
        self.stepAgents = stepAgents
        self.triggers = triggers
        self.crons = crons
        self.status = status
        self.flowSteps = flowSteps
        self.createdAt = createdAt
        self.parentWaveId = parentWaveId
    }

    /// Single-repo convenience mirroring the old flat shape: packs the per-repo
    /// execution fields into one `RepoWork`. Keeps existing call sites (views,
    /// tests, placeholders) compiling unchanged.
    public init(
        id: String,
        name: String = "",
        repo: String,
        flow: String = "",
        goal: String = "ship-roadmap",
        metrics: [String] = [],
        direction: [String] = [],
        area: [String] = [],
        agent: String? = nil,
        stepAgents: [String: String]? = nil,
        triggers: [Trigger] = [],
        crons: [WaveCron] = [],
        status: WaveStatus = .idle,
        iteration: Int = 0,
        localWorktree: String? = nil,
        remoteBranch: String? = nil,
        commits: [CommitEntry] = [],
        diffStat: String? = nil,
        flowSteps: [String] = [],
        openPRCount: Int = 0,
        activeRun: Run? = nil,
        createdAt: Date? = nil
    ) {
        self.init(
            id: id,
            name: name,
            repos: [
                RepoWork(
                    repo: repo,
                    status: status,
                    iteration: iteration,
                    localWorktree: localWorktree,
                    remoteBranch: remoteBranch,
                    commits: commits,
                    diffStat: diffStat,
                    openPRCount: openPRCount,
                    activeRun: activeRun
                )
            ],
            flow: flow,
            goal: goal,
            metrics: metrics,
            direction: direction,
            area: area,
            agent: agent,
            stepAgents: stepAgents,
            triggers: triggers,
            crons: crons,
            status: status,
            flowSteps: flowSteps,
            createdAt: createdAt,
            parentWaveId: nil
        )
    }
}
