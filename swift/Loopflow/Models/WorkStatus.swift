import Foundation

public enum WorkStatus: Codable, Sendable, Hashable {
    case ready
    case done
    case abandoned

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let value = try container.decode(String.self)
        switch value {
        case "ready": self = .ready
        case "done": self = .done
        case "abandoned": self = .abandoned
        default:
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "unknown Work status \(value)"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        switch self {
        case .ready:
            var container = encoder.singleValueContainer()
            try container.encode("ready")
        case .done:
            var container = encoder.singleValueContainer()
            try container.encode("done")
        case .abandoned:
            var container = encoder.singleValueContainer()
            try container.encode("abandoned")
        }
    }

    public var label: String {
        switch self {
        case .ready: "ready"
        case .done: "done"
        case .abandoned: "abandoned"
        }
    }
}

public struct WorkReference: Codable, Sendable, Hashable {
    public enum Kind: String, Codable, Sendable, Hashable {
        case wave
        case project
        case task
    }

    public let kind: Kind
    public let id: String

    public init(kind: Kind, id: String) {
        self.kind = kind
        self.id = id
    }

    public static func wave(id: String) -> WorkReference {
        WorkReference(kind: .wave, id: id)
    }

    public static func project(id: String) -> WorkReference {
        WorkReference(kind: .project, id: id)
    }

    public static func task(id: String) -> WorkReference {
        WorkReference(kind: .task, id: id)
    }
}

/// The durable author shared by Answers and Steers.
public enum WorkAuthor: Codable, Sendable, Hashable {
    case user
    case run(id: String)

    private enum CodingKeys: String, CodingKey {
        case kind, id
    }

    private enum Kind: String, Codable {
        case user, run
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(Kind.self, forKey: .kind) {
        case .user:
            self = .user
        case .run:
            self = .run(id: try container.decode(String.self, forKey: .id))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .user:
            try container.encode(Kind.user, forKey: .kind)
        case .run(let id):
            try container.encode(Kind.run, forKey: .kind)
            try container.encode(id, forKey: .id)
        }
    }
}
