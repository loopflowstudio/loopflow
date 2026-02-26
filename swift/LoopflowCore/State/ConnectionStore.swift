import Foundation

@MainActor
@Observable
public final class ConnectionStore {
    private enum PersistedSettings: Codable {
        case bundled(ServerConnection?)
        case remote(ServerConnection)

        private enum CodingKeys: String, CodingKey {
            case mode
            case remoteConnection
        }

        private enum ModeValue: String, Codable {
            case bundled
            case remote
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            let mode = try container.decode(ModeValue.self, forKey: .mode)
            switch mode {
            case .bundled:
                let remote = try container.decodeIfPresent(ServerConnection.self, forKey: .remoteConnection)
                self = .bundled(remote)
            case .remote:
                guard let connection = try container.decodeIfPresent(
                    ServerConnection.self,
                    forKey: .remoteConnection
                ) else {
                    throw DecodingError.dataCorruptedError(
                        forKey: .remoteConnection,
                        in: container,
                        debugDescription: "Remote mode requires a remote connection."
                    )
                }
                self = .remote(connection)
            }
        }

        func encode(to encoder: Encoder) throws {
            var container = encoder.container(keyedBy: CodingKeys.self)
            switch self {
            case .bundled(let remote):
                try container.encode(ModeValue.bundled, forKey: .mode)
                try container.encodeIfPresent(remote, forKey: .remoteConnection)
            case .remote(let connection):
                try container.encode(ModeValue.remote, forKey: .mode)
                try container.encode(connection, forKey: .remoteConnection)
            }
        }
    }

    private struct LegacyPersistedSettings: Codable {
        var mode: ConnectionMode
        var remoteConnection: ServerConnection?
    }

    private struct InitialState {
        var mode: ConnectionMode
        var activeConnection: ServerConnection
        var remoteConnection: ServerConnection?
        var shouldPersist: Bool
    }

    private let defaults: UserDefaults
    private static let defaultsKey = "concerto.connectionSettings.v2"
    private static let legacyDefaultsKey = "concerto.serverConnection.v1"
    private let secretStore: ConnectionSecretStore
    private let pinStore: CertificatePinStore

    public var mode: ConnectionMode
    public var activeConnection: ServerConnection
    private var remoteConnection: ServerConnection?

    public convenience init(
        secretStore: ConnectionSecretStore = .shared,
        pinStore: CertificatePinStore = .shared,
        defaults: UserDefaults = .standard
    ) {
        self.init(
            secretStore: secretStore,
            pinStore: pinStore,
            defaults: defaults,
            configLoader: { loadConcertoConfig() }
        )
    }

    init(
        secretStore: ConnectionSecretStore = .shared,
        pinStore: CertificatePinStore = .shared,
        defaults: UserDefaults = .standard,
        configLoader: () -> ConcertoConfig?
    ) {
        self.secretStore = secretStore
        self.pinStore = pinStore
        self.defaults = defaults

        let initial = Self.loadInitialState(
            defaults: defaults,
            secretStore: secretStore,
            configLoader: configLoader
        )
        mode = initial.mode
        activeConnection = initial.activeConnection
        remoteConnection = initial.remoteConnection

        if initial.shouldPersist {
            persistSettings()
        }
    }

    public func setMode(_ mode: ConnectionMode) {
        self.mode = mode
        activeConnection = (mode == .remote ? remoteConnection : nil) ?? .local
        persistSettings()
    }

    public func setRemoteConnection(_ connection: ServerConnection) {
        var next = connection
        if next.authMode.requiresToken {
            if let token = next.staticToken {
                _ = secretStore.saveToken(token, for: next)
            } else {
                next.staticToken = secretStore.token(for: next)
            }
        } else {
            _ = secretStore.deleteToken(for: next)
            next.staticToken = nil
        }

        mode = .remote
        remoteConnection = next
        activeConnection = next
        persistSettings()
    }

    public func setBundledRuntimeConnection(_ connection: ServerConnection) {
        mode = .bundled
        activeConnection = connection
        persistSettings()
    }

    public var configuredRemoteConnection: ServerConnection? {
        remoteConnection
    }

    public func token(for connection: ServerConnection) -> String? {
        connection.staticToken ?? secretStore.token(for: connection)
    }

    public func clearPinnedCertificate(for connection: ServerConnection? = nil) {
        pinStore.clearPinnedFingerprint(for: connection ?? activeConnection)
    }

    public func trustNewCertificate(_ requirement: TrustRequirement) {
        guard case let .certificateChanged(host, port, _, newFingerprint) = requirement else {
            return
        }

        let connection = ServerConnection(
            host: host,
            port: port,
            useTLS: true,
            authMode: .none
        )
        pinStore.setPinnedFingerprint(newFingerprint, for: connection)
    }

    private func persistSettings() {
        let value: PersistedSettings
        if mode == .remote, var remoteConnection {
            remoteConnection.staticToken = nil
            value = .remote(remoteConnection)
        } else {
            var persistedRemote = remoteConnection
            persistedRemote?.staticToken = nil
            value = .bundled(persistedRemote)
        }

        guard let data = try? JSONEncoder().encode(value) else { return }
        defaults.set(data, forKey: Self.defaultsKey)
    }

    private static func loadSettings(from defaults: UserDefaults) -> PersistedSettings? {
        guard let data = defaults.data(forKey: Self.defaultsKey) else { return nil }
        return try? JSONDecoder().decode(PersistedSettings.self, from: data)
    }

    private static func loadLegacyConnection(from defaults: UserDefaults) -> ServerConnection? {
        guard let data = defaults.data(forKey: Self.legacyDefaultsKey) else { return nil }
        return try? JSONDecoder().decode(ServerConnection.self, from: data)
    }

    private static func loadLegacySettings(from defaults: UserDefaults) -> LegacyPersistedSettings? {
        guard let data = defaults.data(forKey: Self.defaultsKey) else { return nil }
        return try? JSONDecoder().decode(LegacyPersistedSettings.self, from: data)
    }

    private static func bundledState(
        remoteConnection: ServerConnection?,
        secretStore: ConnectionSecretStore,
        shouldPersist: Bool
    ) -> InitialState {
        var remote = remoteConnection
        if let persisted = remote {
            remote?.staticToken = secretStore.token(for: persisted)
        }

        return InitialState(
            mode: .bundled,
            activeConnection: .local,
            remoteConnection: remote,
            shouldPersist: shouldPersist
        )
    }

    private static func remoteState(
        from connection: ServerConnection,
        secretStore: ConnectionSecretStore,
        shouldPersist: Bool
    ) -> InitialState {
        var remote = connection
        remote.staticToken = secretStore.token(for: connection)
        return InitialState(
            mode: .remote,
            activeConnection: remote,
            remoteConnection: remote,
            shouldPersist: shouldPersist
        )
    }

    private static func loadInitialState(
        defaults: UserDefaults,
        secretStore: ConnectionSecretStore,
        configLoader: () -> ConcertoConfig?
    ) -> InitialState {
        if let loaded = loadSettings(from: defaults) {
            switch loaded {
            case .bundled(let remote):
                return bundledState(
                    remoteConnection: remote,
                    secretStore: secretStore,
                    shouldPersist: false
                )
            case .remote(let remote):
                return remoteState(
                    from: remote,
                    secretStore: secretStore,
                    shouldPersist: false
                )
            }
        }

        if let legacySettings = loadLegacySettings(from: defaults) {
            if legacySettings.mode == .remote, let remote = legacySettings.remoteConnection {
                return remoteState(
                    from: remote,
                    secretStore: secretStore,
                    shouldPersist: true
                )
            }
            return bundledState(
                remoteConnection: nil,
                secretStore: secretStore,
                shouldPersist: true
            )
        }

        if let legacy = loadLegacyConnection(from: defaults) {
            if legacy.isLocal {
                return bundledState(
                    remoteConnection: nil,
                    secretStore: secretStore,
                    shouldPersist: true
                )
            }
            return remoteState(
                from: legacy,
                secretStore: secretStore,
                shouldPersist: true
            )
        }

        if let connection = configLoader()?.connection, !connection.isLocalhost {
            return remoteState(
                from: connection.toServerConnection(),
                secretStore: secretStore,
                shouldPersist: true
            )
        }

        return bundledState(
            remoteConnection: nil,
            secretStore: secretStore,
            shouldPersist: false
        )
    }
}

private extension RemoteConnectionConfig {
    var isLocalhost: Bool {
        let normalized = host.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return normalized == "localhost" || normalized == "127.0.0.1" || normalized == "::1"
    }
}
