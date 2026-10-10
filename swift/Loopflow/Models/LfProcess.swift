import Foundation

public enum ProcessKind: String, Codable, Sendable, Equatable {
    case lf
    case agent
}

/// One recorded process. Missing outcomes do not establish current liveness.
public struct LfProcess: Codable, Sendable, Equatable, Identifiable {
    public let id: String
    public let pid: UInt32?
    public let kind: ProcessKind
    public let agentSessionID: String?
    public let osStartedAt: Int64?
    public var id: String { id }
    public let traceID: String
    public let parentLfProcessId: String?
    public let viaAgent: Bool?
    public let callerSessionID: String?
    public let callerProviderGeneration: Int64?
    public let command: String?
    public let repo: String?
    public let cwd: String?
    public let startedAt: Int64
    public let completedAt: Int64?
    public let outcome: String?
    public let exitCode: Int32?
    public let signal: String?
    public let error: String?

    enum CodingKeys: String, CodingKey {
        case id, pid, kind, command, repo, cwd, outcome, signal, error
        case agentSessionID = "agent_session_id"
        case osStartedAt = "os_started_at"
        case traceID = "trace_id"
        case parentLfProcessId = "parent_lf_process_id"
        case viaAgent = "via_agent"
        case callerSessionID = "caller_session_id"
        case callerProviderGeneration = "caller_provider_generation"
        case startedAt = "started_at"
        case completedAt = "completed_at"
        case exitCode = "exit_code"
    }
}

/// Continue with the same filters; refresh from page one to observe new writes.
public struct LfProcessCursor: Codable, Sendable, Equatable {
    public let startedAt: Int64
    public let id: String

    enum CodingKeys: String, CodingKey {
        case id
        case startedAt = "started_at"
    }
}

public struct LfProcessPage: Codable, Sendable, Equatable {
    public let entries: [LfProcess]
    public let next: LfProcessCursor?
}
