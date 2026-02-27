import Foundation

public struct DaemonRepo: Codable, Hashable, Sendable {
    public let name: String
    public let waveCount: Int

    public init(name: String, waveCount: Int) {
        self.name = name
        self.waveCount = waveCount
    }
}

public struct DiscoveredDaemon: Codable, Hashable, Identifiable, Sendable {
    public let machineId: String
    public let machineName: String?
    public let url: String?
    public let capabilities: [String]
    public let repos: [DaemonRepo]
    public let connectionToken: String
    public let lastHeartbeat: Date?

    public var id: String { machineId }
    public var displayName: String { machineName ?? machineId }
    public var daemonURL: URL? { Self.parseDaemonURL(url) }

    public init(
        machineId: String,
        machineName: String?,
        url: String?,
        capabilities: [String],
        repos: [DaemonRepo],
        connectionToken: String,
        lastHeartbeat: Date?
    ) {
        self.machineId = machineId
        self.machineName = machineName
        self.url = url
        self.capabilities = capabilities
        self.repos = repos
        self.connectionToken = connectionToken
        self.lastHeartbeat = lastHeartbeat
    }

    public func makeConnection() throws -> ServerConnection {
        guard let parsed = daemonURL,
              let host = parsed.host,
              !host.isEmpty else {
            throw DiscoveryServiceError.invalidDaemonURL(url)
        }

        let scheme = parsed.scheme?.lowercased()
        let useTLS = scheme == "https" || scheme == "wss"
        let port = parsed.port ?? (useTLS ? 443 : 2486)
        guard (1 ... 65_535).contains(port) else {
            throw DiscoveryServiceError.invalidDaemonURL(url)
        }

        let token = connectionToken.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !token.isEmpty else {
            throw DiscoveryServiceError.missingConnectionToken(machineId: machineId)
        }

        return ServerConnection(
            host: host,
            port: port,
            useTLS: useTLS,
            authMode: .staticToken,
            staticToken: token
        )
    }

    public var repoSummary: String {
        guard !repos.isEmpty else { return "No repos" }
        return repos
            .map { "\($0.name) (\($0.waveCount))" }
            .joined(separator: ", ")
    }

    private static func parseDaemonURL(_ rawURL: String?) -> URL? {
        let trimmedURL = rawURL?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard !trimmedURL.isEmpty else {
            return nil
        }

        let normalizedURL = trimmedURL.contains("://") ? trimmedURL : "http://\(trimmedURL)"
        return URL(string: normalizedURL)
    }
}
