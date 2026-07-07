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
    public let step: String
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
        case step
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
        step: String,
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
        self.step = step
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

public struct SessionConnectionInfo: Sendable, Equatable {
    public let kind: String
    public let sessionName: String
    public let host: String
    public let cwd: String
    public let status: SessionStatus

    public init(
        kind: String,
        sessionName: String,
        host: String,
        cwd: String,
        status: SessionStatus
    ) {
        self.kind = kind
        self.sessionName = sessionName
        self.host = host
        self.cwd = cwd
        self.status = status
    }

    public var usesLocalTmux: Bool {
        let normalized = host
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .trimmingCharacters(in: CharacterSet(charactersIn: "[]"))
            .lowercased()
        return switch normalized {
        case "localhost", "127.0.0.1", "::1":
            true
        default:
            false
        }
    }
}

public struct SessionLaunchResponse: Sendable, Equatable {
    public let session: Session
    public let connection: SessionConnectionInfo

    public init(session: Session, connection: SessionConnectionInfo) {
        self.session = session
        self.connection = connection
    }
}

public struct WaveAgentTreeSession: Sendable, Equatable {
    public let session: Session
    public let connection: SessionConnectionInfo?

    public init(session: Session, connection: SessionConnectionInfo?) {
        self.session = session
        self.connection = connection
    }
}

public struct WaveAgentTree: Sendable, Identifiable, Equatable {
    public let object: String
    public let id: String
    public let wave: Wave
    public let childWaves: [Wave]
    public let sessions: [WaveAgentTreeSession]

    public init(
        object: String,
        id: String,
        wave: Wave,
        childWaves: [Wave],
        sessions: [WaveAgentTreeSession]
    ) {
        self.object = object
        self.id = id
        self.wave = wave
        self.childWaves = childWaves
        self.sessions = sessions
    }
}
