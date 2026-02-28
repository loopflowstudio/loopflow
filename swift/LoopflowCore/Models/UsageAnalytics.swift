import Foundation

public enum UsageGroupBy: String, Sendable, CaseIterable {
    case wave
    case flow
    case step
    case model
    case source
}

public enum UsageTimeBucket: String, Sendable, CaseIterable {
    case day
    case week
    case month
}

public struct UsageAnalyticsFilters: Sendable {
    public var wave: String?
    public var flow: String?
    public var step: String?
    public var model: String?
    public var from: Date?
    public var to: Date?

    public init(
        wave: String? = nil,
        flow: String? = nil,
        step: String? = nil,
        model: String? = nil,
        from: Date? = nil,
        to: Date? = nil
    ) {
        self.wave = wave
        self.flow = flow
        self.step = step
        self.model = model
        self.from = from
        self.to = to
    }
}

public struct TokenTotals: Sendable, Decodable {
    public let input: Int
    public let output: Int
    public let reasoning: Int
    public let cacheRead: Int
    public let cacheWrite: Int

    enum CodingKeys: String, CodingKey {
        case input
        case output
        case reasoning
        case cacheRead = "cache_read"
        case cacheWrite = "cache_write"
    }

    public var total: Int {
        input + output + reasoning + cacheRead + cacheWrite
    }
}

public struct UsageSummaryGroup: Sendable, Decodable, Identifiable {
    public let key: String
    public let tokens: TokenTotals
    public let sessions: Int
    public let turns: Int

    public var id: String { key }
}

public struct UsageSummary: Sendable, Decodable {
    public let object: String
    public let groupBy: String
    public let from: String?
    public let to: String?
    public let groups: [UsageSummaryGroup]

    enum CodingKeys: String, CodingKey {
        case object
        case groupBy = "group_by"
        case from
        case to
        case groups
    }
}

public struct UsageTimeseriesBucket: Sendable, Decodable, Identifiable {
    public let period: String
    public let groups: [UsageSummaryGroup]

    public var id: String { period }
}

public struct UsageTimeseries: Sendable, Decodable {
    public let object: String
    public let bucket: String
    public let groupBy: String
    public let from: String?
    public let to: String?
    public let buckets: [UsageTimeseriesBucket]

    enum CodingKeys: String, CodingKey {
        case object
        case bucket
        case groupBy = "group_by"
        case from
        case to
        case buckets
    }
}
