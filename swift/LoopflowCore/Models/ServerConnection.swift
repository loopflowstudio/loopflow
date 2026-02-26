import Foundation

public enum ConnectionAuthMode: String, CaseIterable, Codable, Sendable {
    case none
    case staticToken

    public var requiresToken: Bool {
        self != .none
    }
}

public struct ServerConnection: Hashable, Sendable {
    public var host: String
    public var port: Int
    public var useTLS: Bool
    public var authMode: ConnectionAuthMode
    public var staticToken: String?

    public init(
        host: String,
        port: Int,
        useTLS: Bool,
        authMode: ConnectionAuthMode,
        staticToken: String? = nil
    ) {
        self.host = host
        self.port = port
        self.useTLS = useTLS
        self.authMode = authMode
        self.staticToken = staticToken
    }

    public var connectionKey: String {
        "\(host):\(port)"
    }

    public var httpBaseURL: URL {
        makeURL(scheme: useTLS ? "https" : "http")
    }

    public var wsBaseURL: URL {
        makeURL(scheme: useTLS ? "wss" : "ws", path: "/ws")
    }

    private func makeURL(scheme: String, path: String = "") -> URL {
        var components = URLComponents()
        components.scheme = scheme
        components.host = host
        components.port = port
        components.path = path
        return components.url ?? URL(string: "\(scheme)://127.0.0.1:2486\(path)")!
    }

    /// Used only for timeout tier selection.
    /// Auth and TLS are always controlled by explicit fields.
    public var isLocal: Bool {
        host == "127.0.0.1" && !useTLS && authMode == .none
    }

    public var displayName: String {
        let scheme = useTLS ? "https" : "http"
        return "\(scheme)://\(host):\(port)"
    }

    public static let local = ServerConnection(
        host: "127.0.0.1",
        port: 2486,
        useTLS: false,
        authMode: .none
    )
}

extension ServerConnection: Codable {
    enum CodingKeys: String, CodingKey {
        case host
        case port
        case useTLS
        case authMode
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        host = try container.decode(String.self, forKey: .host)
        port = try container.decode(Int.self, forKey: .port)
        useTLS = try container.decode(Bool.self, forKey: .useTLS)
        authMode = try container.decode(ConnectionAuthMode.self, forKey: .authMode)
        staticToken = nil
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(host, forKey: .host)
        try container.encode(port, forKey: .port)
        try container.encode(useTLS, forKey: .useTLS)
        try container.encode(authMode, forKey: .authMode)
    }
}

public enum RepoTarget: Hashable, Sendable {
    case local(URL)
    case remote(path: String, host: String)

    public var path: String {
        switch self {
        case .local(let url):
            return url.path
        case .remote(let path, _):
            return path
        }
    }

    public var displayName: String {
        switch self {
        case .local(let url):
            return url.lastPathComponent
        case .remote(let path, _):
            let name = URL(fileURLWithPath: path).lastPathComponent
            return name.isEmpty ? path : name
        }
    }

    public var isRemote: Bool {
        switch self {
        case .local:
            false
        case .remote:
            true
        }
    }

    public var remoteHost: String? {
        switch self {
        case .local:
            nil
        case .remote(_, let host):
            host
        }
    }

    public var localURL: URL? {
        switch self {
        case .local(let url):
            url
        case .remote:
            nil
        }
    }
}

public struct RemoteRepo: Hashable, Codable, Sendable, Identifiable {
    public let path: String
    public let name: String
    public let waveCount: Int
    public let registered: Bool
    public let addedAt: Date?

    public var id: String { path }

    public init(
        path: String,
        name: String,
        waveCount: Int,
        registered: Bool = false,
        addedAt: Date? = nil
    ) {
        self.path = path
        self.name = name
        self.waveCount = waveCount
        self.registered = registered
        self.addedAt = addedAt
    }
}

public enum ConnectionState: Equatable, Sendable {
    case disconnected(DisconnectReason?)
    case connecting(ConnectionPhase)
    case connected
    case reconnecting(attempt: Int, nextDelay: TimeInterval, lastError: DisconnectReason?)
    case authFailed(String?)
    case trustRequired(TrustRequirement)
}

public enum ConnectionPhase: String, Sendable {
    case tlsTrustCheck
    case authCheck
    case repoDiscovery
    case wsProbe
}

public enum DisconnectReason: Equatable, Sendable {
    case networkUnavailable
    case timeout
    case daemonTimeout
    case serverUnreachable
    case serverError(status: Int)
    case wsClosed(code: Int?)
    case authRejected
    case trustMismatch
    case unknown(String)
}

public enum TrustRequirement: Equatable, Sendable {
    case certificateChanged(host: String, port: Int, oldFingerprint: String, newFingerprint: String)
}
