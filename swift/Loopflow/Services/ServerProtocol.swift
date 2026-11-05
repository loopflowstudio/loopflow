import Foundation

/// Revision 2 of the server protocol: one connection per client, a negotiated
/// revision, requests multiplexed by id. Mirrors `rust/loopflow/src/server/wire.rs`;
/// carriers add no Work authority.
public enum ServerProtocol {
    public static let revision = 2
    public static let minRevision = 2
    /// The largest control-connection message; larger ones travel by ticket.
    public static let maxMessage = 1024 * 1024
    /// The largest message a ticketed connection carries.
    public static let maxBulk = 64 * 1024 * 1024
    /// The remote control connection; a ticketed connection opens the ticket path.
    public static let machinePath = "/v1/machine"
    public static let ticketPath = "/v1/ticket"
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
    /// A ticketed connection's first message; the server answers `admitted`.
    case ticket(String)

    private enum Keys: String, CodingKey { case type, id, operation, checkpoint, revision, client, ticket, minRevision = "min_revision" }
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
        case .ticket(let ticket):
            try box.encode("ticket", forKey: .type)
            try box.encode(ticket, forKey: .ticket)
        }
    }
}

/// Mobile access as `lf server mobile --status` reports it; Desktop reads it
/// through that command, so no client sends `mobile.*` operations.
public struct MachineMobileConfiguration: Codable, Sendable {
    public struct Owner: Codable, Sendable {
        public let node: String
        public let user: UInt64
    }
    public let enabled: Bool
    public let endpoint: String
    public let owner: Owner
}

/// What `lf server mobile --status` prints.
public struct MachineMobileStatus: Codable, Sendable {
    public let configuration: MachineMobileConfiguration?
    public let ready: Bool
    public let error: String?
}

/// What a request asks for, named by its `kind`: a dotted resource and verb.
/// A read or write encodes its own `kind` and fields.
public enum ServerOperation: Encodable, Sendable {
    case sessionOwner(session: String)
    case read(ServedRead)
    case command(ServedWrite)
    case configuration(JSONValue)
    /// `{ticket}` for a terminal connection to this owner's existing protocol.
    case attach(owner: String)
    /// `{ticket}` for a connection carrying one request of `bytes` bytes.
    case upload(bytes: Int)

    private enum Keys: String, CodingKey { case kind, owner, session, request, bytes }
    public func encode(to encoder: Encoder) throws {
        switch self {
        case .read(let query): try query.encode(to: encoder)
        case .command(let change): try change.encode(to: encoder)
        case .upload(let bytes):
            var box = encoder.container(keyedBy: Keys.self)
            try box.encode("request.upload", forKey: .kind)
            try box.encode(bytes, forKey: .bytes)
        case .sessionOwner(let session):
            var box = encoder.container(keyedBy: Keys.self)
            try box.encode("session.owner", forKey: .kind)
            try box.encode(session, forKey: .session)
        case .configuration(let request):
            var box = encoder.container(keyedBy: Keys.self)
            try box.encode("config.file", forKey: .kind)
            try box.encode(request, forKey: .request)
        case .attach(let owner):
            var box = encoder.container(keyedBy: Keys.self)
            try box.encode("terminal.attach", forKey: .kind)
            try box.encode(owner, forKey: .owner)
        }
    }
}

/// One message from the server.
public enum ServerMessage: Decodable, Sendable {
    case welcome(ServerWelcome)
    case status(ServerStatus)
    /// What the request returned: a read's or write's value, `{owner}` for
    /// `session.owner`, null once a subscription or attachment is confirmed.
    case response(id: String, traceID: String, result: JSONValue)
    case reading(id: String, frame: JSONValue)
    /// The answer for `id` is `bytes` long; a ticketed connection delivers it.
    case bulk(id: String, ticket: String, bytes: Int)
    /// A ticketed connection's answer; what follows is what the ticket names.
    case admitted
    /// `id` is nil before a request exists. An error for a subscription ends it.
    /// `code` is the Rust `ErrorCode`'s wire name: incompatible, invalid,
    /// not_found, conflict, unavailable, outcome_unknown or busy. `data` is
    /// structured detail, `.null` when there is none.
    case error(id: String?, code: String, message: String, data: JSONValue)

    private enum Keys: String, CodingKey { case type, id, result, frame, code, message, data, ticket, bytes, traceID = "trace_id" }
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
        case "bulk":
            self = .bulk(id: try box.decode(String.self, forKey: .id), ticket: try box.decode(String.self, forKey: .ticket),
                         bytes: try box.decode(Int.self, forKey: .bytes))
        case "admitted": self = .admitted
        case "error":
            self = .error(id: try box.decodeIfPresent(String.self, forKey: .id), code: try box.decode(String.self, forKey: .code),
                          message: try box.decode(String.self, forKey: .message), data: try box.decode(JSONValue.self, forKey: .data))
        default: throw DecodingError.dataCorruptedError(forKey: .type, in: box, debugDescription: "Unsupported server message; update the client")
        }
    }
}

extension JSONValue {
    /// One field of an object result; `.null` for anything else.
    public subscript(field: String) -> JSONValue {
        if case .object(let fields) = self { return fields[field] ?? .null }
        return .null
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
