import Foundation

// Wire models for a wave's live conversation, mirroring the Rust types the wave
// chat server serves (`chat::turns::ChatTurn` + `chat::types::ConversationItem`).
// snake_case on the wire; every
// field is required or explicitly Optional — no defaults masking absent fields,
// so the Rust and Swift shapes stay in lockstep (see CLAUDE.md "DTOs").

/// Who authored a turn. Mirrors Rust `ChatRole`.
public enum ChatRole: String, Codable, Sendable, Hashable {
    case user
    case assistant
}

public enum ChildActivitySubject: String, Codable, Sendable, Hashable {
    case project, task
}

public enum ChildActivityKind: String, Codable, Sendable, Hashable {
    case stateChanged = "state_changed"
    case prOpened = "pr_opened"
    case completed
    case failed
}

public struct ChildControlActivity: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let subject: ChildActivitySubject
    public let subjectId: String
    public let workId: String
    public let kind: ChildActivityKind
    public let title: String
    public let summary: String

    private enum CodingKeys: String, CodingKey {
        case id, subject, kind, title, summary
        case subjectId = "subject_id"
        case workId = "work_id"
    }
}

public enum PlayheadStepKind: String, Codable, Sendable, Hashable {
    case skill, op, and, xor, or, loop
}

public struct PlayheadStepPlan: Codable, Sendable, Hashable {
    public let name: String
    public let kind: PlayheadStepKind
}

public struct QueuedInvocation: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let flow: String
    public let steps: [PlayheadStepPlan]
}

public struct InvocationState: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let flow: String
    public let steps: [PlayheadStepPlan]
    public let cursor: Int
    public let iteration: Int
    public let queue: [QueuedInvocation]
}

public struct PlayheadStepRef: Codable, Sendable, Hashable {
    public let invocationId: String
    public let flow: String
    public let step: String
    public let kind: PlayheadStepKind
    public let index: Int
    public let total: Int
    public let iteration: Int

    private enum CodingKeys: String, CodingKey {
        case flow, step, kind, index, total, iteration
        case invocationId = "invocation_id"
    }
}

public struct BodyProvenance: Codable, Sendable, Hashable {
    public let bodyId: String
    public let invocationId: String
    public let stepIndex: Int
    public let flow: String
    public let step: String
    public let iteration: Int
    public let sessionId: String?
    public let harness: String?
    public let model: String?
    public let host: String
    public let worktree: String
    public let startedAt: String
    public let endedAt: String?
    public let terminationReason: String?

    private enum CodingKeys: String, CodingKey {
        case flow, step, iteration, harness, model, host, worktree
        case bodyId = "body_id"
        case invocationId = "invocation_id"
        case stepIndex = "step_index"
        case sessionId = "session_id"
        case startedAt = "started_at"
        case endedAt = "ended_at"
        case terminationReason = "termination_reason"
    }
}

public struct PlayheadView: Codable, Sendable, Hashable {
    public let stack: [InvocationState]
    public let active: BodyProvenance?
    public let now: PlayheadStepRef?
    public let next: PlayheadStepRef?
    public let returnTo: PlayheadStepRef?

    private enum CodingKeys: String, CodingKey {
        case stack, active, now, next
        case returnTo = "return_to"
    }
}

// `Lifecycle` and `FileEdit` are shared conversation wire types.
// One `Lifecycle` covers turns and items; a `user` turn is always `completed`.

/// A tool/command/file/message/thought item the agent produced, serde-tagged by
/// `type` on the wire. Unknown tags decode as `.unknown` rather than throwing, so
/// a newer server that grows the enum doesn't break an older client.
public enum ConversationItem: Codable, Sendable, Hashable, Identifiable {
    case command(id: String, command: [String], cwd: String, status: Lifecycle, output: String?, exitCode: Int?, durationMs: Int?)
    case file(id: String, changes: [FileEdit], status: Lifecycle)
    case message(id: String, text: String, phase: String?)
    case thought(id: String, text: String)
    case tool(id: String, name: String, status: Lifecycle, input: JSONValue?, output: String?)
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

    public var isVisibleInConversation: Bool {
        if case let .thought(_, text) = self {
            return !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
        return true
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
                input: try c.decodeIfPresent(JSONValue.self, forKey: .input),
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
}

/// One turn in a wave's conversation — the unit the chat server streams.
public enum ChatTurnError: Error, Equatable {
    case mixedActivity
    case invalidActivityEnvelope
    case invalidHumanTurn
}

public struct ChatTurn: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let role: ChatRole
    public let text: String
    public let status: Lifecycle
    public let items: [ConversationItem]
    public let createdAt: String
    /// Body that produced an assistant span; nil for human turns.
    public let body: BodyProvenance?
    public let activity: ChildControlActivity?

    private enum CodingKeys: String, CodingKey {
        case id, role, text, status, items, body, activity
        case createdAt = "created_at"
    }

    public init(
        id: String,
        role: ChatRole,
        text: String,
        status: Lifecycle,
        items: [ConversationItem],
        createdAt: String,
        body: BodyProvenance?,
        activity: ChildControlActivity?
    ) throws {
        self.id = id
        self.role = role
        self.text = text
        self.status = status
        self.items = items
        self.createdAt = createdAt
        self.body = body
        self.activity = activity
        try validate()
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            id: values.decode(String.self, forKey: .id),
            role: values.decode(ChatRole.self, forKey: .role),
            text: values.decode(String.self, forKey: .text),
            status: values.decode(Lifecycle.self, forKey: .status),
            items: values.decode([ConversationItem].self, forKey: .items),
            createdAt: values.decode(String.self, forKey: .createdAt),
            body: values.decodeIfPresent(BodyProvenance.self, forKey: .body),
            activity: values.decodeIfPresent(ChildControlActivity.self, forKey: .activity)
        )
    }

    private func validate() throws {
        if activity != nil {
            guard text.isEmpty, items.isEmpty, body == nil else {
                throw ChatTurnError.mixedActivity
            }
            guard role == .user, status == .completed else {
                throw ChatTurnError.invalidActivityEnvelope
            }
        } else if role == .user {
            guard status == .completed, items.isEmpty, body == nil else {
                throw ChatTurnError.invalidHumanTurn
            }
        }
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

    /// Grow this turn by one increment, mirroring the listener's
    /// `ChatTurn::absorb_item`: a `stream` fragment concatenates into `text`,
    /// any other prose message joins newline-separated, and every other item
    /// appends to `items`. The one exception is `commentary` — operational
    /// narration the provider tagged as process, not conclusion — which stays a
    /// discrete item so `turnPresentation` can curate it behind a disclosure.
    /// The reader applies this to each incremental turn frame so its open-turn
    /// reconstruction matches the served turn without the whole turn crossing
    /// the wire per token.
    public func absorbing(_ item: ConversationItem) throws -> ChatTurn {
        var grownText = text
        var grownItems = items
        if case let .message(_, messageText, phase) = item {
            switch phase {
            case "stream":
                grownText += messageText
            case "commentary":
                grownItems.append(item)
            default:
                if !grownText.isEmpty { grownText += "\n" }
                grownText += messageText
            }
        } else {
            grownItems.append(item)
        }
        return try ChatTurn(
            id: id,
            role: role,
            text: grownText,
            status: status,
            items: grownItems,
            createdAt: createdAt,
            body: body,
            activity: activity
        )
    }

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

/// One live increment to an open turn. Human chat carries it in
/// `message-delta`; the resident stream uses `turn-delta`. Mirrors Rust
/// `TurnDelta`; the reader applies it with [`ChatTurn/absorbing(_:)`] against
/// the turn named by `turnId`.
public struct TurnDelta: Codable, Sendable, Hashable {
    public let turnId: String
    public let item: ConversationItem

    private enum CodingKeys: String, CodingKey {
        case turnId = "turn_id"
        case item
    }

    public init(turnId: String, item: ConversationItem) {
        self.turnId = turnId
        self.item = item
    }
}
