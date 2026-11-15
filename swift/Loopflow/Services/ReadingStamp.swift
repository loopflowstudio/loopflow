import Foundation

/// SQLite revisions belong to one Machine and database lifetime. External
/// observations, including process liveness, are not certified by these counters.
public struct ReadingStamp: Codable, Equatable, Sendable {
    public struct Revisions: Codable, Equatable, Sendable {
        public let planning: Int64
        public let sessions: Int64
        public let graphs: Int64
        public let processes: Int64
        public let usage: Int64
    }

    public struct Observations: Codable, Equatable, Sendable {
        public struct Dependency: Codable, Equatable, Sendable {
            public let fact: String
            public let token: UInt64
        }
        public let source: String
        public let validatedAt: UInt64
        public let dependencies: [Dependency]
        private enum CodingKeys: String, CodingKey {
            case source, dependencies, validatedAt = "validated_at"
        }
    }

    public let observations: Observations?
    public let machine: String
    public let epoch: String
    public let schema: UInt32
    public let query: String?
    public let complete: Bool
    public let revisions: Revisions

    private enum CodingKeys: String, CodingKey { case machine, epoch, schema, query, complete, revisions, observations }

    public init(from decoder: any Decoder) throws {
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        observations = try fields.decode(Observations?.self, forKey: .observations)
        machine = try fields.decode(String.self, forKey: .machine)
        epoch = try fields.decode(String.self, forKey: .epoch)
        schema = try fields.decode(UInt32.self, forKey: .schema)
        query = try fields.decode(String?.self, forKey: .query)
        complete = try fields.decode(Bool.self, forKey: .complete)
        revisions = try fields.decode(Revisions.self, forKey: .revisions)
    }

    public func encode(to encoder: any Encoder) throws {
        var fields = encoder.container(keyedBy: CodingKeys.self)
        try fields.encode(observations, forKey: .observations)
        try fields.encode(machine, forKey: .machine)
        try fields.encode(epoch, forKey: .epoch)
        try fields.encode(schema, forKey: .schema)
        try fields.encode(query, forKey: .query)
        try fields.encode(complete, forKey: .complete)
        try fields.encode(revisions, forKey: .revisions)
    }
}

/// Both native clients decode the complete subscription envelope. A null stamp
/// cannot acknowledge a write, even when the frame answers a checkpoint.
public struct SubscriptionReading<Body: Decodable>: Decodable {
    public let answers: UInt64?
    public let stamp: ReadingStamp?
    public let unavailable: String?
    public let body: Body?

    private enum CodingKeys: String, CodingKey { case answers, stamp, unavailable, body }

    public init(from decoder: any Decoder) throws {
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        answers = try fields.decode(UInt64?.self, forKey: .answers)
        stamp = try fields.decode(ReadingStamp?.self, forKey: .stamp)
        unavailable = try fields.decode(String?.self, forKey: .unavailable)
        body = try fields.decode(Body?.self, forKey: .body)
    }
}

extension SubscriptionReading: Sendable where Body: Sendable {}
