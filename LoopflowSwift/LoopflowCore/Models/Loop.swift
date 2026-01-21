// Loop model for lfd background loops.
// Reads from ~/.lf/lfd.db (loops and loop_runs tables)

import Foundation
import SwiftUI

public enum LoopType: String, Sendable, Codable {
    case loop
    case flow
    case subscribe
    case schedule
}

public enum LoopStatus: String, Sendable, Codable {
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

public enum LoopMergeMode: String, Sendable, Codable {
    case pr
    case land
}

public struct Loop: Sendable, Identifiable, Hashable {
    public let id: String
    public let type: LoopType
    public let area: String
    public let goals: [String]
    public let flow: String?
    public let repo: String
    public let loopMain: String
    public var status: LoopStatus
    public var iteration: Int
    public var prLimit: Int
    public var mergeMode: LoopMergeMode
    public var pid: Int?
    public var createdAt: Date

    // Runtime state from loop_runs
    public var currentRunId: String?
    public var currentStep: String?

    // Computed at load time - commits ahead of main
    public var commitsAhead: Int = 0

    public init(
        id: String,
        type: LoopType,
        area: String,
        goals: [String],
        flow: String? = nil,
        repo: String,
        loopMain: String,
        status: LoopStatus,
        iteration: Int,
        prLimit: Int,
        mergeMode: LoopMergeMode,
        pid: Int? = nil,
        createdAt: Date,
        currentRunId: String? = nil,
        currentStep: String? = nil,
        commitsAhead: Int = 0
    ) {
        self.id = id
        self.type = type
        self.area = area
        self.goals = goals
        self.flow = flow
        self.repo = repo
        self.loopMain = loopMain
        self.status = status
        self.iteration = iteration
        self.prLimit = prLimit
        self.mergeMode = mergeMode
        self.pid = pid
        self.createdAt = createdAt
        self.currentRunId = currentRunId
        self.currentStep = currentStep
        self.commitsAhead = commitsAhead
    }

    public var shortId: String { String(id.prefix(7)) }

    public var areaDisplay: String {
        area == "." ? "root" : area
    }

    public var goalsDisplay: String {
        goals.isEmpty ? "adaptive" : goals.joined(separator: ", ")
    }

    public var flowDisplay: String {
        flow ?? "default"
    }

    public var detailText: String {
        let parts = [flowDisplay, goalsDisplay].filter { !$0.isEmpty }
        return parts.joined(separator: " · ")
    }

    public var statusText: String {
        switch status {
        case .running: return currentStep ?? "Running"
        case .waiting: return "Waiting"
        case .idle: return "Idle"
        case .error: return "Error"
        }
    }

    public var iterationText: String {
        iteration > 0 ? "iter \(iteration)" : ""
    }
}

public struct LoopRun: Sendable, Identifiable {
    public let id: String
    public let loopId: String
    public let iteration: Int
    public let status: LoopStatus
    public let startedAt: Date
    public var endedAt: Date?
    public var worktree: String?
    public var currentStep: String?
    public var error: String?
    public var prUrl: String?
}
