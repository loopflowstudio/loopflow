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
    public let traceID: String
    public let parentLfProcessId: String?
    public let viaAgent: Bool?
    public let callerSessionID: String?
    public let callerAgentProcessId: String?
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
        case callerAgentProcessId = "caller_agent_process_id"
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

extension LfProcess {
    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = StructuredIdentity.uuid(try values.decode(String.self, forKey: .id))
        pid = try values.decodeIfPresent(UInt32.self, forKey: .pid)
        kind = try values.decode(ProcessKind.self, forKey: .kind)
        agentSessionID = (try values.decodeIfPresent(String.self, forKey: .agentSessionID)).map { StructuredIdentity.session($0) }
        osStartedAt = try values.decodeIfPresent(Int64.self, forKey: .osStartedAt)
        traceID = StructuredIdentity.uuid(try values.decode(String.self, forKey: .traceID))
        parentLfProcessId = (try values.decodeIfPresent(String.self, forKey: .parentLfProcessId)).map { StructuredIdentity.uuid($0) }
        viaAgent = try values.decodeIfPresent(Bool.self, forKey: .viaAgent)
        callerSessionID = (try values.decodeIfPresent(String.self, forKey: .callerSessionID)).map { StructuredIdentity.session($0) }
        callerAgentProcessId = (try values.decodeIfPresent(String.self, forKey: .callerAgentProcessId)).map { StructuredIdentity.uuid($0) }
        command = try values.decodeIfPresent(String.self, forKey: .command)
        repo = try values.decodeIfPresent(String.self, forKey: .repo)
        cwd = try values.decodeIfPresent(String.self, forKey: .cwd)
        startedAt = try values.decode(Int64.self, forKey: .startedAt)
        completedAt = try values.decodeIfPresent(Int64.self, forKey: .completedAt)
        outcome = try values.decodeIfPresent(String.self, forKey: .outcome)
        exitCode = try values.decodeIfPresent(Int32.self, forKey: .exitCode)
        signal = try values.decodeIfPresent(String.self, forKey: .signal)
        error = try values.decodeIfPresent(String.self, forKey: .error)
    }
}
