// WaveRun - a single execution of a Wave.

import Foundation
import SwiftUI

public enum WaveRunStatus: String, Sendable, Codable {
    case pending
    case running
    case waiting
    case completed
    case failed
    case cancelled

    public var color: Color {
        switch self {
        case .running: return .green
        case .waiting: return .yellow
        case .pending: return .blue
        case .completed: return .gray
        case .failed: return .red
        case .cancelled: return .orange
        }
    }
}

public struct WaveRun: Sendable, Identifiable, Hashable {
    public let id: String
    public let waveId: String?

    public let flow: String
    public let area: String
    public let repo: String
    public let direction: [String]

    public var status: WaveRunStatus
    public var iteration: Int
    public var stepIndex: Int

    public var worktree: String?
    public var branch: String?
    public var currentStep: String?
    public var error: String?
    public var pr: PullRequest?

    public var startedAt: Date?
    public var endedAt: Date?
    public var createdAt: Date

    public init(
        id: String,
        waveId: String?,
        flow: String,
        area: String,
        repo: String,
        direction: [String] = [],
        status: WaveRunStatus = .pending,
        iteration: Int = 0,
        stepIndex: Int = 0,
        worktree: String? = nil,
        branch: String? = nil,
        currentStep: String? = nil,
        error: String? = nil,
        pr: PullRequest? = nil,
        startedAt: Date? = nil,
        endedAt: Date? = nil,
        createdAt: Date = Date()
    ) {
        self.id = id
        self.waveId = waveId
        self.flow = flow
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

    public var shortId: String { String(id.prefix(7)) }

    public var areaDisplay: String {
        area == "." ? "root" : area
    }

    public var directionDisplay: String {
        direction.isEmpty ? "default" : direction.joined(separator: ", ")
    }

    public var flowDisplay: String {
        flow.isEmpty ? "default" : flow
    }

    public var statusText: String {
        switch status {
        case .running: return currentStep ?? "Running"
        case .waiting: return "Waiting"
        case .pending: return "Pending"
        case .completed: return "Completed"
        case .failed: return "Failed"
        case .cancelled: return "Cancelled"
        }
    }
}
