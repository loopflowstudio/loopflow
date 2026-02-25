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

    public init(
        secretStore: ConnectionSecretStore = .shared,
        pinStore: CertificatePinStore = .shared,
        defaults: UserDefaults = .standard
    ) {
        self.secretStore = secretStore
        self.pinStore = pinStore
        self.defaults = defaults

        let initial = Self.loadInitialState(defaults: defaults, secretStore: secretStore)
        mode = initial.mode
        activeConnection = initial.activeConnection
        remoteConnection = initial.remoteConnection

        if initial.shouldPersist {
            persistSettings()
        }
    }

    public func setMode(_ mode: ConnectionMode) {
        self.mode = mode
        if mode == .remote, let remoteConnection {
            activeConnection = remoteConnection
        } else {
            activeConnection = .local
        }
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

    private static func loadInitialState(
        defaults: UserDefaults,
        secretStore: ConnectionSecretStore
    ) -> InitialState {
        if let loaded = loadSettings(from: defaults) {
            switch loaded {
            case .bundled(var remote):
                if let persisted = remote {
                    remote?.staticToken = secretStore.token(for: persisted)
                }
                return InitialState(
                    mode: .bundled,
                    activeConnection: .local,
                    remoteConnection: remote,
                    shouldPersist: false
                )
            case .remote(var remote):
                remote.staticToken = secretStore.token(for: remote)
                return InitialState(
                    mode: .remote,
                    activeConnection: remote,
                    remoteConnection: remote,
                    shouldPersist: false
                )
            }
        }

        if let legacySettings = loadLegacySettings(from: defaults) {
            if legacySettings.mode == .remote, var remote = legacySettings.remoteConnection {
                remote.staticToken = secretStore.token(for: remote)
                return InitialState(
                    mode: .remote,
                    activeConnection: remote,
                    remoteConnection: remote,
                    shouldPersist: true
                )
            }

            return InitialState(
                mode: .bundled,
                activeConnection: .local,
                remoteConnection: nil,
                shouldPersist: true
            )
        }

        if let legacy = loadLegacyConnection(from: defaults) {
            guard !legacy.isLocal else {
                return InitialState(
                    mode: .bundled,
                    activeConnection: .local,
                    remoteConnection: nil,
                    shouldPersist: true
                )
            }

            var remote = legacy
            remote.staticToken = secretStore.token(for: legacy)
            return InitialState(
                mode: .remote,
                activeConnection: remote,
                remoteConnection: remote,
                shouldPersist: true
            )
        }

        return InitialState(
            mode: .bundled,
            activeConnection: .local,
            remoteConnection: nil,
            shouldPersist: false
        )
    }
}
