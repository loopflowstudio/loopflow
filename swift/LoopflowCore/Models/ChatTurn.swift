import Foundation

// Wire models for a wave's live conversation, mirroring the Rust types the wave
// chat server serves (`lfd::conversations::turns::ChatTurn` +
// `lfd::conversations::types::ConversationItem`). snake_case on the wire; every
// field is required or explicitly Optional — no defaults masking absent fields,
// so the Rust and Swift shapes stay in lockstep (see CLAUDE.md "DTOs").

/// Who authored a turn. Mirrors Rust `ChatRole`.
public enum ChatRole: String, Codable, Sendable, Hashable {
    case user
    case assistant
}

// `Lifecycle` and `FileEdit` are shared with the session models in
// AgentSession.swift (made `Codable` there); the wire shape is identical.
// One `Lifecycle` covers turns and items; a `user` turn is always `completed`.

/// A tool/command/file/message/thought item the agent produced, serde-tagged by
/// `type` on the wire. Unknown tags decode as `.unknown` rather than throwing, so
/// a newer server that grows the enum doesn't break an older client.
public enum ConversationItem: Codable, Sendable, Hashable, Identifiable {
    case command(id: String, command: [String], cwd: String, status: Lifecycle, output: String?, exitCode: Int?, durationMs: Int?)
    case file(id: String, changes: [FileEdit], status: Lifecycle)
    case message(id: String, text: String, phase: String?)
    case thought(id: String, text: String)
    case tool(id: String, name: String, status: Lifecycle, input: String?, output: String?)
    case unknown(id: String, type: String)

    public var id: String {
        switch self {
        case let .command(id, _, _, _, _, _, _): return id
        case let .file(id, _, _): return id
        case let .message(id, _, _): return id
        case let .thought(id, _): return id
        case let .tool(id, _, _, _, _): return id
        case let .unknown(id, _): return id
        }
    }

    private enum CodingKeys: String, CodingKey {
        case type, id, command, cwd, status, output
        case exitCode = "exit_code"
        case durationMs = "duration_ms"
        case changes, name, input, text, phase
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let type = try c.decode(String.self, forKey: .type)
        let id = try c.decode(String.self, forKey: .id)
        switch type {
        case "command":
            self = .command(
                id: id,
                command: try c.decode([String].self, forKey: .command),
                cwd: try c.decode(String.self, forKey: .cwd),
                status: try c.decode(Lifecycle.self, forKey: .status),
                output: try c.decodeIfPresent(String.self, forKey: .output),
                exitCode: try c.decodeIfPresent(Int.self, forKey: .exitCode),
                durationMs: try c.decodeIfPresent(Int.self, forKey: .durationMs)
            )
        case "file":
            self = .file(
                id: id,
                changes: try c.decode([FileEdit].self, forKey: .changes),
                status: try c.decode(Lifecycle.self, forKey: .status)
            )
        case "message":
            self = .message(
                id: id,
                text: try c.decode(String.self, forKey: .text),
                phase: try c.decodeIfPresent(String.self, forKey: .phase)
            )
        case "thought":
            self = .thought(id: id, text: try c.decode(String.self, forKey: .text))
        case "tool":
            self = .tool(
                id: id,
                name: try c.decode(String.self, forKey: .name),
                status: try c.decode(Lifecycle.self, forKey: .status),
                input: try Self.decodeLooseString(c, forKey: .input),
                output: try c.decodeIfPresent(String.self, forKey: .output)
            )
        default:
            self = .unknown(id: id, type: type)
        }
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(id, forKey: .id)
        switch self {
        case let .command(_, command, cwd, status, output, exitCode, durationMs):
            try c.encode("command", forKey: .type)
            try c.encode(command, forKey: .command)
            try c.encode(cwd, forKey: .cwd)
            try c.encode(status, forKey: .status)
            try c.encodeIfPresent(output, forKey: .output)
            try c.encodeIfPresent(exitCode, forKey: .exitCode)
            try c.encodeIfPresent(durationMs, forKey: .durationMs)
        case let .file(_, changes, status):
            try c.encode("file", forKey: .type)
            try c.encode(changes, forKey: .changes)
            try c.encode(status, forKey: .status)
        case let .message(_, text, phase):
            try c.encode("message", forKey: .type)
            try c.encode(text, forKey: .text)
            try c.encodeIfPresent(phase, forKey: .phase)
        case let .thought(_, text):
            try c.encode("thought", forKey: .type)
            try c.encode(text, forKey: .text)
        case let .tool(_, name, status, input, output):
            try c.encode("tool", forKey: .type)
            try c.encode(name, forKey: .name)
            try c.encode(status, forKey: .status)
            try c.encodeIfPresent(input, forKey: .input)
            try c.encodeIfPresent(output, forKey: .output)
        case let .unknown(_, type):
            try c.encode(type, forKey: .type)
        }
    }

    /// `tool.input` is arbitrary JSON on the wire. Render it as a string: pass a
    /// JSON string through, pretty-print an object/array, or stringify a scalar.
    private static func decodeLooseString(_ c: KeyedDecodingContainer<CodingKeys>, forKey key: CodingKeys) throws -> String? {
        if let s = try? c.decodeIfPresent(String.self, forKey: key) { return s }
        guard c.contains(key) else { return nil }
        if let obj = try? c.decode(JSONValue.self, forKey: key) { return obj.displayString }
        return nil
    }
}

/// One turn in a wave's conversation — the unit the chat server streams.
public struct ChatTurn: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let role: ChatRole
    public let text: String
    public let status: Lifecycle
    public let items: [ConversationItem]
    public let createdAt: String

    private enum CodingKeys: String, CodingKey {
        case id, role, text, status, items
        case createdAt = "created_at"
    }

    public init(id: String, role: ChatRole, text: String, status: Lifecycle, items: [ConversationItem], createdAt: String) {
        self.id = id
        self.role = role
        self.text = text
        self.status = status
        self.items = items
        self.createdAt = createdAt
    }

    /// Monotonic sequence parsed from a `"turn-<n>"` id; used to order the thread.
    public var sequence: Int {
        guard id.hasPrefix("turn-"), let n = Int(id.dropFirst("turn-".count)) else { return .max }
        return n
    }

    public var createdAtDate: Date? {
        ChatTurn.rfc3339.date(from: createdAt) ?? ChatTurn.rfc3339Fractional.date(from: createdAt)
    }

    public var isInProgress: Bool { status == .running }

    // ISO8601DateFormatter is safe for concurrent read-only formatting but isn't
    // marked Sendable; these are only ever read.
    nonisolated(unsafe) private static let rfc3339: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime]
        return f
    }()

    nonisolated(unsafe) private static let rfc3339Fractional: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()
}

// `tool.input` is arbitrary JSON on the wire. Decode it into the shared
// `JSONValue` (AgentSession.swift) and flatten to a display string.
extension JSONValue: Decodable {
    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() { self = .null }
        else if let b = try? c.decode(Bool.self) { self = .bool(b) }
        else if let n = try? c.decode(Double.self) { self = .number(n) }
        else if let s = try? c.decode(String.self) { self = .string(s) }
        else if let a = try? c.decode([JSONValue].self) { self = .array(a) }
        else if let o = try? c.decode([String: JSONValue].self) { self = .object(o) }
        else { self = .null }
    }

    var displayString: String {
        switch self {
        case let .string(s): return s
        case let .number(n): return n == n.rounded() ? String(Int(n)) : String(n)
        case let .bool(b): return String(b)
        case .null: return "null"
        case let .array(a): return "[" + a.map(\.displayString).joined(separator: ", ") + "]"
        case let .object(o):
            return "{" + o.sorted { $0.key < $1.key }.map { "\($0.key): \($0.value.displayString)" }.joined(separator: ", ") + "}"
        }
    }
}
