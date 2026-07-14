import Foundation

public enum JSONValue: Sendable, Hashable, Codable {
    case object([String: JSONValue])
    case array([JSONValue])
    case string(String)
    case number(Double)
    case bool(Bool)
    case null

    public var stringValue: String? {
        if case .string(let value) = self {
            return value
        }
        return nil
    }

    public var objectValue: [String: JSONValue]? {
        if case .object(let value) = self {
            return value
        }
        return nil
    }

    public var displayString: String {
        switch self {
        case let .string(value):
            return value
        case let .number(value):
            if value.isFinite,
               value == value.rounded(),
               value >= Double(Int.min),
               value <= Double(Int.max) {
                return String(Int(value))
            }
            return String(value)
        case let .bool(value):
            return String(value)
        case .null:
            return "null"
        case let .array(value):
            return "[" + value.map(\.displayString).joined(separator: ", ") + "]"
        case let .object(value):
            return "{" + value.sorted { $0.key < $1.key }
                .map { "\($0.key): \($0.value.displayString)" }
                .joined(separator: ", ") + "}"
        }
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            self = .null
        } else if let value = try? container.decode(Bool.self) {
            self = .bool(value)
        } else if let value = try? container.decode(Double.self) {
            self = .number(value)
        } else if let value = try? container.decode(String.self) {
            self = .string(value)
        } else if let value = try? container.decode([JSONValue].self) {
            self = .array(value)
        } else if let value = try? container.decode([String: JSONValue].self) {
            self = .object(value)
        } else {
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "Unsupported JSON value"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case let .object(value):
            try container.encode(value)
        case let .array(value):
            try container.encode(value)
        case let .string(value):
            try container.encode(value)
        case let .number(value):
            try container.encode(value)
        case let .bool(value):
            try container.encode(value)
        case .null:
            try container.encodeNil()
        }
    }
}

/// Lifecycle of a turn or item. Mirrors Rust `Lifecycle` (types.rs); one enum
/// spans turns and items on the wire.
public enum Lifecycle: String, Sendable, Hashable, Codable {
    case pending
    case running
    case completed
    case failed
    case interrupted
}

public struct FileEdit: Sendable, Hashable, Codable {
    public let path: String
    public let kind: String?
    public let diff: String?

    // No default parameters: FileEdit crosses the wire (ConversationItem.file),
    // and DTO fields are required or explicitly Optional — see CLAUDE.md "DTOs".
    public init(path: String, kind: String?, diff: String?) {
        self.path = path
        self.kind = kind
        self.diff = diff
    }
}
