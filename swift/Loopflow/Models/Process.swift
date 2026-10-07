import Foundation

/// One actual lf process. Missing outcomes do not establish current liveness.
public struct Process: Codable, Sendable, Equatable, Identifiable {
    public let id: String
    public let traceID: String
    public let parentProcessID: String?
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
        case id, command, repo, cwd, outcome, signal, error
        case traceID = "trace_id"
        case parentProcessID = "parent_process_id"
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
    public let id: String

    enum CodingKeys: String, CodingKey {
        case id
        case startedAt = "started_at"
    }
}

public struct ProcessPage: Codable, Sendable, Equatable {
    public let entries: [Process]
    public let next: ProcessCursor?
}
