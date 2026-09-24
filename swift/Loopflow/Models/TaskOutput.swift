import Foundation

public struct TaskFlowStage: Codable, Sendable, Hashable {
    public let invocationId: String
    public let stepIndex: UInt32
    public let iteration: UInt32

    enum CodingKeys: String, CodingKey {
        case invocationId = "invocation_id"
        case stepIndex = "step_index"
        case iteration
    }
}

public enum OutputSource: String, Codable, Sendable, Hashable {
    case journal, claude, codex
    case openCode = "open_code"
}

public struct OutputGap: Decodable, Sendable, Hashable {
    public let code: String
    public let message: String
}

public struct TaskOutputSource: Decodable, Sendable {
    public let runId: String
    public let provider: String
    public let stage: TaskFlowStage?
    public var records: [OutputRecord]
    public let source: OutputSource
    public let available: Bool
    public let hasMore: Bool
    public let reset: Bool
    public let gaps: [OutputGap]

    enum CodingKeys: String, CodingKey {
        case runId = "run_id"
        case hasMore = "has_more"
        case provider, stage, records, source, available, reset, gaps
    }
}

public struct OutputRecord: Decodable, Sendable {
    public let sourceItemId: String
    public let revision: String
    public let event: ConversationEvent

    enum CodingKeys: String, CodingKey {
        case sourceItemId = "source_item_id"
        case revision, event
    }
}

public struct TaskOutputPage: Decodable, Sendable {
    public let taskId: String
    public let sources: [TaskOutputSource]
    public let gaps: [OutputGap]
    public let nextCursor: String

    enum CodingKeys: String, CodingKey {
        case taskId = "task_id"
        case nextCursor = "next_cursor"
        case sources, gaps
    }
}

/// The normalized Rust conversation stream. Item snapshots retain their identity
/// so native revisions replace a displayed item rather than duplicating its text.
public enum ConversationEvent: Decodable, Sendable {
    case turnStarted(turnId: String)
    case turnCompleted(turnId: String, status: Lifecycle)
    case itemStarted(turnId: String, item: ConversationItem)
    case itemCompleted(turnId: String, item: ConversationItem)
    case itemUpdated(turnId: String, itemId: String, data: ConversationItemDelta)
    case textDelta(turnId: String, content: String)
    case reasoningDelta(turnId: String, content: String)
    case error(code: String, message: String, evidence: JSONValue?)
    case usageCheckpoint(turnId: String, usage: JSONValue, finalReceipt: Bool)
    case diffUpdated(turnId: String, diff: String)
    case suggestedActions(turnId: String, actions: [JSONValue])
    case statusChanged(status: String)

    enum CodingKeys: String, CodingKey {
        case type, status, item, data, content, code, message, evidence, usage, diff, actions
        case turnId = "turn_id"
        case itemId = "item_id"
        case finalReceipt = "final_receipt"
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let type = try c.decode(String.self, forKey: .type)
        switch type {
        case "turn_started": self = .turnStarted(turnId: try c.decode(String.self, forKey: .turnId))
        case "turn_completed": self = .turnCompleted(turnId: try c.decode(String.self, forKey: .turnId), status: try c.decode(Lifecycle.self, forKey: .status))
        case "item_started": self = .itemStarted(turnId: try c.decode(String.self, forKey: .turnId), item: try c.decode(ConversationItem.self, forKey: .item))
        case "item_completed": self = .itemCompleted(turnId: try c.decode(String.self, forKey: .turnId), item: try c.decode(ConversationItem.self, forKey: .item))
        case "item_updated": self = .itemUpdated(turnId: try c.decode(String.self, forKey: .turnId), itemId: try c.decode(String.self, forKey: .itemId), data: try c.decode(ConversationItemDelta.self, forKey: .data))
        case "text_delta": self = .textDelta(turnId: try c.decode(String.self, forKey: .turnId), content: try c.decode(String.self, forKey: .content))
        case "reasoning_delta": self = .reasoningDelta(turnId: try c.decode(String.self, forKey: .turnId), content: try c.decode(String.self, forKey: .content))
        case "error": self = .error(code: try c.decode(String.self, forKey: .code), message: try c.decode(String.self, forKey: .message), evidence: try c.decodeIfPresent(JSONValue.self, forKey: .evidence))
        case "usage_checkpoint": self = .usageCheckpoint(turnId: try c.decode(String.self, forKey: .turnId), usage: try c.decode(JSONValue.self, forKey: .usage), finalReceipt: try c.decode(Bool.self, forKey: .finalReceipt))
        case "diff_updated": self = .diffUpdated(turnId: try c.decode(String.self, forKey: .turnId), diff: try c.decode(String.self, forKey: .diff))
        case "suggested_actions": self = .suggestedActions(turnId: try c.decode(String.self, forKey: .turnId), actions: try c.decode([JSONValue].self, forKey: .actions))
        case "status_changed": self = .statusChanged(status: try c.decode(String.self, forKey: .status))
        default: throw DecodingError.dataCorruptedError(forKey: .type, in: c, debugDescription: "Unknown conversation event: \(type)")
        }
    }
}

public enum ConversationItemDelta: Decodable, Sendable {
    case output(String)
    case planText(String)

    enum CodingKeys: String, CodingKey { case type, content }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let type = try c.decode(String.self, forKey: .type)
        let content = try c.decode(String.self, forKey: .content)
        switch type {
        case "output": self = .output(content)
        case "plan_text": self = .planText(content)
        default: throw DecodingError.dataCorruptedError(forKey: .type, in: c, debugDescription: "Unknown item delta: \(type)")
        }
    }
}
