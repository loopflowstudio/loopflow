import Foundation
import LoopflowCore

@MainActor
@Observable
final class ConnectionStore {
    private let defaults: UserDefaults
    private static let defaultsKey = "concerto.serverConnection.v1"
    private let secretStore: ConnectionSecretStore
    private let pinStore: CertificatePinStore

    var activeConnection: ServerConnection {
        didSet {
            persistConnection(activeConnection)
        }
    }

    init(
        secretStore: ConnectionSecretStore = .shared,
        pinStore: CertificatePinStore = .shared,
        defaults: UserDefaults = .standard
    ) {
        self.secretStore = secretStore
        self.pinStore = pinStore
        self.defaults = defaults

        if let loaded = Self.loadConnection(from: defaults) {
            var connection = loaded
            connection.staticToken = secretStore.token(for: loaded)
            activeConnection = connection
        } else {
            activeConnection = .local
        }
    }

    func setConnection(_ connection: ServerConnection) {
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

        activeConnection = next
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

    private func persistConnection(_ connection: ServerConnection) {
        var value = connection
        value.staticToken = nil

        guard let data = try? JSONEncoder().encode(value) else { return }
        defaults.set(data, forKey: Self.defaultsKey)
    }

    private static func loadConnection(from defaults: UserDefaults) -> ServerConnection? {
        guard let data = defaults.data(forKey: Self.defaultsKey) else { return nil }
        return try? JSONDecoder().decode(ServerConnection.self, from: data)
    }
}
