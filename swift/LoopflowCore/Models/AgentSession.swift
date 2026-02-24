import Foundation

public struct AgentSessionConfig: Sendable, Hashable {
    public var model: String?
    public var cwd: String?
    public var systemPrompt: String?
    public var maxTurns: Int?
    public var yoloMode: Bool

    public init(
        model: String? = nil,
        cwd: String? = nil,
        systemPrompt: String? = nil,
        maxTurns: Int? = nil,
        yoloMode: Bool = false
    ) {
        self.model = model
        self.cwd = cwd
        self.systemPrompt = systemPrompt
        self.maxTurns = maxTurns
        self.yoloMode = yoloMode
    }
}

public struct AgentSession: Sendable, Hashable {
    public let id: String
    public let provider: String
    public let status: String
    public let waveRunId: String?
    public let providerSessionId: String?
    public let config: AgentSessionConfig
    public let createdAt: Date?
    public let endedAt: Date?

    public init(
        id: String,
        provider: String,
        status: String,
        waveRunId: String?,
        providerSessionId: String?,
        config: AgentSessionConfig,
        createdAt: Date?,
        endedAt: Date?
    ) {
        self.id = id
        self.provider = provider
        self.status = status
        self.waveRunId = waveRunId
        self.providerSessionId = providerSessionId
        self.config = config
        self.createdAt = createdAt
        self.endedAt = endedAt
    }
}

public enum AgentSessionEvent: Sendable, Hashable {
    case turnStarted(turnId: String)
    case turnCompleted(turnId: String, status: String)
    case textDelta(turnId: String, content: String)
    case reasoningDelta(turnId: String, content: String)
    case statusChanged(status: String)
    case error(code: String, message: String)
    case other(type: String, payload: [String: String])
}

public struct AgentSessionEventEnvelope: Sendable, Hashable {
    public let seq: Int?
    public let event: AgentSessionEvent

    public init(seq: Int?, event: AgentSessionEvent) {
        self.seq = seq
        self.event = event
    }
}
