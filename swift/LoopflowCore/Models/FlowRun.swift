// FlowRun - an execution instance of a Flow, spawned by a Wave.

import Foundation
import SwiftUI

public enum FlowRunStatus: String, Sendable, Codable {
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

public struct FlowRun: Sendable, Identifiable {
    public let id: String
    public let waveId: String?

    public let flow: String
    public let area: String
    public let repo: String
    public let direction: [String]

    public var status: FlowRunStatus
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
        waveId: String?,
        flow: String,
        area: String,
        repo: String,
        direction: [String] = [],
        status: FlowRunStatus = .pending,
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
        self.waveId = waveId
        self.flow = flow
        self.area = area
        self.repo = repo
        self.direction = direction
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
        case .pending: return "Pending"
        case .completed: return "Completed"
        case .failed: return "Failed"
        case .cancelled: return "Cancelled"
        }
    }
}
