import Foundation

/// One recorded lf process. Missing outcomes do not establish current liveness.
public struct LfProcess: Codable, Sendable, Equatable, Identifiable {
    public let id: String
    public let pid: UInt32?
    public let osStartedAt: Int64?
    public let traceID: String
    public let parentLfProcessId: String?
    public let viaAgent: Bool?
    public let callerSessionID: String?
    public let callerAgentProcessId: AgentProcessId?
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
        case id, pid, command, repo, cwd, outcome, signal, error
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

/// One recorded provider OS process, launched under the lf process that reserved it.
public struct AgentProcess: Codable, Sendable, Equatable, Identifiable {
    public let id: AgentProcessId
    public let pid: UInt32?
    public let osStartedAt: Int64?
    public let agentSessionID: String?
    public let traceID: String
    public let parentLfProcessId: String?
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
        case id, pid, command, repo, cwd, outcome, signal, error
        case osStartedAt = "os_started_at"
        case agentSessionID = "agent_session_id"
        case traceID = "trace_id"
        case parentLfProcessId = "parent_lf_process_id"
        case startedAt = "started_at"
        case completedAt = "completed_at"
        case exitCode = "exit_code"
    }
}

/// One row of the process inventory, tagged by `kind`.
public enum ProcessEntry: Codable, Sendable, Equatable, Identifiable {
    case lf(LfProcess)
    case agent(AgentProcess)

    public var id: String {
        switch self {
        case .lf(let process): process.id
        case .agent(let process): process.id.rawValue
        }
    }

    public var isAgent: Bool {
        if case .agent = self { true } else { false }
    }

    public var command: String? {
        switch self {
        case .lf(let process): process.command
        case .agent(let process): process.command
        }
    }

    public var outcome: String? {
        switch self {
        case .lf(let process): process.outcome
        case .agent(let process): process.outcome
        }
    }

    private enum Kind: String, Codable { case lf, agent }
    private enum KindKey: String, CodingKey { case kind }

    public init(from decoder: Decoder) throws {
        switch try decoder.container(keyedBy: KindKey.self).decode(Kind.self, forKey: .kind) {
        case .lf: self = .lf(try LfProcess(from: decoder))
        case .agent: self = .agent(try AgentProcess(from: decoder))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var tag = encoder.container(keyedBy: KindKey.self)
        switch self {
        case .lf(let process):
            try tag.encode(Kind.lf, forKey: .kind)
            try process.encode(to: encoder)
        case .agent(let process):
            try tag.encode(Kind.agent, forKey: .kind)
            try process.encode(to: encoder)
        }
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
    public let entries: [ProcessEntry]
    public let next: ProcessCursor?
}

extension LfProcess {
    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = StructuredIdentity.uuid(try values.decode(String.self, forKey: .id))
        pid = try values.decodeIfPresent(UInt32.self, forKey: .pid)
        osStartedAt = try values.decodeIfPresent(Int64.self, forKey: .osStartedAt)
        traceID = StructuredIdentity.uuid(try values.decode(String.self, forKey: .traceID))
        parentLfProcessId = (try values.decodeIfPresent(String.self, forKey: .parentLfProcessId)).map { StructuredIdentity.uuid($0) }
        viaAgent = try values.decodeIfPresent(Bool.self, forKey: .viaAgent)
        callerSessionID = (try values.decodeIfPresent(String.self, forKey: .callerSessionID)).map { StructuredIdentity.session($0) }
        callerAgentProcessId = try values.decodeIfPresent(AgentProcessId.self, forKey: .callerAgentProcessId)
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

extension AgentProcess {
    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decode(AgentProcessId.self, forKey: .id)
        pid = try values.decodeIfPresent(UInt32.self, forKey: .pid)
        osStartedAt = try values.decodeIfPresent(Int64.self, forKey: .osStartedAt)
        agentSessionID = (try values.decodeIfPresent(String.self, forKey: .agentSessionID)).map { StructuredIdentity.session($0) }
        traceID = StructuredIdentity.uuid(try values.decode(String.self, forKey: .traceID))
        parentLfProcessId = (try values.decodeIfPresent(String.self, forKey: .parentLfProcessId)).map { StructuredIdentity.uuid($0) }
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
