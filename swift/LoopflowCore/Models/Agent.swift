// Agent - an autonomous AI coding agent.
// Activation modes: loop (continuous), watch (on file change), cron (scheduled).

import Foundation
import SwiftUI

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
    public let voice: [String]
    public let area: [String]
    public let repo: String

    public var status: AgentStatus
    public var iteration: Int

    public var mainBranch: String
    public var prLimit: Int
    public var mergeMode: MergeMode

    public var pid: Int?
    public var createdAt: Date

    // Activation config (determines mode)
    public var watchPaths: String?
    public var cron: String?
    public var lastMainSha: String?

    public init(
        id: String,
        flow: String,
        voice: [String] = [],
        area: [String] = ["."],
        repo: String,
        status: AgentStatus = .idle,
        iteration: Int = 0,
        mainBranch: String,
        prLimit: Int = 5,
        mergeMode: MergeMode = .pr,
        pid: Int? = nil,
        createdAt: Date = Date(),
        watchPaths: String? = nil,
        cron: String? = nil,
        lastMainSha: String? = nil
    ) {
        self.id = id
        self.flow = flow
        self.voice = voice
        self.area = area
        self.repo = repo
        self.status = status
        self.iteration = iteration
        self.mainBranch = mainBranch
        self.prLimit = prLimit
        self.mergeMode = mergeMode
        self.pid = pid
        self.createdAt = createdAt
        self.watchPaths = watchPaths
        self.cron = cron
        self.lastMainSha = lastMainSha
    }

    public var shortId: String { String(id.prefix(7)) }

    /// Activation mode: 'watch', 'cron', or 'loop'
    public var mode: String {
        if watchPaths != nil { return "watch" }
        if cron != nil { return "cron" }
        return "loop"
    }

    public var areaDisplay: String {
        area.first == "." ? "root" : area.joined(separator: ", ")
    }

    public var voiceDisplay: String {
        voice.isEmpty ? "default" : voice.joined(separator: ", ")
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
        let parts = [flowDisplay, voiceDisplay].filter { !$0.isEmpty }
        return parts.joined(separator: " · ")
    }
}
