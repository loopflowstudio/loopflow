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

public struct AgentSessionConfig: Sendable, Hashable {
    public var step: String
    public var repoRoot: String
    public var directions: [String]
    public var area: String?
    public var wave: String?
    public var message: String?
    public var agent: String?
    public var cwd: String?
    public var maxTurns: Int?
    public var yoloMode: Bool
    public var clientHasUI: Bool?
    public var clientCompact: Bool?

    public init(
        step: String,
        repoRoot: String,
        directions: [String] = [],
        area: String? = nil,
        wave: String? = nil,
        message: String? = nil,
        agent: String? = nil,
        cwd: String? = nil,
        maxTurns: Int? = nil,
        yoloMode: Bool = false,
        clientHasUI: Bool? = nil,
        clientCompact: Bool? = nil
    ) {
        self.step = step
        self.repoRoot = repoRoot
        self.directions = directions
        self.area = area
        self.wave = wave
        self.message = message
        self.agent = agent
        self.cwd = cwd
        self.maxTurns = maxTurns
        self.yoloMode = yoloMode
        self.clientHasUI = clientHasUI
        self.clientCompact = clientCompact
    }
}

public struct AgentSession: Sendable, Hashable {
    public let id: String
    public let harness: String
    public let status: String
    public let runId: String?
    public let providerSessionId: String?
    public let inputSupported: Bool
    public let config: AgentSessionConfig
    public let createdAt: Date?
    public let endedAt: Date?

    public init(
        id: String,
        harness: String,
        status: String,
        runId: String?,
        providerSessionId: String?,
        inputSupported: Bool,
        config: AgentSessionConfig,
        createdAt: Date?,
        endedAt: Date?
    ) {
        self.id = id
        self.harness = harness
        self.status = status
        self.runId = runId
        self.providerSessionId = providerSessionId
        self.inputSupported = inputSupported
        self.config = config
        self.createdAt = createdAt
        self.endedAt = endedAt
    }
}

public struct DocumentEntry: Sendable, Hashable {
    public let path: String
    public let source: String
    public let tokens: UInt64

    public init(path: String, source: String, tokens: UInt64) {
        self.path = path
        self.source = source
        self.tokens = tokens
    }
}

public struct ContextSnapshot: Sendable, Hashable {
    public let sources: [String: UInt64]
    public let sourceCounts: [String: UInt64]
    public let documents: [DocumentEntry]
    public let total: UInt64
    public let diffTier: String
    public let stepName: String?
    public let directionNames: [String]
    public let waveName: String?
    public let hasClipboard: Bool

    public init(
        sources: [String: UInt64],
        sourceCounts: [String: UInt64],
        documents: [DocumentEntry],
        total: UInt64,
        diffTier: String,
        stepName: String?,
        directionNames: [String],
        waveName: String?,
        hasClipboard: Bool
    ) {
        self.sources = sources
        self.sourceCounts = sourceCounts
        self.documents = documents
        self.total = total
        self.diffTier = diffTier
        self.stepName = stepName
        self.directionNames = directionNames
        self.waveName = waveName
        self.hasClipboard = hasClipboard
    }
}
