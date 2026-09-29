import Foundation

/// One actual lf process. Missing outcomes do not establish current liveness.
public struct Exec: Codable, Sendable, Equatable, Identifiable {
    public let id: String
    public let traceID: String
    public let parentExecID: String?
    public let viaAgent: Bool?
    public let callerSessionID: String?
    public let callerProviderGeneration: Int64?
    public let callerFlowTurn: String?
    public let command: String?
    public let repo: String?
    public let cwd: String?
    public let startedAt: Int64
    public let completedAt: Int64?
    public let outcome: String?
    public let exitCode: Int32?
    public let signal: String?
    public let wave: String?

    enum CodingKeys: String, CodingKey {
        case id, command, repo, cwd, outcome, signal, wave
        case traceID = "trace_id"
        case parentExecID = "parent_exec_id"
        case viaAgent = "via_agent"
        case callerSessionID = "caller_session_id"
        case callerProviderGeneration = "caller_provider_generation"
        case callerFlowTurn = "caller_flow_turn"
        case startedAt = "started_at"
        case completedAt = "completed_at"
        case exitCode = "exit_code"
    }
}

/// Continue with the same filters; refresh from page one to observe new writes.
public struct ExecCursor: Codable, Sendable, Equatable {
    public let startedAt: Int64
    public let id: String

    enum CodingKeys: String, CodingKey {
        case id
        case startedAt = "started_at"
    }
}

public struct ExecPage: Codable, Sendable, Equatable {
    public let entries: [Exec]
    public let next: ExecCursor?
}
