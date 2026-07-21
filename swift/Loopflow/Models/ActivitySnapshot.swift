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

public struct OutputActivity: Codable, Sendable, Hashable {
    public let measuredOutputTokens: UInt64
    public let outputTokensFast: UInt64
    public let outputTokensSlow: UInt64
    public let outputTokensPerSecondFast: Double
    public let outputTokensPerSecondSlow: Double
    public let measuredTurns: UInt64
    public let unmeasuredTurns: UInt64

    enum CodingKeys: String, CodingKey {
        case measuredOutputTokens = "measured_output_tokens"
        case outputTokensFast = "output_tokens_fast"
        case outputTokensSlow = "output_tokens_slow"
        case outputTokensPerSecondFast = "output_tokens_per_second_fast"
        case outputTokensPerSecondSlow = "output_tokens_per_second_slow"
        case measuredTurns = "measured_turns"
        case unmeasuredTurns = "unmeasured_turns"
    }
}

public struct ActivityNode: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let parentId: String?
    public let kind: ActivityNodeKind
    public let label: String
    public let pid: UInt32?
    public let startedAt: Int64
    public let lastProgressAt: Int64?
    public let state: ActivityState
    public let direct: OutputActivity
    public let cumulative: OutputActivity

    enum CodingKeys: String, CodingKey {
        case id, kind, label, pid, state, direct, cumulative
        case parentId = "parent_id"
        case startedAt = "started_at"
        case lastProgressAt = "last_progress_at"
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
    public let fastWindowSeconds: Int64
    public let slowWindowSeconds: Int64
    public let aggregate: OutputActivity
    public let nodes: [ActivityNode]
    public let providerProcesses: [ProviderProcess]

    enum CodingKeys: String, CodingKey {
        case aggregate, nodes
        case schemaVersion = "schema_version"
        case observedAt = "observed_at"
        case fastWindowSeconds = "fast_window_seconds"
        case slowWindowSeconds = "slow_window_seconds"
        case providerProcesses = "provider_processes"
    }
}
