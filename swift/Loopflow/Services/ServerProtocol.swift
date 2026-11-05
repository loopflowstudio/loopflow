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
    /// The control connection; a ticketed connection opens the ticket path.
    public static let machinePath = "/v1/machine"
    public static let ticketPath = "/v1/ticket"
}

/// Who connected, for diagnostics; it grants nothing.
public struct ServerClientIdentity: Encodable, Sendable {
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
    /// One request line for the live subscription `id`.
    case checkpoint(id: String, frame: JSONValue)
    /// Forget a request's answer, or end a subscription.
    case cancel(id: String)
    /// A ticketed connection's first message; the server answers `admitted`.
    case ticket(String)

    private enum Keys: String, CodingKey { case type, id, operation, frame, revision, client, ticket, minRevision = "min_revision" }
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
        case .checkpoint(let id, let frame):
            try box.encode("checkpoint", forKey: .type)
            try box.encode(id, forKey: .id)
            try box.encode(frame, forKey: .frame)
        case .cancel(let id):
            try box.encode("cancel", forKey: .type)
            try box.encode(id, forKey: .id)
        case .ticket(let ticket):
            try box.encode("ticket", forKey: .type)
            try box.encode(ticket, forKey: .ticket)
        }
    }
}

public struct MachineMobileConfiguration: Codable, Sendable {
    public struct Owner: Codable, Sendable {
        public let node: String
        public let user: UInt64
    }
    public let enabled: Bool
    public let endpoint: String
    public let owner: Owner
}

public struct MachineMobileStatus: Codable, Sendable {
    public let configuration: MachineMobileConfiguration?
    public let ready: Bool
    public let error: String?
}

public enum ServerOperation: Encodable, Sendable {
    /// Same-user local socket only.
    case mobileStatus
    /// Same-user local socket only.
    case configureRemote(configuration: MachineMobileConfiguration?)
    case sessionOwner(session: String)
    case read(ServedRead)
    case subscribe(ServedRead, generation: UInt64)
    case command(ServedWrite)
    case configuration(JSONValue)
    /// A ticket for a terminal connection to this owner's existing protocol.
    case attach(owner: String)
    /// A ticket for a connection carrying one request of `bytes` bytes.
    case bulk(bytes: Int)

    private enum Keys: String, CodingKey { case kind, query, generation, change, owner, session, request, configuration, bytes }
    public func encode(to encoder: Encoder) throws {
        var box = encoder.container(keyedBy: Keys.self)
        switch self {
        case .mobileStatus: try box.encode("mobile_status", forKey: .kind)
        case .configureRemote(let configuration):
            try box.encode("configure_remote", forKey: .kind)
            try box.encode(configuration, forKey: .configuration)
        case .sessionOwner(let session):
            try box.encode("session_owner", forKey: .kind)
            try box.encode(session, forKey: .session)
        case .read(let query):
            try box.encode("read", forKey: .kind)
            try box.encode(query, forKey: .query)
        case .subscribe(let query, let generation):
            try box.encode("subscribe", forKey: .kind)
            try box.encode(query, forKey: .query)
            try box.encode(generation, forKey: .generation)
        case .configuration(let request):
            try box.encode("configuration", forKey: .kind)
            try box.encode(request, forKey: .request)
        case .command(let change):
            try box.encode("command", forKey: .kind)
            try box.encode(change, forKey: .change)
        case .attach(let owner):
            try box.encode("attach", forKey: .kind)
            try box.encode(owner, forKey: .owner)
        case .bulk(let bytes):
            try box.encode("bulk", forKey: .kind)
            try box.encode(bytes, forKey: .bytes)
        }
    }
}

/// One message from the server.
public enum ServerMessage: Decodable, Sendable {
    case welcome(ServerWelcome)
    case status(ServerStatus)
    case response(id: String, result: ServerReply)
    case reading(id: String, generation: UInt64, frame: JSONValue)
    /// The answer for `id` is `bytes` long; a ticketed connection delivers it.
    case bulk(id: String, ticket: String, bytes: Int)
    /// A ticketed connection's answer; what follows is what the ticket names.
    case admitted
    /// `id` is nil before a request exists. An error for a subscription ends it.
    case error(id: String?, code: String, message: String)

    private enum Keys: String, CodingKey { case type, id, result, generation, frame, code, message, ticket, bytes }
    public init(from decoder: Decoder) throws {
        let box = try decoder.container(keyedBy: Keys.self)
        switch try box.decode(String.self, forKey: .type) {
        case "welcome": self = .welcome(try ServerWelcome(from: decoder))
        case "status": self = .status(try ServerStatus(from: decoder))
        case "response":
            self = .response(id: try box.decode(String.self, forKey: .id), result: try box.decode(ServerReply.self, forKey: .result))
        case "reading":
            self = .reading(id: try box.decode(String.self, forKey: .id), generation: try box.decode(UInt64.self, forKey: .generation),
                            frame: try box.decode(JSONValue.self, forKey: .frame))
        case "bulk":
            self = .bulk(id: try box.decode(String.self, forKey: .id), ticket: try box.decode(String.self, forKey: .ticket),
                         bytes: try box.decode(Int.self, forKey: .bytes))
        case "admitted": self = .admitted
        case "error":
            self = .error(id: try box.decodeIfPresent(String.self, forKey: .id), code: try box.decode(String.self, forKey: .code),
                          message: try box.decode(String.self, forKey: .message))
        default: throw DecodingError.dataCorruptedError(forKey: .type, in: box, debugDescription: "Unsupported server message; update the client")
        }
    }
}

public struct ServerWelcome: Decodable, Sendable {
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
    public let capabilities: [String]
    public let limits: ServerLimits

    private enum CodingKeys: String, CodingKey {
        case revision, instance, build, machine, capabilities, limits
        case serverRevision = "server_revision", minRevision = "min_revision", executableSHA256 = "executable_sha256"
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

    private enum CodingKeys: String, CodingKey { case revision, instance, minRevision = "min_revision", executableSHA256 = "executable_sha256" }
}

public enum ServerReply: Decodable, Sendable {
    case mobileStatus(MachineMobileStatus)
    case remote(configuration: MachineMobileConfiguration?)
    case read(JSONValue)
    case subscribed
    /// `value` is stdout parsed as JSON when the command's spelling prints JSON.
    case command(traceID: String, code: Int, value: JSONValue?, output: String, stderr: String)
    case sessionOwner(owner: String?)
    /// Single use, bound to this connection, expiring unused after 30 seconds.
    case ticket(String)

    private enum Keys: String, CodingKey { case kind, value, output, stderr, code, owner, status, configuration, ticket, traceID = "trace_id" }
    public init(from decoder: Decoder) throws {
        let box = try decoder.container(keyedBy: Keys.self)
        switch try box.decode(String.self, forKey: .kind) {
        case "mobile_status": self = .mobileStatus(try box.decode(MachineMobileStatus.self, forKey: .status))
        case "remote": self = .remote(configuration: try box.decodeIfPresent(MachineMobileConfiguration.self, forKey: .configuration))
        case "read": self = .read(try box.decode(JSONValue.self, forKey: .value))
        case "subscribed": self = .subscribed
        case "command":
            self = .command(traceID: try box.decode(String.self, forKey: .traceID), code: try box.decode(Int.self, forKey: .code),
                            value: try box.decodeIfPresent(JSONValue.self, forKey: .value),
                            output: try box.decode(String.self, forKey: .output), stderr: try box.decode(String.self, forKey: .stderr))
        case "session_owner": self = .sessionOwner(owner: try box.decodeIfPresent(String.self, forKey: .owner))
        case "ticket": self = .ticket(try box.decode(String.self, forKey: .ticket))
        default: throw DecodingError.dataCorruptedError(forKey: .kind, in: box, debugDescription: "Unsupported server result; update the client")
        }
    }
}
