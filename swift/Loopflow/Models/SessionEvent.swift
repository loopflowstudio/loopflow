import Foundation

/// Provider-issued opaque conversation identity; encoded as the provider's string.
public struct AgentSessionId: RawRepresentable, Codable, Sendable, Hashable {
    public let rawValue: String

    public init(rawValue: String) { self.rawValue = rawValue }
}

/// Native receipts retain missing attribution and provider-specific evidence.
public struct SessionEvent: Codable, Sendable, Equatable, Identifiable {
    public let seq: Int64
    public let sessionID: String
    public let agentSession: AgentSessionId?
    public let providerTurn: String?
    public let kind: Kind
    public let providerGeneration: Int64?
    public let lfProcessId: String?
    public let taskID: String?
    public let waveID: String?
    public let observedAt: Int64
    public let payload: JSONValue

    public var id: Int64 { seq }

    public enum Kind: String, Codable, Sendable {
        case captured, started, usage, completed, output, observed
    }

    enum CodingKeys: String, CodingKey {
        case seq, kind, payload
        case sessionID = "session_id"
        case agentSession = "provider_thread"
        case providerTurn = "provider_turn"
        case providerGeneration = "provider_generation"
        case lfProcessId = "lf_process_id"
        case taskID = "task_id"
        case waveID = "wave_id"
        case observedAt = "observed_at"
    }
}

/// Lossless JSON structure for provider-authored receipt payloads.
public enum JSONValue: Codable, Sendable, Hashable {
    case null
    case bool(Bool)
    case integer(Int64)
    case number(Double)
    case string(String)
    case array([JSONValue])
    case object([String: JSONValue])

    public init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer()
        if value.decodeNil() { self = .null }
        else if let decoded = try? value.decode(Bool.self) { self = .bool(decoded) }
        else if let decoded = try? value.decode(Int64.self) { self = .integer(decoded) }
        else if let decoded = try? value.decode(Double.self) { self = .number(decoded) }
        else if let decoded = try? value.decode(String.self) { self = .string(decoded) }
        else if let decoded = try? value.decode([JSONValue].self) { self = .array(decoded) }
        else { self = .object(try value.decode([String: JSONValue].self)) }
    }

    public func encode(to encoder: Encoder) throws {
        var value = encoder.singleValueContainer()
        switch self {
        case .null: try value.encodeNil()
        case .bool(let decoded): try value.encode(decoded)
        case .integer(let decoded): try value.encode(decoded)
        case .number(let decoded): try value.encode(decoded)
        case .string(let decoded): try value.encode(decoded)
        case .array(let decoded): try value.encode(decoded)
        case .object(let decoded): try value.encode(decoded)
        }
    }
}
