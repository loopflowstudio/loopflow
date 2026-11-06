import Foundation

/// Revision 2 of the server protocol: one connection per client, a negotiated
/// revision, requests multiplexed by id. Mirrors `rust/loopflow/src/server/wire.rs`;
/// carriers add no Work authority.
public enum ServerProtocol {
    public static let revision = 2
    public static let minRevision = 2
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

/// One message from the client.
public enum ServerClientMessage: Encodable, Sendable {
    case hello(ServerHello)
    /// A pre-hello readiness probe; the server answers `status` and closes.
    case status
    case request(id: String, operation: ServerOperation)
    /// A read with a watch: `subscribed`, then readings until cancelled.
    case subscribe(id: String, operation: ServedRead)
    /// Ask the live subscription `id` for a reading now; the next reading's
    /// `answers` carries `checkpoint`, changed or not.
    case checkpoint(id: String, checkpoint: UInt64)
    /// Forget a request's answer, or end a subscription.
    case cancel(id: String)

    private enum Keys: String, CodingKey { case type, id, operation, checkpoint, revision, client, minRevision = "min_revision" }
    public func encode(to encoder: Encoder) throws {
        var box = encoder.container(keyedBy: Keys.self)
        switch self {
        case .hello(let hello):
            try box.encode("hello", forKey: .type)
            try box.encode(hello.revision, forKey: .revision)
            try box.encode(hello.minRevision, forKey: .minRevision)
            try box.encode(hello.client, forKey: .client)
        case .status: try box.encode("status", forKey: .type)
        case .request(let id, let operation):
            try box.encode("request", forKey: .type)
            try box.encode(id, forKey: .id)
            try box.encode(operation, forKey: .operation)
        case .subscribe(let id, let operation):
            try box.encode("subscribe", forKey: .type)
            try box.encode(id, forKey: .id)
            try box.encode(operation, forKey: .operation)
        case .checkpoint(let id, let checkpoint):
            try box.encode("checkpoint", forKey: .type)
            try box.encode(id, forKey: .id)
            try box.encode(checkpoint, forKey: .checkpoint)
        case .cancel(let id):
            try box.encode("cancel", forKey: .type)
            try box.encode(id, forKey: .id)
        }
    }
}

/// What a request asks for, named by its `kind`: a dotted resource and verb.
/// A read or write encodes its own `kind` and fields.
public enum ServerOperation: Encodable, Sendable {
    case read(ServedRead)
    case command(ServedWrite)
    case config(ConfigOperation)
    /// The Session's terminal; with `owner`, only while that exact Process owns it.
    case attach(session: String, owner: String?)

    private enum Keys: String, CodingKey { case kind, owner, session }
    public func encode(to encoder: Encoder) throws {
        switch self {
        case .read(let query): try query.encode(to: encoder)
        case .command(let change): try change.encode(to: encoder)
        case .config(let operation): try operation.encode(to: encoder)
        case .attach(let session, let owner):
            var box = encoder.container(keyedBy: Keys.self)
            try box.encode("session.attach", forKey: .kind)
            try box.encode(session, forKey: .session)
            try box.encode(owner, forKey: .owner)
        }
    }
}

/// The `config.*` operations Desktop's Settings and Appearance panes use.
/// A destination is the Rust `config::files::Destination` as JSON: an owner,
/// never a file plus a scope. Mirrors `Read::Config*` and `Write::Config*`.
public enum ConfigOperation: Encodable, Sendable {
    case catalog
    case get(destination: JSONValue, key: String)
    case explain(destination: JSONValue, key: String)
    /// `value` is YAML.
    case set(destination: JSONValue, key: String, value: String)
    case unset(destination: JSONValue, key: String)
    /// Appearance's theme file: set to `value`, or read when nil.
    case theme(path: String, value: String?)

    private enum Keys: String, CodingKey { case kind, destination, key, value, path }
    public func encode(to encoder: Encoder) throws {
        var box = encoder.container(keyedBy: Keys.self)
        switch self {
        case .catalog: try box.encode("config.catalog", forKey: .kind)
        case .get(let destination, let key), .explain(let destination, let key), .unset(let destination, let key):
            let kind = switch self {
            case .get: "config.get"
            case .explain: "config.explain"
            default: "config.unset"
            }
            try box.encode(kind, forKey: .kind)
            try box.encode(destination, forKey: .destination)
            try box.encode(key, forKey: .key)
        case .set(let destination, let key, let value):
            try box.encode("config.set", forKey: .kind)
            try box.encode(destination, forKey: .destination)
            try box.encode(key, forKey: .key)
            try box.encode(value, forKey: .value)
        case .theme(let path, let value):
            try box.encode("config.theme", forKey: .kind)
            try box.encode(path, forKey: .path)
            try box.encode(value, forKey: .value)
        }
    }
}

/// One message from the server.
public enum ServerMessage: Decodable, Sendable {
    case welcome(ServerWelcome)
    case status(ServerStatus)
    /// What the request returned: a read's or write's value, or `{owner}` for
    /// `session.attach`; null once a subscription is confirmed.
    case response(id: String, traceID: String, result: JSONValue)
    case reading(id: String, frame: JSONValue)
    /// The subscription's last reading was its answer (`task.wait` reached
    /// its state); nothing follows for `id`.
    case ended(id: String)
    /// `id` is nil before a request exists. An error for a subscription ends it.
    /// `code` is the Rust `ErrorCode`'s wire name: incompatible, invalid,
    /// not_found, conflict, unavailable, outcome_unknown or busy. `data` is
    /// structured detail, `.null` when there is none.
    case error(id: String?, code: String, message: String, data: JSONValue)

    private enum Keys: String, CodingKey { case type, id, result, frame, code, message, data, traceID = "trace_id" }
    public init(from decoder: Decoder) throws {
        let box = try decoder.container(keyedBy: Keys.self)
        switch try box.decode(String.self, forKey: .type) {
        case "welcome": self = .welcome(try ServerWelcome(from: decoder))
        case "status": self = .status(try ServerStatus(from: decoder))
        case "response":
            self = .response(id: try box.decode(String.self, forKey: .id), traceID: try box.decode(String.self, forKey: .traceID),
                             result: try box.decode(JSONValue.self, forKey: .result))
        case "reading":
            self = .reading(id: try box.decode(String.self, forKey: .id), frame: try box.decode(JSONValue.self, forKey: .frame))
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

    private enum CodingKeys: String, CodingKey { case maxMessage = "max_message", maxConnections = "max_connections" }
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

    private enum CodingKeys: String, CodingKey { case id, client, requests, connectedAt = "connected_at" }
}
