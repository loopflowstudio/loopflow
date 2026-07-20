import Foundation

public struct TurnRecord: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let invocationId: String
    public let basis: WorkBasis
    public let state: InvocationBoundaryStateRecord
    public let providerTurnId: String?
    public let rootOutput: String?
    public let startedAt: String
    public let endedAt: String?

    enum CodingKeys: String, CodingKey {
        case id, basis, state
        case invocationId = "invocation_id"
        case providerTurnId = "provider_turn_id"
        case rootOutput = "root_output"
        case startedAt = "started_at"
        case endedAt = "ended_at"
    }
}

public enum AnswerRouteRecord: Codable, Sendable, Hashable {
    case user
    case parent(WorkReference)

    private enum CodingKeys: String, CodingKey { case kind, work }
    private enum Kind: String, Codable { case user, parent }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(Kind.self, forKey: .kind) {
        case .user:
            self = .user
        case .parent:
            self = .parent(try values.decode(WorkReference.self, forKey: .work))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .user:
            try values.encode(Kind.user, forKey: .kind)
        case .parent(let work):
            try values.encode(Kind.parent, forKey: .kind)
            try values.encode(work, forKey: .work)
        }
    }
}

public enum AnswerAuthorRecord: Codable, Sendable, Hashable {
    case user
    case run(String)

    private struct DynamicKey: CodingKey {
        let stringValue: String
        let intValue: Int? = nil

        init?(stringValue: String) { self.stringValue = stringValue }
        init?(intValue: Int) { return nil }
    }

    public init(from decoder: Decoder) throws {
        if let value = try? decoder.singleValueContainer().decode(String.self), value == "User" {
            self = .user
            return
        }
        let values = try decoder.container(keyedBy: DynamicKey.self)
        guard let run = DynamicKey(stringValue: "Run"), values.contains(run) else {
            throw DecodingError.dataCorrupted(
                DecodingError.Context(
                    codingPath: decoder.codingPath,
                    debugDescription: "Answer author must be User or Run"
                )
            )
        }
        self = .run(try values.decode(String.self, forKey: run))
    }

    public func encode(to encoder: Encoder) throws {
        switch self {
        case .user:
            var value = encoder.singleValueContainer()
            try value.encode("User")
        case .run(let id):
            var values = encoder.container(keyedBy: DynamicKey.self)
            guard let run = DynamicKey(stringValue: "Run") else { return }
            try values.encode(id, forKey: run)
        }
    }
}

public struct AnswerRecord: Codable, Sendable, Hashable {
    public let askId: String
    public let author: AnswerAuthorRecord
    public let text: String
    public let answeredAt: String

    enum CodingKeys: String, CodingKey {
        case author, text
        case askId = "ask_id"
        case answeredAt = "answered_at"
    }
}

public struct AskExchangeRecord: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let turnId: String
    public let route: AnswerRouteRecord
    public let question: String
    public let askedAt: String
    public let answer: AnswerRecord?

    enum CodingKeys: String, CodingKey {
        case id, route, question, answer
        case turnId = "turn_id"
        case askedAt = "asked_at"
    }
}
