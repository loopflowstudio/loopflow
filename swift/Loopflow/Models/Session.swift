import Foundation

public enum SessionStatus: String, Sendable, Codable, CaseIterable {
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

public enum SessionUse: String, Sendable, Codable, CaseIterable {
    case waveAgent = "wave_agent"
    case worker
    case palette
}

public struct Session: Sendable, Identifiable, Codable, Equatable {
    public let id: String
    public let waveId: String
    public let runId: String?
    public let parentSessionId: String?
    public let sessionUse: SessionUse
    public let skill: String
    public let agent: String
    public let cwd: String
    public let argv: [String]
    public let env: [String: String]
    public let source: String
    public let tmuxName: String
    public let status: SessionStatus
    public let createdAt: Date
    public let attachedAt: Date?
    public let startedAt: Date?
    public let completedAt: Date?

    enum CodingKeys: String, CodingKey {
        case id
        case waveId = "wave_id"
        case runId = "run_id"
        case parentSessionId = "parent_session_id"
        case sessionUse = "use"
        case skill
        case agent
        case cwd
        case argv
        case env
        case source
        case tmuxName = "tmux_name"
        case status
        case createdAt = "created_at"
        case attachedAt = "attached_at"
        case startedAt = "started_at"
        case completedAt = "completed_at"
    }

    public init(
        id: String,
        waveId: String,
        runId: String?,
        parentSessionId: String?,
        sessionUse: SessionUse,
        skill: String,
        agent: String,
        cwd: String,
        argv: [String],
        env: [String: String],
        source: String,
        tmuxName: String,
        status: SessionStatus,
        createdAt: Date,
        attachedAt: Date?,
        startedAt: Date?,
        completedAt: Date?
    ) {
        self.id = id
        self.waveId = waveId
        self.runId = runId
        self.parentSessionId = parentSessionId
        self.sessionUse = sessionUse
        self.skill = skill
        self.agent = agent
        self.cwd = cwd
        self.argv = argv
        self.env = env
        self.source = source
        self.tmuxName = tmuxName
        self.status = status
        self.createdAt = createdAt
        self.attachedAt = attachedAt
        self.startedAt = startedAt
        self.completedAt = completedAt
    }
}
