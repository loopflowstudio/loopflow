// Models for lfd daemon.
// Reads from ~/.lf/lfd.db (runs, loops, subscriptions, schedules tables)

import Foundation
import SwiftUI

// MARK: - Run (execution instance)

public enum RunStatus: String, Sendable, Codable {
    case pending
    case running
    case completed
    case failed
    case cancelled

    public var color: Color {
        switch self {
        case .running: return .green
        case .pending: return .blue
        case .completed: return .gray
        case .failed: return .red
        case .cancelled: return .orange
        }
    }
}

public struct Run: Sendable, Identifiable {
    public let id: String
    public let parent: String?  // "loop:<id>" | "subscription:<id>" | "schedule:<id>" | nil

    public let flow: String
    public let area: String
    public let repo: String
    public let goals: [String]

    public var status: RunStatus
    public var iteration: Int

    public var worktree: String?
    public var branch: String?
    public var currentStep: String?
    public var error: String?
    public var prUrl: String?

    public var startedAt: Date?
    public var endedAt: Date?
    public var createdAt: Date

    public init(
        id: String,
        parent: String?,
        flow: String,
        area: String,
        repo: String,
        goals: [String] = [],
        status: RunStatus = .pending,
        iteration: Int = 0,
        worktree: String? = nil,
        branch: String? = nil,
        currentStep: String? = nil,
        error: String? = nil,
        prUrl: String? = nil,
        startedAt: Date? = nil,
        endedAt: Date? = nil,
        createdAt: Date = Date()
    ) {
        self.id = id
        self.parent = parent
        self.flow = flow
        self.area = area
        self.repo = repo
        self.goals = goals
        self.status = status
        self.iteration = iteration
        self.worktree = worktree
        self.branch = branch
        self.currentStep = currentStep
        self.error = error
        self.prUrl = prUrl
        self.startedAt = startedAt
        self.endedAt = endedAt
        self.createdAt = createdAt
    }

    public var shortId: String { String(id.prefix(7)) }

    public var parentType: String? {
        guard let parent else { return nil }
        return parent.split(separator: ":").first.map(String.init)
    }

    public var parentId: String? {
        guard let parent else { return nil }
        let parts = parent.split(separator: ":", maxSplits: 1)
        return parts.count > 1 ? String(parts[1]) : nil
    }

    public var areaDisplay: String {
        area == "." ? "root" : area
    }

    public var goalsDisplay: String {
        goals.isEmpty ? "adaptive" : goals.joined(separator: ", ")
    }

    public var flowDisplay: String {
        flow.isEmpty ? "default" : flow
    }

    public var statusText: String {
        switch status {
        case .running: return currentStep ?? "Running"
        case .pending: return "Pending"
        case .completed: return "Completed"
        case .failed: return "Failed"
        case .cancelled: return "Cancelled"
        }
    }
}

// MARK: - Trigger status (shared by Loop, Subscription, Schedule)

public enum TriggerStatus: String, Sendable, Codable {
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

// MARK: - Loop (continuous runner)

public struct Loop: Sendable, Identifiable, Hashable {
    public let id: String
    public let flow: String
    public let area: String
    public let repo: String
    public let goals: [String]

    public var status: TriggerStatus
    public var iteration: Int

    public var mainBranch: String
    public var prLimit: Int
    public var mergeMode: MergeMode

    public var pid: Int?
    public var createdAt: Date

    // Runtime state
    public var currentRunId: String?
    public var currentStep: String?
    public var commitsAhead: Int = 0

    public init(
        id: String,
        flow: String,
        area: String,
        repo: String,
        goals: [String] = [],
        status: TriggerStatus = .idle,
        iteration: Int = 0,
        mainBranch: String,
        prLimit: Int = 5,
        mergeMode: MergeMode = .pr,
        pid: Int? = nil,
        createdAt: Date = Date(),
        currentRunId: String? = nil,
        currentStep: String? = nil,
        commitsAhead: Int = 0
    ) {
        self.id = id
        self.flow = flow
        self.area = area
        self.repo = repo
        self.goals = goals
        self.status = status
        self.iteration = iteration
        self.mainBranch = mainBranch
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
        flow.isEmpty ? "default" : flow
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

    public var detailText: String {
        let parts = [flowDisplay, goalsDisplay].filter { !$0.isEmpty }
        return parts.joined(separator: " · ")
    }
}

// MARK: - Subscription (pathset watcher)

public struct Subscription: Sendable, Identifiable, Hashable {
    public let id: String
    public let flow: String
    public let area: String
    public let repo: String
    public let goals: [String]

    public var pathset: String
    public var lastMainSha: String?

    public var status: TriggerStatus
    public var iteration: Int

    public var mainBranch: String
    public var prLimit: Int
    public var mergeMode: MergeMode

    public var pid: Int?
    public var createdAt: Date

    // Runtime state
    public var currentRunId: String?
    public var currentStep: String?
    public var commitsAhead: Int = 0

    public init(
        id: String,
        flow: String,
        area: String,
        repo: String,
        goals: [String] = [],
        pathset: String = "",
        lastMainSha: String? = nil,
        status: TriggerStatus = .idle,
        iteration: Int = 0,
        mainBranch: String,
        prLimit: Int = 5,
        mergeMode: MergeMode = .pr,
        pid: Int? = nil,
        createdAt: Date = Date(),
        currentRunId: String? = nil,
        currentStep: String? = nil,
        commitsAhead: Int = 0
    ) {
        self.id = id
        self.flow = flow
        self.area = area
        self.repo = repo
        self.goals = goals
        self.pathset = pathset
        self.lastMainSha = lastMainSha
        self.status = status
        self.iteration = iteration
        self.mainBranch = mainBranch
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
        flow.isEmpty ? "default" : flow
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

    public var detailText: String {
        let parts = [flowDisplay, goalsDisplay].filter { !$0.isEmpty }
        return parts.joined(separator: " · ")
    }
}

// MARK: - Schedule (cron trigger)

public struct Schedule: Sendable, Identifiable, Hashable {
    public let id: String
    public let flow: String
    public let area: String
    public let repo: String
    public let goals: [String]

    public var cron: String

    public var status: TriggerStatus
    public var iteration: Int

    public var mainBranch: String
    public var prLimit: Int
    public var mergeMode: MergeMode

    public var pid: Int?
    public var createdAt: Date

    // Runtime state
    public var currentRunId: String?
    public var currentStep: String?
    public var commitsAhead: Int = 0

    public init(
        id: String,
        flow: String,
        area: String,
        repo: String,
        goals: [String] = [],
        cron: String = "",
        status: TriggerStatus = .idle,
        iteration: Int = 0,
        mainBranch: String,
        prLimit: Int = 5,
        mergeMode: MergeMode = .pr,
        pid: Int? = nil,
        createdAt: Date = Date(),
        currentRunId: String? = nil,
        currentStep: String? = nil,
        commitsAhead: Int = 0
    ) {
        self.id = id
        self.flow = flow
        self.area = area
        self.repo = repo
        self.goals = goals
        self.cron = cron
        self.status = status
        self.iteration = iteration
        self.mainBranch = mainBranch
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
        flow.isEmpty ? "default" : flow
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

    public var detailText: String {
        let parts = [flowDisplay, goalsDisplay].filter { !$0.isEmpty }
        return parts.joined(separator: " · ")
    }
}

// MARK: - Trigger (any trigger type)

public enum Trigger: Sendable, Identifiable {
    case loop(Loop)
    case subscription(Subscription)
    case schedule(Schedule)

    public var id: String {
        switch self {
        case .loop(let l): return l.id
        case .subscription(let s): return s.id
        case .schedule(let s): return s.id
        }
    }

    public var status: TriggerStatus {
        switch self {
        case .loop(let l): return l.status
        case .subscription(let s): return s.status
        case .schedule(let s): return s.status
        }
    }

    public var area: String {
        switch self {
        case .loop(let l): return l.area
        case .subscription(let s): return s.area
        case .schedule(let s): return s.area
        }
    }

    public var repo: String {
        switch self {
        case .loop(let l): return l.repo
        case .subscription(let s): return s.repo
        case .schedule(let s): return s.repo
        }
    }

    public var mainBranch: String {
        switch self {
        case .loop(let l): return l.mainBranch
        case .subscription(let s): return s.mainBranch
        case .schedule(let s): return s.mainBranch
        }
    }

    public var commitsAhead: Int {
        switch self {
        case .loop(let l): return l.commitsAhead
        case .subscription(let s): return s.commitsAhead
        case .schedule(let s): return s.commitsAhead
        }
    }

    public var areaDisplay: String {
        switch self {
        case .loop(let l): return l.areaDisplay
        case .subscription(let s): return s.areaDisplay
        case .schedule(let s): return s.areaDisplay
        }
    }
}
