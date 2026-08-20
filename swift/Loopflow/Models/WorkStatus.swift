import Foundation

public enum WorkStatus: Codable, Sendable, Hashable {
    case ready
    case running(runID: String)
    case waiting(wait: WorkWait)
    case done
    case abandoned

    private enum CodingKeys: String, CodingKey {
        case running
        case waiting
    }

    private enum RunningKeys: String, CodingKey {
        case runID = "run_id"
    }

    private enum WaitingKeys: String, CodingKey {
        case wait
    }

    public init(from decoder: Decoder) throws {
        if let value = try? decoder.singleValueContainer().decode(String.self) {
            switch value {
            case "ready": self = .ready
            case "done": self = .done
            case "abandoned": self = .abandoned
            default:
                throw DecodingError.dataCorruptedError(
                    in: try decoder.singleValueContainer(),
                    debugDescription: "unknown Work status \(value)"
                )
            }
            return
        }

        let container = try decoder.container(keyedBy: CodingKeys.self)
        if container.contains(.running) {
            let running = try container.nestedContainer(keyedBy: RunningKeys.self, forKey: .running)
            self = .running(runID: try running.decode(String.self, forKey: .runID))
            return
        }
        if container.contains(.waiting) {
            let waiting = try container.nestedContainer(keyedBy: WaitingKeys.self, forKey: .waiting)
            self = .waiting(wait: try waiting.decode(WorkWait.self, forKey: .wait))
            return
        }
        throw DecodingError.dataCorrupted(
            DecodingError.Context(
                codingPath: decoder.codingPath,
                debugDescription: "Work status must be ready, running, waiting, done, or abandoned"
            )
        )
    }

    public func encode(to encoder: Encoder) throws {
        switch self {
        case .ready:
            var container = encoder.singleValueContainer()
            try container.encode("ready")
        case .running(let runID):
            var container = encoder.container(keyedBy: CodingKeys.self)
            var running = container.nestedContainer(keyedBy: RunningKeys.self, forKey: .running)
            try running.encode(runID, forKey: .runID)
        case .waiting(let wait):
            var container = encoder.container(keyedBy: CodingKeys.self)
            var waiting = container.nestedContainer(keyedBy: WaitingKeys.self, forKey: .waiting)
            try waiting.encode(wait, forKey: .wait)
        case .done:
            var container = encoder.singleValueContainer()
            try container.encode("done")
        case .abandoned:
            var container = encoder.singleValueContainer()
            try container.encode("abandoned")
        }
    }

    public var isRunning: Bool {
        if case .running = self { return true }
        return false
    }

    public var label: String {
        switch self {
        case .ready: "ready"
        case .running: "running"
        case .waiting: "waiting"
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

public struct WorkBasis: Codable, Sendable, Hashable {
    public let epochID: String
    public let revision: UInt64

    private enum CodingKeys: String, CodingKey {
        case epochID = "epoch_id"
        case revision
    }
}

public struct WorkEventReference: Codable, Sendable, Hashable {
    public let source: String
    public let id: String
}

public struct WorkCapabilityReference: Codable, Sendable, Hashable {
    public let kind: String
    public let key: String
}

public struct WorkEffectReference: Codable, Sendable, Hashable {
    public let kind: String
    public let idempotencyKey: String

    private enum CodingKeys: String, CodingKey {
        case kind
        case idempotencyKey = "idempotency_key"
    }
}

public enum WorkWaitOn: Codable, Sendable, Hashable {
    case input(after: WorkBasis)
    case time(notBefore: String)
    case event(WorkEventReference)
    case child(WorkReference)
    case capability(WorkCapabilityReference)
    case effect(WorkEffectReference)

    private enum CodingKeys: String, CodingKey {
        case kind
        case after
        case notBefore = "not_before"
        case event
        case work
        case capability
        case effect
    }

    private enum Kind: String, Codable {
        case input
        case time
        case event
        case child
        case capability
        case effect
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(Kind.self, forKey: .kind) {
        case .input: self = .input(after: try container.decode(WorkBasis.self, forKey: .after))
        case .time:
            self = .time(notBefore: try container.decode(String.self, forKey: .notBefore))
        case .event:
            self = .event(try container.decode(WorkEventReference.self, forKey: .event))
        case .child:
            self = .child(try container.decode(WorkReference.self, forKey: .work))
        case .capability:
            self = .capability(
                try container.decode(WorkCapabilityReference.self, forKey: .capability)
            )
        case .effect:
            self = .effect(try container.decode(WorkEffectReference.self, forKey: .effect))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .input(let after):
            try container.encode(Kind.input, forKey: .kind)
            try container.encode(after, forKey: .after)
        case .time(let notBefore):
            try container.encode(Kind.time, forKey: .kind)
            try container.encode(notBefore, forKey: .notBefore)
        case .event(let event):
            try container.encode(Kind.event, forKey: .kind)
            try container.encode(event, forKey: .event)
        case .child(let work):
            try container.encode(Kind.child, forKey: .kind)
            try container.encode(work, forKey: .work)
        case .capability(let capability):
            try container.encode(Kind.capability, forKey: .kind)
            try container.encode(capability, forKey: .capability)
        case .effect(let effect):
            try container.encode(Kind.effect, forKey: .kind)
            try container.encode(effect, forKey: .effect)
        }
    }
}

public struct WorkWait: Codable, Sendable, Hashable {
    public let id: String
    public let work: WorkReference
    public let epochID: String
    public let on: WorkWaitOn
    public let createdAt: String
    public let resolvedAt: String?

    private enum CodingKeys: String, CodingKey {
        case id
        case work
        case epochID = "epoch_id"
        case on
        case createdAt = "created_at"
        case resolvedAt = "resolved_at"
    }
}
