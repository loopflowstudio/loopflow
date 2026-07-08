// Run - a single execution of a Wave.

import Foundation
import SwiftUI

public enum RunStatus: Sendable, Codable, Hashable, RawRepresentable {
    case unspecified
    case pending
    case running
    case waiting
    case completed
    case ok
    case failed
    case error
    case escalated
    case unknown(String)

    public init?(rawValue: String) {
        self = Self(lfToken: rawValue)
    }

    public init(lfToken: String) {
        switch lfToken {
        case "unspecified": self = .unspecified
        case "pending": self = .pending
        case "running": self = .running
        case "waiting": self = .waiting
        case "completed": self = .completed
        case "ok": self = .ok
        case "failed": self = .failed
        case "error": self = .error
        case "escal.": self = .escalated
        default:
            LoggingService.model("Unknown lf run status: \(lfToken)")
            self = .unknown(lfToken)
        }
    }

    public var rawValue: String {
        switch self {
        case .unspecified: return "unspecified"
        case .pending: return "pending"
        case .running: return "running"
        case .waiting: return "waiting"
        case .completed: return "completed"
        case .ok: return "ok"
        case .failed: return "failed"
        case .error: return "error"
        case .escalated: return "escal."
        case .unknown(let value): return value
        }
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        self = Self(lfToken: try container.decode(String.self))
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }

    public var color: Color {
        switch self {
        case .running: return .statusSuccess
        case .waiting: return .statusWarning
        case .pending: return .statusInfo
        case .completed, .ok: return .statusNeutral
        case .failed, .error: return .statusError
        case .unspecified, .escalated, .unknown: return .statusWarning
        }
    }

    public var displayName: String {
        switch self {
        case .unspecified: return "Unknown"
        case .pending: return "Pending"
        case .running: return "Running"
        case .waiting: return "Waiting"
        case .completed, .ok: return "Completed"
        case .failed, .error: return "Failed"
        case .escalated: return "Escalated"
        case .unknown(let value): return "Unknown: \(value)"
        }
    }
}

public struct Run: Sendable, Identifiable, Hashable {
    public let id: String
    public let waveId: String?

    public let flow: String
    public let task: String?
    public let area: String?
    public let repo: String
    public let direction: [String]

    public var status: RunStatus
    public var iteration: Int
    public var stepIndex: Int

    public var worktree: String?
    public var branch: String?
    public var currentStep: String?
    public var error: String?
    public var pr: PullRequest?

    public var startedAt: Date?
    public var endedAt: Date?
    public var createdAt: Date?

    public init(
        id: String,
        waveId: String?,
        flow: String,
        task: String? = nil,
        area: String? = nil,
        repo: String,
        direction: [String] = [],
        status: RunStatus = .pending,
        iteration: Int = 0,
        stepIndex: Int = 0,
        worktree: String? = nil,
        branch: String? = nil,
        currentStep: String? = nil,
        error: String? = nil,
        pr: PullRequest? = nil,
        startedAt: Date? = nil,
        endedAt: Date? = nil,
        createdAt: Date? = nil
    ) {
        self.id = id
        self.waveId = waveId
        self.flow = flow
        self.task = task
        self.area = area
        self.repo = repo
        self.direction = direction
        self.status = status
        self.iteration = iteration
        self.stepIndex = stepIndex
        self.worktree = worktree
        self.branch = branch
        self.currentStep = currentStep
        self.error = error
        self.pr = pr
        self.startedAt = startedAt
        self.endedAt = endedAt
        self.createdAt = createdAt
    }

}

public extension Run {
    var duration: String? {
        guard let startedAt, let endedAt else { return nil }
        let interval = max(0, endedAt.timeIntervalSince(startedAt))
        let minutes = Int(interval) / 60
        let seconds = Int(interval) % 60
        return "\(minutes)m\(String(format: "%02d", seconds))s"
    }

    var relativeTime: String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        let reference = endedAt ?? startedAt ?? createdAt ?? Date.distantPast
        return formatter.localizedString(for: reference, relativeTo: Date())
    }
}
