// Loop model for lfd background loops.
// Reads from ~/.lf/lfd.db (loops and loop_runs tables)

import Foundation
import SwiftUI

enum LoopType: String, Codable {
    case loop
    case flow
    case subscribe
    case schedule
}

enum LoopStatus: String, Codable {
    case idle
    case running
    case waiting
    case error

    var color: Color {
        switch self {
        case .running: return .green
        case .waiting: return .blue
        case .idle: return .gray
        case .error: return .red
        }
    }
}

enum LoopMergeMode: String, Codable {
    case pr
    case land
}

struct Loop: Identifiable, Hashable {
    let id: String
    let type: LoopType
    let area: String
    let goals: [String]
    let flow: String?
    let repo: String
    let loopMain: String
    var status: LoopStatus
    var iteration: Int
    var prLimit: Int
    var mergeMode: LoopMergeMode
    var pid: Int?
    var createdAt: Date

    // Runtime state from loop_runs
    var currentRunId: String?
    var currentStep: String?

    // Computed at load time - commits ahead of main
    var commitsAhead: Int = 0

    var shortId: String { String(id.prefix(7)) }

    var areaDisplay: String {
        area == "." ? "root" : area
    }

    var goalsDisplay: String {
        goals.isEmpty ? "adaptive" : goals.joined(separator: ", ")
    }

    var flowDisplay: String {
        flow ?? "default"
    }

    var detailText: String {
        let parts = [flowDisplay, goalsDisplay].filter { !$0.isEmpty }
        return parts.joined(separator: " · ")
    }

    var statusText: String {
        switch status {
        case .running: return currentStep ?? "Running"
        case .waiting: return "Waiting"
        case .idle: return "Idle"
        case .error: return "Error"
        }
    }

    var iterationText: String {
        iteration > 0 ? "iter \(iteration)" : ""
    }
}

struct LoopRun: Identifiable {
    let id: String
    let loopId: String
    let iteration: Int
    let status: LoopStatus
    let startedAt: Date
    var endedAt: Date?
    var worktree: String?
    var currentStep: String?
    var error: String?
    var prUrl: String?
}
