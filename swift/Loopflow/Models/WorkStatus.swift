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

/// A Task's lifecycle: unstarted → working → shipping → done, or abandoned.
/// Pause is a hold over working or shipping, shown in their place.
public enum TaskState: String, Codable, Sendable, Hashable {
    case paused
    /// No Workflow taken up, or still at its start.
    case unstarted
    /// Past start, with a person still to take part.
    case working
    /// Handed off: it finishes on its own, through every remaining step, to merge.
    case shipping
    case done
    case abandoned

    public var label: String { rawValue.replacingOccurrences(of: "_", with: " ") }
    public var isTerminal: Bool { self == .done || self == .abandoned }
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
        self.id = switch kind {
        case .wave: StructuredIdentity.uuid(id)
        case .project: StructuredIdentity.prefixed(id, "proj_")
        case .task: StructuredIdentity.prefixed(id, "task_")
        }
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
    case named(id: String)
    case captured(id: Int64)
    case imported(id: String)

    private enum CodingKeys: String, CodingKey {
        case kind, id
    }

    private enum Kind: String, Codable {
        case user, named, captured, imported
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(Kind.self, forKey: .kind) {
        case .user:
            self = .user
        case .named:
            self = .named(id: try container.decode(String.self, forKey: .id))
        case .captured:
            self = .captured(id: try container.decode(Int64.self, forKey: .id))
        case .imported:
            self = .imported(id: try container.decode(String.self, forKey: .id))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .user:
            try container.encode(Kind.user, forKey: .kind)
        case .named(let id):
            try container.encode(Kind.named, forKey: .kind)
            try container.encode(id, forKey: .id)
        case .captured(let id):
            try container.encode(Kind.captured, forKey: .kind)
            try container.encode(id, forKey: .id)
        case .imported(let id):
            try container.encode(Kind.imported, forKey: .kind)
            try container.encode(id, forKey: .id)
        }
    }
}

extension WorkReference {
    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        self.init(kind: try values.decode(Kind.self, forKey: .kind), id: try values.decode(String.self, forKey: .id))
    }
}
