import Foundation

public enum TerminalSessionStatus: String, Sendable, Codable, CaseIterable {
    case pending
    case attached
    case running
    case succeeded
    case failed
    case canceled

    public var isTerminal: Bool {
        switch self {
        case .succeeded, .failed, .canceled:
            true
        case .pending, .attached, .running:
            false
        }
    }
}

public struct TerminalSession: Sendable, Identifiable, Codable, Equatable {
    public let id: String
    public let waveId: String
    public let waveRunId: String?
    public let step: String
    public let agent: String
    public let cwd: String
    public let argv: [String]
    public let env: [String: String]
    public let source: String
    public let status: TerminalSessionStatus
    public let createdAt: Date
    public let attachedAt: Date?
    public let startedAt: Date?
    public let completedAt: Date?

    public init(
        id: String,
        waveId: String,
        waveRunId: String? = nil,
        step: String,
        agent: String,
        cwd: String,
        argv: [String] = [],
        env: [String: String] = [:],
        source: String = "wave_step",
        status: TerminalSessionStatus,
        createdAt: Date,
        attachedAt: Date? = nil,
        startedAt: Date? = nil,
        completedAt: Date? = nil
    ) {
        self.id = id
        self.waveId = waveId
        self.waveRunId = waveRunId
        self.step = step
        self.agent = agent
        self.cwd = cwd
        self.argv = argv
        self.env = env
        self.source = source
        self.status = status
        self.createdAt = createdAt
        self.attachedAt = attachedAt
        self.startedAt = startedAt
        self.completedAt = completedAt
    }
}

public struct TerminalLaunchSpec: Sendable, Equatable {
    public let sessionId: String
    public let waveId: String
    public let step: String
    public let agent: String
    public let cwd: String
    public let argv: [String]
    public let env: [String: String]
    public let completionToken: String

    public init(
        sessionId: String,
        waveId: String,
        step: String,
        agent: String,
        cwd: String,
        argv: [String],
        env: [String: String],
        completionToken: String
    ) {
        self.sessionId = sessionId
        self.waveId = waveId
        self.step = step
        self.agent = agent
        self.cwd = cwd
        self.argv = argv
        self.env = env
        self.completionToken = completionToken
    }
}
