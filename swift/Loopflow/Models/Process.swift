import Foundation

/// One actual lf process. Missing outcomes do not establish current liveness.
public struct Process: Codable, Sendable, Equatable, Identifiable {
    public let lfid: String
    public let pid: UInt32?
    public var id: String { lfid }
    public let traceID: String
    public let parentProcessLFID: String?
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
        case lfid, pid, command, repo, cwd, outcome, signal, error
        case traceID = "trace_id"
        case parentProcessLFID = "parent_process_lfid"
        case viaAgent = "via_agent"
        case callerSessionID = "caller_session_id"
        case callerProviderGeneration = "caller_provider_generation"
        case startedAt = "started_at"
        case completedAt = "completed_at"
        case exitCode = "exit_code"
    }
}

/// Continue with the same filters; refresh from page one to observe new writes.
public struct ProcessCursor: Codable, Sendable, Equatable {
    public let startedAt: Int64
    public let lfid: String

    enum CodingKeys: String, CodingKey {
        case lfid
        case startedAt = "started_at"
    }
}

public struct ProcessPage: Codable, Sendable, Equatable {
    public let entries: [Process]
    public let next: ProcessCursor?
}
