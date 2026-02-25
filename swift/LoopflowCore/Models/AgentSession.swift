import Foundation

public enum JSONValue: Sendable, Hashable {
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
}

public enum ItemStatus: String, Sendable, Hashable {
    case inProgress = "in_progress"
    case completed
    case failed
    case declined
}

public struct FileEdit: Sendable, Hashable {
    public let path: String
    public let kind: String?
    public let diff: String?

    public init(path: String, kind: String? = nil, diff: String? = nil) {
        self.path = path
        self.kind = kind
        self.diff = diff
    }
}

public struct CommandItem: Sendable, Hashable {
    public let id: String
    public let command: [String]
    public let cwd: String
    public let status: ItemStatus
    public let output: String?
    public let exitCode: Int?
    public let durationMs: UInt64?

    public init(
        id: String,
        command: [String] = [],
        cwd: String = "",
        status: ItemStatus,
        output: String? = nil,
        exitCode: Int? = nil,
        durationMs: UInt64? = nil
    ) {
        self.id = id
        self.command = command
        self.cwd = cwd
        self.status = status
        self.output = output
        self.exitCode = exitCode
        self.durationMs = durationMs
    }
}

public struct FileItem: Sendable, Hashable {
    public let id: String
    public let changes: [FileEdit]
    public let status: ItemStatus

    public init(id: String, changes: [FileEdit] = [], status: ItemStatus) {
        self.id = id
        self.changes = changes
        self.status = status
    }
}

public struct MessageItem: Sendable, Hashable {
    public let id: String
    public let text: String
    public let phase: String?

    public init(id: String, text: String = "", phase: String? = nil) {
        self.id = id
        self.text = text
        self.phase = phase
    }
}

public struct ThoughtItem: Sendable, Hashable {
    public let id: String
    public let text: String

    public init(id: String, text: String = "") {
        self.id = id
        self.text = text
    }
}

public struct ToolItem: Sendable, Hashable {
    public let id: String
    public let name: String
    public let status: ItemStatus
    public let input: JSONValue?
    public let output: String?

    public init(
        id: String,
        name: String,
        status: ItemStatus,
        input: JSONValue? = nil,
        output: String? = nil
    ) {
        self.id = id
        self.name = name
        self.status = status
        self.input = input
        self.output = output
    }
}

public enum SessionItem: Sendable, Hashable {
    case command(CommandItem)
    case file(FileItem)
    case message(MessageItem)
    case thought(ThoughtItem)
    case tool(ToolItem)
    case unknown(type: String, payload: JSONValue)

    public var id: String? {
        switch self {
        case .command(let item): return item.id
        case .file(let item): return item.id
        case .message(let item): return item.id
        case .thought(let item): return item.id
        case .tool(let item): return item.id
        case .unknown: return nil
        }
    }

    public var status: ItemStatus? {
        switch self {
        case .command(let item): return item.status
        case .file(let item): return item.status
        case .tool(let item): return item.status
        case .message, .thought, .unknown:
            return nil
        }
    }
}

public enum ItemDelta: Sendable, Hashable {
    case output(content: String)
    case planText(content: String)
    case unknown(type: String, payload: JSONValue)
}

public struct AgentSessionConfig: Sendable, Hashable {
    public var step: String
    public var repoRoot: String
    public var directions: [String]
    public var area: String?
    public var wave: String?
    public var message: String?
    public var model: String?
    public var cwd: String?
    public var maxTurns: Int?
    public var yoloMode: Bool

    public init(
        step: String,
        repoRoot: String,
        directions: [String] = [],
        area: String? = nil,
        wave: String? = nil,
        message: String? = nil,
        model: String? = nil,
        cwd: String? = nil,
        maxTurns: Int? = nil,
        yoloMode: Bool = false
    ) {
        self.step = step
        self.repoRoot = repoRoot
        self.directions = directions
        self.area = area
        self.wave = wave
        self.message = message
        self.model = model
        self.cwd = cwd
        self.maxTurns = maxTurns
        self.yoloMode = yoloMode
    }
}

public struct AgentSession: Sendable, Hashable {
    public let id: String
    public let harness: String
    public let status: String
    public let waveRunId: String?
    public let providerSessionId: String?
    public let config: AgentSessionConfig
    public let createdAt: Date?
    public let endedAt: Date?

    public init(
        id: String,
        harness: String,
        status: String,
        waveRunId: String?,
        providerSessionId: String?,
        config: AgentSessionConfig,
        createdAt: Date?,
        endedAt: Date?
    ) {
        self.id = id
        self.harness = harness
        self.status = status
        self.waveRunId = waveRunId
        self.providerSessionId = providerSessionId
        self.config = config
        self.createdAt = createdAt
        self.endedAt = endedAt
    }
}

public enum AgentSessionEvent: Sendable, Hashable {
    case turnStarted(turnId: String)
    case turnCompleted(turnId: String, status: String)
    case itemStarted(turnId: String, item: SessionItem)
    case itemUpdated(turnId: String, itemId: String, delta: ItemDelta)
    case itemCompleted(turnId: String, item: SessionItem)
    case textDelta(turnId: String, content: String)
    case reasoningDelta(turnId: String, content: String)
    case diffUpdated(turnId: String, diff: String)
    case statusChanged(status: String)
    case error(code: String, message: String)
    case other(type: String, payload: JSONValue)
}

public struct AgentSessionEventEnvelope: Sendable, Hashable {
    public let seq: Int?
    public let event: AgentSessionEvent?
    public let replayCompletedLastSeq: Int?

    public init(seq: Int?, event: AgentSessionEvent) {
        self.seq = seq
        self.event = event
        self.replayCompletedLastSeq = nil
    }

    public init(replayCompletedLastSeq: Int?) {
        self.seq = nil
        self.event = nil
        self.replayCompletedLastSeq = replayCompletedLastSeq
    }
}
