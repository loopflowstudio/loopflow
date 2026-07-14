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

/// An interactive session running in the embedded terminal.
public struct InteractiveSession: Sendable, Identifiable {
    public let id: String
    public let waveId: String
    public let skill: String
    public let worktreePath: String
    public let prompt: String?
    public let startedAt: Date

    public init(
        id: String = UUID().uuidString,
        waveId: String,
        skill: String,
        worktreePath: String,
        prompt: String? = nil,
        startedAt: Date = Date()
    ) {
        self.id = id
        self.waveId = waveId
        self.skill = skill
        self.worktreePath = worktreePath
        self.prompt = prompt
        self.startedAt = startedAt
    }

    /// Build the shell command to run this session.
    /// Runs the skill, then auto-commits and pushes when the agent exits.
    public var command: String {
        var cmd = "lf \(skill)"
        if let prompt = prompt {
            cmd += " \(shellEscape(prompt))"
        }
        cmd += " && lf commit --push"
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

/// A durable control plane for one repository. Projects and Tasks carry the
/// shipping state; a Wave itself has no worktree, branch, diff, or PR.
public struct Wave: Sendable, Identifiable, Hashable {
    public let id: String
    public var name: String
    public var goal: String
    public var metrics: [String]
    public var direction: [String]
    public var area: [String]
    public var agent: String?
    public var skillAgents: [String: String]?
    public var triggers: [Trigger]
    public var crons: [WaveCron]
    public var status: WaveStatus
    /// The single repository whose main checkout is this Wave's control plane.
    public var repo: String
    public var iteration: Int
    public var createdAt: Date?
    /// Parent wave in the chord tree. `nil` for a root wave.
    public var parentWaveId: String?

    public init(
        id: String,
        name: String = "",
        repo: String,
        goal: String = "ship-roadmap",
        metrics: [String] = [],
        direction: [String] = [],
        area: [String] = [],
        agent: String? = nil,
        skillAgents: [String: String]? = nil,
        triggers: [Trigger] = [],
        crons: [WaveCron] = [],
        status: WaveStatus = .idle,
        iteration: Int = 0,
        createdAt: Date? = nil,
        parentWaveId: String? = nil
    ) {
        self.id = id
        self.name = name
        self.repo = repo
        self.goal = goal
        self.metrics = metrics
        self.direction = direction
        self.area = area
        self.agent = agent
        self.skillAgents = skillAgents
        self.triggers = triggers
        self.crons = crons
        self.status = status
        self.iteration = iteration
        self.createdAt = createdAt
        self.parentWaveId = parentWaveId
    }
}
