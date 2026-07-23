import Foundation

public enum UsageScopeKind: String, Codable, Sendable, Hashable {
    case global
    case repository
    case wave
    case project
    case task
    case exec
    case invocation
}

public struct UsageScope: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let parentId: String?
    public let kind: UsageScopeKind
    public let label: String
    public let repo: String?
    public let wave: String?
    public let project: String?
    public let task: String?
    public let execId: String?
    public let invocationId: String?

    enum CodingKeys: String, CodingKey {
        case id, kind, label, repo, wave, project, task
        case parentId = "parent_id"
        case execId = "exec_id"
        case invocationId = "invocation_id"
    }
}

public struct UsageInterval: Codable, Sendable, Hashable, Identifiable {
    public var id: Int64 { windowSeconds }

    public let windowSeconds: Int64
    public let inputTokens: UInt64?
    public let totalInputTokens: UInt64?
    public let outputTokens: UInt64
    public let reasoningTokens: UInt64?
    public let cacheReadTokens: UInt64?
    public let cacheWriteTokens: UInt64?
    public let peakInputTokens: UInt64?
    public let contextWindowTokens: UInt64?
    public let costUsd: Double?
    public let outputTokensPerSecond: Double
    public let measuredTurns: UInt64
    public let unmeasuredTurns: UInt64
    public let outputComplete: Bool

    enum CodingKeys: String, CodingKey {
        case outputComplete = "output_complete"
        case windowSeconds = "window_seconds"
        case inputTokens = "input_tokens"
        case totalInputTokens = "total_input_tokens"
        case outputTokens = "output_tokens"
        case reasoningTokens = "reasoning_tokens"
        case cacheReadTokens = "cache_read_tokens"
        case cacheWriteTokens = "cache_write_tokens"
        case peakInputTokens = "peak_input_tokens"
        case contextWindowTokens = "context_window_tokens"
        case costUsd = "cost_usd"
        case outputTokensPerSecond = "output_tokens_per_second"
        case measuredTurns = "measured_turns"
        case unmeasuredTurns = "unmeasured_turns"
    }
}

public struct UsageReading: Codable, Sendable, Hashable, Identifiable {
    public var id: String { scope.id }

    public let scope: UsageScope
    public let intervals: [UsageInterval]

    public func interval(seconds: Int64) -> UsageInterval? {
        intervals.first { $0.windowSeconds == seconds }
    }
}

public struct UsageBucket: Codable, Sendable, Hashable, Identifiable {
    public var id: Int64 { startedAt }

    public let startedAt: Int64
    public let endedAt: Int64
    public let inputTokens: UInt64?
    public let totalInputTokens: UInt64?
    public let outputTokens: UInt64
    public let reasoningTokens: UInt64?
    public let cacheReadTokens: UInt64?
    public let cacheWriteTokens: UInt64?
    public let costUsd: Double?

    enum CodingKeys: String, CodingKey {
        case startedAt = "started_at"
        case endedAt = "ended_at"
        case inputTokens = "input_tokens"
        case totalInputTokens = "total_input_tokens"
        case outputTokens = "output_tokens"
        case reasoningTokens = "reasoning_tokens"
        case cacheReadTokens = "cache_read_tokens"
        case cacheWriteTokens = "cache_write_tokens"
        case costUsd = "cost_usd"
    }
}

public struct UsageSnapshot: Codable, Sendable, Hashable {
    public let schemaVersion: UInt32
    public let observedAt: Int64
    public let windows: [Int64]
    public let readings: [UsageReading]
    public let historyBucketSeconds: Int64
    public let globalHistory: [UsageBucket]

    public var global: UsageReading? {
        readings.first { $0.scope.kind == .global }
    }

    public func reading(scopeId: String) -> UsageReading? {
        readings.first { $0.scope.id == scopeId }
    }

    enum CodingKeys: String, CodingKey {
        case windows, readings
        case schemaVersion = "schema_version"
        case observedAt = "observed_at"
        case historyBucketSeconds = "history_bucket_seconds"
        case globalHistory = "global_history"
    }
}
