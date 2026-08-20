import Foundation

public enum ActivityNodeKind: String, Codable, Sendable, Hashable {
    case exec
    case providerLaunch = "provider_launch"
}

public enum ActivityState: String, Codable, Sendable, Hashable {
    case working
    case waiting
    case stalled
}

public struct ActivityNode: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let parentId: String?
    public let kind: ActivityNodeKind
    public let label: String
    public let repo: String?
    public let wave: String?
    public let project: String?
    public let task: String?
    public let pid: UInt32?
    public let startedAt: Int64
    public let lastProgressAt: Int64?
    public let state: ActivityState
    public let usageScopeId: String

    enum CodingKeys: String, CodingKey {
        case id, kind, label, repo, wave, project, task, pid, state
        case parentId = "parent_id"
        case startedAt = "started_at"
        case lastProgressAt = "last_progress_at"
        case usageScopeId = "usage_scope_id"
    }
}

public enum ProviderClaim: String, Codable, Sendable, Hashable {
    case orphaned
    case unclaimed
}

public struct ProviderProcess: Codable, Sendable, Hashable, Identifiable {
    public let pid: UInt32
    public let ppid: UInt32
    public let processGroup: UInt32
    public let startedAt: Int64
    public let kernelState: String
    public let provider: String
    public let command: String
    public let claim: ProviderClaim

    public var id: UInt32 { pid }

    enum CodingKeys: String, CodingKey {
        case pid, ppid, provider, command, claim
        case processGroup = "process_group"
        case startedAt = "started_at"
        case kernelState = "kernel_state"
    }
}

public struct ActivitySnapshot: Codable, Sendable, Hashable {
    public let schemaVersion: UInt32
    public let observedAt: Int64
    public let usage: UsageSnapshot
    public let nodes: [ActivityNode]
    public let providerProcesses: [ProviderProcess]

    enum CodingKeys: String, CodingKey {
        case usage, nodes
        case schemaVersion = "schema_version"
        case observedAt = "observed_at"
        case providerProcesses = "provider_processes"
    }
}
