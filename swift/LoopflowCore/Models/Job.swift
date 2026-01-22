// Job model for lfd background jobs.
// Reads from ~/.lf/lfd.db (jobs and job_runs tables)

import Foundation
import SwiftUI

// Execution mode: how many times the agent runs
public enum ExecutionMode: String, Sendable, Codable {
    case continuous  // Keep iterating until stopped
    case oneShot = "one_shot"  // Run once, then done
}

// Trigger type: what causes the agent to run
public enum TriggerType: String, Sendable, Codable {
    case manual   // Started explicitly
    case pathset  // File changes on main
    case cron     // Scheduled time
}

// Legacy type enum - combines execution mode and trigger
// Kept for backwards compatibility with database
public enum JobType: String, Sendable, Codable {
    case loop      // CONTINUOUS + MANUAL
    case flow      // ONE_SHOT + MANUAL
    case subscribe // CONTINUOUS + PATHSET
    case schedule  // CONTINUOUS + CRON

    public var executionMode: ExecutionMode {
        switch self {
        case .flow: return .oneShot
        default: return .continuous
        }
    }

    public var triggerType: TriggerType {
        switch self {
        case .loop, .flow: return .manual
        case .subscribe: return .pathset
        case .schedule: return .cron
        }
    }
}

public enum JobStatus: String, Sendable, Codable {
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

public enum JobMergeMode: String, Sendable, Codable {
    case pr
    case land
}

public struct Job: Sendable, Identifiable, Hashable {
    public let id: String
    public let type: JobType
    public let area: String
    public let goals: [String]
    public let flow: String?
    public let repo: String
    public let jobMain: String
    public var status: JobStatus
    public var iteration: Int
    public var prLimit: Int
    public var mergeMode: JobMergeMode
    public var pid: Int?
    public var createdAt: Date

    // Runtime state from job_runs
    public var currentRunId: String?
    public var currentStep: String?

    // Computed at load time - commits ahead of main
    public var commitsAhead: Int = 0

    public init(
        id: String,
        type: JobType,
        area: String,
        goals: [String],
        flow: String? = nil,
        repo: String,
        jobMain: String,
        status: JobStatus,
        iteration: Int,
        prLimit: Int,
        mergeMode: JobMergeMode,
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
        self.jobMain = jobMain
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

    // Semantic accessors (derived from type)
    public var executionMode: ExecutionMode { type.executionMode }
    public var triggerType: TriggerType { type.triggerType }
    public var isOneShot: Bool { executionMode == .oneShot }
    public var isTriggered: Bool { triggerType != .manual }

    // Backwards compatibility property
    public var loopMain: String { jobMain }
}

public struct JobRun: Sendable, Identifiable {
    public let id: String
    public let jobId: String
    public let iteration: Int
    public let status: JobStatus
    public let startedAt: Date
    public var endedAt: Date?
    public var worktree: String?
    public var currentStep: String?
    public var error: String?
    public var prUrl: String?

    // Backwards compatibility property
    public var loopId: String { jobId }
}

// Backwards compatibility type aliases
public typealias Loop = Job
public typealias LoopType = JobType
public typealias LoopStatus = JobStatus
public typealias LoopMergeMode = JobMergeMode
public typealias LoopRun = JobRun
