import Foundation
import LoopflowCore

@MainActor
@Observable
final class ConnectionStore {
    private struct PersistedSettings: Codable {
        var mode: ConnectionMode
        var remoteConnection: ServerConnection?
    }

    private let defaults: UserDefaults
    private static let defaultsKey = "concerto.connectionSettings.v2"
    private static let legacyDefaultsKey = "concerto.serverConnection.v1"
    private let secretStore: ConnectionSecretStore
    private let pinStore: CertificatePinStore

    var mode: ConnectionMode
    var activeConnection: ServerConnection
    private var remoteConnection: ServerConnection?

    init(
        secretStore: ConnectionSecretStore = .shared,
        pinStore: CertificatePinStore = .shared,
        defaults: UserDefaults = .standard
    ) {
        self.secretStore = secretStore
        self.pinStore = pinStore
        self.defaults = defaults

        let initialMode: ConnectionMode
        let initialActiveConnection: ServerConnection
        let initialRemoteConnection: ServerConnection?
        let shouldPersistMigratedLegacy: Bool

        if let loaded = Self.loadSettings(from: defaults) {
            initialMode = loaded.mode
            if var remote = loaded.remoteConnection {
                remote.staticToken = secretStore.token(for: remote)
                initialRemoteConnection = remote
            } else {
                initialRemoteConnection = nil
            }
            if initialMode == .remote, let remoteConnection = initialRemoteConnection {
                initialActiveConnection = remoteConnection
            } else {
                initialActiveConnection = .local
            }
            shouldPersistMigratedLegacy = false
        } else if let legacy = Self.loadLegacyConnection(from: defaults) {
            if legacy.isLocal {
                initialMode = .bundled
                initialRemoteConnection = nil
                initialActiveConnection = .local
            } else {
                var remote = legacy
                remote.staticToken = secretStore.token(for: legacy)
                initialMode = .remote
                initialRemoteConnection = remote
                initialActiveConnection = remote
            }
            shouldPersistMigratedLegacy = true
        } else {
            initialMode = .bundled
            initialActiveConnection = .local
            initialRemoteConnection = nil
            shouldPersistMigratedLegacy = false
        }

        mode = initialMode
        activeConnection = initialActiveConnection
        remoteConnection = initialRemoteConnection

        if shouldPersistMigratedLegacy {
            persistSettings()
        }
    }

    func setMode(_ mode: ConnectionMode) {
        self.mode = mode
        if mode == .remote, let remoteConnection {
            activeConnection = remoteConnection
        } else {
            activeConnection = .local
        }
        persistSettings()
    }

    func setRemoteConnection(_ connection: ServerConnection) {
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

    func setBundledRuntimeConnection(_ connection: ServerConnection) {
        mode = .bundled
        activeConnection = connection
        persistSettings()
    }

    func resetBundledRuntimeConnection() {
        guard mode == .bundled else { return }
        activeConnection = .local
    }

    var configuredRemoteConnection: ServerConnection? {
        remoteConnection
    }

    func token(for connection: ServerConnection) -> String? {
        connection.staticToken ?? secretStore.token(for: connection)
    }

    func clearPinnedCertificate(for connection: ServerConnection? = nil) {
        pinStore.clearPinnedFingerprint(for: connection ?? activeConnection)
    }

    func trustNewCertificate(_ requirement: TrustRequirement) {
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
        var persistedRemote = remoteConnection
        persistedRemote?.staticToken = nil

        let value = PersistedSettings(mode: mode, remoteConnection: persistedRemote)
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
}
