// Agent - an autonomous AI coding agent.
// Stimulus types: once (single run), loop (continuous), watch (on file change), cron (scheduled).

import Foundation
import SwiftUI

/// Determines when an agent runs.
public struct Stimulus: Sendable, Hashable, Codable {
    public enum Kind: String, Sendable, Codable {
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
}

public enum AgentStatus: String, Sendable, Codable {
    case idle
    case running
    case waiting
    case error

    public var color: Color {
        switch self {
        case .running: return .green
        case .waiting: return .blue
        case .idle: return .gray
        case .error: return .red
        }
    }
}

public enum MergeMode: String, Sendable, Codable {
    case pr
    case land
}

public struct Agent: Sendable, Identifiable, Hashable {
    public let id: String
    public let flow: String
    public let goal: [String]
    public let area: [String]
    public let repo: String

    public var stimulus: Stimulus
    public var status: AgentStatus
    public var iteration: Int

    public var mainBranch: String
    public var prLimit: Int
    public var mergeMode: MergeMode

    public var pid: Int?
    public var createdAt: Date

    // Watch state
    public var lastMainSha: String?

    public init(
        id: String,
        flow: String,
        goal: [String] = [],
        area: [String] = ["."],
        repo: String,
        stimulus: Stimulus = Stimulus(kind: .loop),
        status: AgentStatus = .idle,
        iteration: Int = 0,
        mainBranch: String,
        prLimit: Int = 5,
        mergeMode: MergeMode = .pr,
        pid: Int? = nil,
        createdAt: Date = Date(),
        lastMainSha: String? = nil
    ) {
        self.id = id
        self.flow = flow
        self.goal = goal
        self.area = area
        self.repo = repo
        self.stimulus = stimulus
        self.status = status
        self.iteration = iteration
        self.mainBranch = mainBranch
        self.prLimit = prLimit
        self.mergeMode = mergeMode
        self.pid = pid
        self.createdAt = createdAt
        self.lastMainSha = lastMainSha
    }

    public var shortId: String { String(id.prefix(7)) }

    public var areaDisplay: String {
        area.first == "." ? "root" : area.joined(separator: ", ")
    }

    public var goalDisplay: String {
        goal.isEmpty ? "default" : goal.joined(separator: ", ")
    }

    public var flowDisplay: String {
        flow.isEmpty ? "default" : flow
    }

    public var statusText: String {
        switch status {
        case .running: return "Running"
        case .waiting: return "Waiting"
        case .idle: return "Idle"
        case .error: return "Error"
        }
    }

    public var iterationText: String {
        iteration > 0 ? "iter \(iteration)" : ""
    }

    public var detailText: String {
        let parts = [flowDisplay, goalDisplay].filter { !$0.isEmpty }
        return parts.joined(separator: " · ")
    }

    public var stimulusText: String {
        stimulus.description
    }
}
