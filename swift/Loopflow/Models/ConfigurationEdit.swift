public struct ConfigurationValue: Codable, Sendable {
    public let value: JSONValue
    public let candidates: [ConfigurationCandidate]
}

public struct ConfigurationCandidate: Codable, Sendable {
    public let key: String
    public let source: String
    public let reason: String
    public let locator: String
    public let scope: String
    public let target: JSONValue
    public let value: JSONValue
}

public struct ConfigurationEntry: Codable, Sendable, Equatable {
    public let path: [String]
    public let value: JSONValue
}

public enum ConfigurationChange: Encodable, Sendable {
    case set(String)
    case reset
    case restore([ConfigurationEntry])

    public func encode(to encoder: Encoder) throws {
        let fields: TaggedFields = switch self {
        case .set(let value): TaggedFields("set", ["value": value])
        case .reset: TaggedFields("reset")
        case .restore(let entries): TaggedFields("restore", ["entries": entries])
        }
        try fields.encode(to: encoder, tag: "kind")
    }
}

public struct ConfigurationEditReceipt: Codable, Sendable {
    public let before: [ConfigurationEntry]
    public let after: [ConfigurationEntry]
}

public struct MoveReview: Codable, Sendable {
    public struct Patch: Codable, Sendable {
        public let path: String
        public let patch: String
    }
    public let changes: [Patch]
    public let effective: ConfigurationValue
    public let receipt: MoveReceipt
}

public struct MoveReceipt: Codable, Sendable {
    public let source: ConfigurationEditReceipt
    public let receiving: ConfigurationEditReceipt
}

public enum MoveResult: Decodable, Sendable {
    case preview(MoveReview)
    case applied(MoveReceipt)
    case pending
    case recovered, dismissed

    private enum Keys: String, CodingKey { case kind, review, receipt }
    private enum Kind: String, Decodable { case preview, applied, pending, recovered, dismissed }
    public init(from decoder: Decoder) throws {
        let fields = try decoder.container(keyedBy: Keys.self)
        switch try fields.decode(Kind.self, forKey: .kind) {
        case .preview: self = .preview(try fields.decode(MoveReview.self, forKey: .review))
        case .applied: self = .applied(try fields.decode(MoveReceipt.self, forKey: .receipt))
        case .pending: self = .pending
        case .recovered: self = .recovered
        case .dismissed: self = .dismissed
        }
    }
}

public enum MoveAction: Encodable, Sendable {
    case preview, resume, dismiss, confirm(MoveReview), undo(MoveReceipt)

    public func encode(to encoder: Encoder) throws {
        let fields: TaggedFields = switch self {
        case .preview: TaggedFields("preview")
        case .resume: TaggedFields("resume")
        case .dismiss: TaggedFields("dismiss")
        case .confirm(let review): TaggedFields("confirm", ["review": review])
        case .undo(let receipt): TaggedFields("undo", ["receipt": receipt])
        }
        try fields.encode(to: encoder, tag: "kind")
    }
}
