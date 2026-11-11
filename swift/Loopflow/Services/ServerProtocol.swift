import Foundation

/// Revision 3 of the server protocol: one connection per client, a negotiated
/// revision, requests multiplexed by id. Mirrors `rust/loopflow/src/server/wire.rs`;
/// carriers add no Work authority.
public enum ServerProtocol {
    public static let revision = 4
    public static let minRevision = 4
    public static let maxMessage = 32 * 1024 * 1024
}

/// Who connected, for diagnostics; it grants nothing.
public struct ServerClientIdentity: Codable, Equatable, Sendable {
    public let kind: String
    public let platform: String
    public let name: String
    public let version: String

    public init(kind: String, platform: String, name: String, version: String) {
        self.kind = kind
        self.platform = platform
        self.name = name
        self.version = version
    }
}

public struct ServerHello: Encodable, Sendable {
    public let revision: Int
    public let minRevision: Int
    public let client: ServerClientIdentity

    public init(revision: Int, minRevision: Int, client: ServerClientIdentity) {
        self.revision = revision
        self.minRevision = minRevision
        self.client = client
    }

    private enum CodingKeys: String, CodingKey { case revision, client, minRevision = "min_revision" }
}

/// What a client branches on: the Rust `ErrorCode`'s wire names.
public enum ServerErrorCode: String, Sendable, Equatable {
    case incompatible, invalid, conflict, unavailable, busy
    case notFound = "not_found"
    case outcomeUnknown = "outcome_unknown"
}

/// A server `error`, typed so callers branch on `code` rather than its text.
/// `code` is nil for a code this client does not know yet.
public struct ServerError: LocalizedError, Sendable, Equatable {
    public let code: ServerErrorCode?
    public let message: String
    public let data: JSONValue

    public init(code: String, message: String, data: JSONValue) {
        self.code = ServerErrorCode(rawValue: code)
        self.message = message
        self.data = data
    }

    /// Only `outcome_unknown` means the effect may have happened: read state
    /// before retrying. Every other code means nothing changed.
    public var mayHaveHappened: Bool { code == .outcomeUnknown }

    public var errorDescription: String? {
        mayHaveHappened ? "\(message). It may have happened; refresh before retrying." : message
    }
}

/// One message from the client.
public enum ServerClientMessage: Encodable, Sendable {
    case hello(ServerHello)
    /// A pre-hello readiness probe; the server answers `status` and closes.
    case status
    /// `select` keeps only those dotted paths of a read's answer; nil keeps it whole.
    case request(id: String, operation: ServerOperation, select: [String]?)
    /// A read with a subscription: a null `response`, then readings until cancelled.
    /// `select` is as for a request; a change outside it sends no reading.
    case subscribe(id: String, operation: ServedRead, select: [String]?, resume: String?)
    /// Ask the live subscription `id` for a reading now; the next reading's
    /// `answers` carries `checkpoint`, changed or not.
    case checkpoint(id: String, checkpoint: UInt64)
    /// Forget a request's answer, or end a subscription.
    case cancel(id: String)
    /// A bridge's first and only message: the ticket `session.attach` issued.
    /// The server answers `bridged`, and the connection then speaks the owner protocol.
    case attach(ticket: String)

    private enum Keys: String, CodingKey { case type, id, operation, select, resume, checkpoint, ticket, revision, client, minRevision = "min_revision" }
    public func encode(to encoder: Encoder) throws {
        var box = encoder.container(keyedBy: Keys.self)
        switch self {
        case .hello(let hello):
            try box.encode("hello", forKey: .type)
            try box.encode(hello.revision, forKey: .revision)
            try box.encode(hello.minRevision, forKey: .minRevision)
            try box.encode(hello.client, forKey: .client)
        case .status: try box.encode("status", forKey: .type)
        case .request(let id, let operation, let select):
            try box.encode("request", forKey: .type)
            try box.encode(id, forKey: .id)
            try box.encode(operation, forKey: .operation)
            try box.encode(select, forKey: .select)
        case .subscribe(let id, let operation, let select, let resume):
            try box.encode("subscribe", forKey: .type)
            try box.encode(id, forKey: .id)
            try box.encode(operation, forKey: .operation)
            try box.encode(select, forKey: .select)
            try box.encode(resume, forKey: .resume)
        case .checkpoint(let id, let checkpoint):
            try box.encode("checkpoint", forKey: .type)
            try box.encode(id, forKey: .id)
            try box.encode(checkpoint, forKey: .checkpoint)
        case .cancel(let id):
            try box.encode("cancel", forKey: .type)
            try box.encode(id, forKey: .id)
        case .attach(let ticket):
            try box.encode("attach", forKey: .type)
            try box.encode(ticket, forKey: .ticket)
        }
    }
}

/// What a request asks for, named by its `kind`: a dotted resource and verb.
/// A read or write encodes its own `kind` and fields.
public enum ServerOperation: Encodable, Sendable {
    case read(ServedRead)
    case command(ServedWrite)
    case config(ConfigOperation)
    /// The unchanged payload of a submitted creation; never arbitrary command replay.
    case submittedCreation(JSONValue)
    /// A ticket for the Session's terminal, answered `{ticket, owner_lf_process_id,
    /// expires_in_ms}`; with `owner`, only while that exact LfProcess owns it.
    case sessionAttach(session: String, owner: String?)
    /// A provider hook's turn or tool boundary, answered `{steers, update, waiting}`.
    /// Desktop never sends it; `lf __context-block` does.
    case sessionBoundary(moment: String, delivery: String, adapter: JSONValue, callback: JSONValue, gap: JSONValue?,
                         printed: [String], inbox: JSONValue?)

    private enum Keys: String, CodingKey {
        case kind, session, ownerLfProcessID = "owner_lf_process_id"
        case moment, delivery, adapter, callback, gap, printed, inbox
    }
    public func encode(to encoder: Encoder) throws {
        switch self {
        case .read(let query): try query.encode(to: encoder)
        case .command(let change): try change.encode(to: encoder)
        case .submittedCreation(let payload): try payload.encode(to: encoder)
        case .config(let operation): try operation.encode(to: encoder)
        case .sessionAttach(let session, let owner):
            var box = encoder.container(keyedBy: Keys.self)
            try box.encode("session.attach", forKey: .kind)
            try box.encode(session, forKey: .session)
            try box.encode(owner, forKey: .ownerLfProcessID)
        case .sessionBoundary(let moment, let delivery, let adapter, let callback, let gap, let printed, let inbox):
            var box = encoder.container(keyedBy: Keys.self)
            try box.encode("session.boundary", forKey: .kind)
            try box.encode(moment, forKey: .moment)
            try box.encode(delivery, forKey: .delivery)
            try box.encode(adapter, forKey: .adapter)
            try box.encode(callback, forKey: .callback)
            try box.encode(gap, forKey: .gap)
            try box.encode(printed, forKey: .printed)
            try box.encode(inbox, forKey: .inbox)
        }
    }
}

/// The `config.*` operations Desktop's Settings and Appearance panes use.
/// A destination is the Rust `config::files::Destination` as JSON: an owner,
/// never a file plus a scope. Mirrors `Read::Config*` and `Write::Config*`.
public enum ConfigOperation: Encodable, Sendable {
    case configCatalog
    case configGet(destination: JSONValue, key: String)
    case configExplain(destination: JSONValue, key: String)
    /// `value` is YAML.
    case configSet(destination: JSONValue, key: String, value: String)
    case configUnset(destination: JSONValue, key: String)
    /// Appearance's theme file: set to `value`, or read when nil.
    case configTheme(path: String, value: String?)

    public func encode(to encoder: Encoder) throws {
        try tagged.encode(to: encoder, tag: "kind")
    }

    private var tagged: TaggedFields {
        switch self {
        case .configCatalog: TaggedFields("config.catalog")
        case .configGet(let destination, let key):
            TaggedFields("config.get", ["destination": destination, "key": key])
        case .configExplain(let destination, let key):
            TaggedFields("config.explain", ["destination": destination, "key": key])
        case .configSet(let destination, let key, let value):
            TaggedFields("config.set", ["destination": destination, "key": key, "value": value])
        case .configUnset(let destination, let key):
            TaggedFields("config.unset", ["destination": destination, "key": key])
        case .configTheme(let path, let value):
            TaggedFields("config.theme", ["path": path, "value": value])
        }
    }
}

/// An occurrence beside a complete reading. Unknown kinds stay subscription-local.
public enum SubscriptionEvent: Decodable, Sendable {
    case taskShipped(taskID: String, prID: String, number: UInt32, url: String, mergeCommit: String)
    case taskSteer(taskID: String, steer: Steer, comment: TaskComment?)
    case graphStepFinished(taskID: String, graphProcessID: String, step: GraphStepProcess)
    case unknown(kind: String)

    private enum Keys: String, CodingKey {
        case kind, number, url, steer, step, comment
        case taskID = "task_id", prID = "pr_id", mergeCommit = "merge_commit", graphProcessID = "graph_process_id"
    }

    public init(from decoder: Decoder) throws {
        let box = try decoder.container(keyedBy: Keys.self)
        let kind = try box.decode(String.self, forKey: .kind)
        switch kind {
        case "task.shipped":
            self = .taskShipped(taskID: try box.decode(String.self, forKey: .taskID),
                                prID: try box.decode(String.self, forKey: .prID),
                                number: try box.decode(UInt32.self, forKey: .number),
                                url: try box.decode(String.self, forKey: .url),
                                mergeCommit: try box.decode(String.self, forKey: .mergeCommit))
        case "task.steer":
            self = .taskSteer(taskID: try box.decode(String.self, forKey: .taskID),
                              steer: try box.decode(Steer.self, forKey: .steer),
                              comment: try box.decode(TaskComment?.self, forKey: .comment))
        case "graph.step_finished":
            self = .graphStepFinished(taskID: try box.decode(String.self, forKey: .taskID),
                                      graphProcessID: try box.decode(String.self, forKey: .graphProcessID),
                                      step: try box.decode(GraphStepProcess.self, forKey: .step))
        default: self = .unknown(kind: kind)
        }
    }
}

public struct Steer: Decodable, Sendable {
    public let id: String
    public let sequence: Int64
    public let source: String
    public let revision: String
    public let author: WorkAuthor
    public let text: String
}

/// One message from the server.
public enum ServerMessage: Decodable, Sendable {
    case welcome(ServerWelcome)
    case status(ServerStatus)
    /// What the request returned: a read's or write's value, or the ticket for
    /// `session.attach`; null once a subscription is confirmed.
    case response(id: String, traceID: String, result: JSONValue)
    /// The answer to `attach`: the connection now speaks the owner protocol with this LfProcess.
    case bridged(ownerLfProcessID: String)
    case reading(id: String, checkpoint: String, frame: JSONValue)
    case event(id: String, seq: UInt64, event: SubscriptionEvent)
    /// The subscription's last reading was its answer (`task.wait` reached
    /// its state); nothing follows for `id`.
    case ended(id: String)
    /// `id` is nil before a request exists. An error for a subscription ends it.
    /// `code` is the Rust `ErrorCode`'s wire name: incompatible, invalid,
    /// not_found, conflict, unavailable, outcome_unknown or busy. `data` is
    /// structured detail, `.null` when there is none.
    case error(id: String?, code: String, message: String, data: JSONValue)

    private enum Keys: String, CodingKey {
        case type, id, result, frame, checkpoint, code, message, data, seq, event, traceID = "trace_id", ownerLfProcessID = "owner_lf_process_id"
    }
    public init(from decoder: Decoder) throws {
        let box = try decoder.container(keyedBy: Keys.self)
        switch try box.decode(String.self, forKey: .type) {
        case "welcome": self = .welcome(try ServerWelcome(from: decoder))
        case "status": self = .status(try ServerStatus(from: decoder))
        case "response":
            self = .response(id: try box.decode(String.self, forKey: .id), traceID: try box.decode(String.self, forKey: .traceID),
                             result: try box.decode(JSONValue.self, forKey: .result))
        case "bridged": self = .bridged(ownerLfProcessID: try box.decode(String.self, forKey: .ownerLfProcessID))
        case "reading":
            self = .reading(id: try box.decode(String.self, forKey: .id), checkpoint: try box.decode(String.self, forKey: .checkpoint), frame: try box.decode(JSONValue.self, forKey: .frame))
        case "event":
            self = .event(id: try box.decode(String.self, forKey: .id), seq: try box.decode(UInt64.self, forKey: .seq),
                          event: try box.decode(SubscriptionEvent.self, forKey: .event))
        case "ended": self = .ended(id: try box.decode(String.self, forKey: .id))
        case "error":
            self = .error(id: try box.decodeIfPresent(String.self, forKey: .id), code: try box.decode(String.self, forKey: .code),
                          message: try box.decode(String.self, forKey: .message), data: try box.decode(JSONValue.self, forKey: .data))
        default: throw DecodingError.dataCorruptedError(forKey: .type, in: box, debugDescription: "Unsupported server message; update the client")
        }
    }
}

public struct ServerWelcome: Decodable, Sendable {
    /// The server's name for this connection, as `lf server status` lists it.
    public let clientID: String
    /// The revision both sides speak for this connection.
    public let revision: Int
    public let serverRevision: Int
    public let minRevision: Int
    /// Changes with every server start.
    public let instance: String
    public let build: String
    public let executableSHA256: String
    /// `.null` while the Machine registry is unavailable or uninitialized.
    public let machine: JSONValue
    public let limits: ServerLimits

    private enum CodingKeys: String, CodingKey {
        case revision, instance, build, machine, limits
        case clientID = "client_id", serverRevision = "server_revision", minRevision = "min_revision"
        case executableSHA256 = "executable_sha256"
    }
}

public struct ServerLimits: Decodable, Sendable {
    public let maxMessage: Int
    public let maxConnections: Int
    public let maxOpen: Int
    public let maxRetainedBytes: Int

    private enum CodingKeys: String, CodingKey { case maxMessage = "max_message", maxConnections = "max_connections", maxOpen = "max_open", maxRetainedBytes = "max_retained_bytes" }
}

public struct ServerStatus: Decodable, Sendable {
    public let revision: Int
    public let minRevision: Int
    public let instance: String
    public let executableSHA256: String
    /// Every welcomed connection still open.
    public let clients: [ServerConnectedClient]

    private enum CodingKeys: String, CodingKey {
        case revision, instance, clients, minRevision = "min_revision", executableSHA256 = "executable_sha256"
    }
}

/// One welcomed connection, from `welcome` until it closes.
public struct ServerConnectedClient: Decodable, Equatable, Sendable {
    public let id: String
    public let client: ServerClientIdentity
    /// RFC 3339.
    public let connectedAt: String
    /// Requests the connection has sent, subscriptions included.
    public let requests: UInt64
    public let open: Int
    public let retainedBytes: Int

    private enum CodingKeys: String, CodingKey { case id, client, requests, open, retainedBytes = "retained_bytes", connectedAt = "connected_at" }
}
