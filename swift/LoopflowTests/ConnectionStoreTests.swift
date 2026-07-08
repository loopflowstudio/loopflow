import Foundation
import Testing
@testable import LoopflowMac
@testable import Loopflow

@MainActor
@Suite("ConnectionStore")
struct ConnectionStoreTests {
    @Test("defaults to bundled mode")
    func defaultsToBundled() {
        let defaults = makeDefaults()
        let store = makeStore(defaults: defaults)

        #expect(store.mode == .bundled)
        #expect(store.activeConnection == .local)
    }

    @Test("persists remote connection mode")
    func persistsRemoteMode() {
        let defaults = makeDefaults()
        let store = makeStore(defaults: defaults)
        let remote = ServerConnection(
            host: "lfd.example.com",
            port: 443,
            useTLS: true,
            authMode: .none
        )

        store.setRemoteConnection(remote)

        let reloaded = makeStore(defaults: defaults)
        #expect(reloaded.mode == .remote)
        #expect(reloaded.activeConnection.host == "lfd.example.com")
        #expect(reloaded.activeConnection.port == 443)
        #expect(reloaded.configuredRemoteConnection?.host == "lfd.example.com")
    }

    @Test("bundled runtime connection is not persisted")
    func bundledRuntimeNotPersisted() {
        let defaults = makeDefaults()
        let store = makeStore(defaults: defaults)
        let remote = ServerConnection(
            host: "lfd.example.com",
            port: 443,
            useTLS: true,
            authMode: .none
        )

        store.setRemoteConnection(remote)
        store.setBundledRuntimeConnection(
            ServerConnection(
                host: "127.0.0.1",
                port: 53001,
                useTLS: false,
                authMode: .staticToken,
                staticToken: "test-token"
            )
        )

        let reloaded = makeStore(defaults: defaults)
        #expect(reloaded.mode == .bundled)
        #expect(reloaded.activeConnection == .local)
        #expect(reloaded.configuredRemoteConnection?.host == "lfd.example.com")
    }

    @Test("seeds remote mode from loopflow config when defaults are empty")
    func seedsFromLoopflowConfig() {
        let defaults = makeDefaults()
        let config = remoteConfig(host: "lfd.example.com")

        let store = makeStore(defaults: defaults, config: config)

        #expect(store.mode == .remote)
        #expect(store.activeConnection.host == "lfd.example.com")
        #expect(store.activeConnection.port == 443)
        #expect(store.activeConnection.useTLS)
        #expect(store.activeConnection.authMode == .staticToken)
    }

    @Test("user defaults settings take priority over loopflow config")
    func userDefaultsWinsOverLoopflowConfig() {
        let defaults = makeDefaults()
        let persisted = ServerConnection(
            host: "saved.example.com",
            port: 443,
            useTLS: true,
            authMode: .none
        )
        let seeded = makeStore(defaults: defaults)
        seeded.setRemoteConnection(persisted)

        let config = remoteConfig(host: "lfd.example.com")
        let reloaded = makeStore(defaults: defaults, config: config)

        #expect(reloaded.mode == .remote)
        #expect(reloaded.activeConnection.host == "saved.example.com")
        #expect(reloaded.activeConnection.port == 443)
    }

    @Test("loopflow config seeds user defaults on first launch")
    func loopflowConfigPersistsAfterFirstLaunch() {
        let defaults = makeDefaults()
        _ = makeStore(defaults: defaults, config: remoteConfig(host: "lfd.example.com"))

        let reloaded = makeStore(defaults: defaults, config: remoteConfig(host: "changed.example.com"))

        #expect(reloaded.mode == .remote)
        #expect(reloaded.activeConnection.host == "lfd.example.com")
    }

    @Test("loopflow config token takes priority and rotates")
    func loopflowConfigTokenTakesPriorityAndRotates() {
        let defaults = makeDefaults()
        let connection = ServerConnection(
            host: "lfd.example.com",
            port: 443,
            useTLS: true,
            authMode: .staticToken,
            staticToken: "stale-token"
        )
        var config = remoteConfig(host: "lfd.example.com", token: "fresh-token")
        let store = makeStore(defaults: defaults, configLoader: { config })

        #expect(store.token(for: connection) == "fresh-token")

        config = remoteConfig(host: "lfd.example.com", token: "rotated-token")

        #expect(store.token(for: connection) == "rotated-token")
    }

    @Test("loopflow config token only applies to matching connection")
    func loopflowConfigTokenRequiresMatchingConnection() {
        let defaults = makeDefaults()
        let connection = ServerConnection(
            host: "other.example.com",
            port: 443,
            useTLS: true,
            authMode: .staticToken,
            staticToken: "connection-token"
        )
        let store = makeStore(
            defaults: defaults,
            config: remoteConfig(host: "lfd.example.com", token: "config-token")
        )

        #expect(store.token(for: connection) == "connection-token")
    }

    @Test("loopback loopflow config is ignored")
    func ignoresLoopbackLoopflowConfig() {
        for host in ["localhost", "127.0.0.1", "::1", " LOCALHOST "] {
            let defaults = makeDefaults()
            let config = remoteConfig(host: host)
            let store = makeStore(defaults: defaults, config: config)

            #expect(store.mode == .bundled)
            #expect(store.activeConnection == .local)
        }
    }

    private func makeDefaults() -> UserDefaults {
        let suite = "ConnectionStoreTests.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defaults.removePersistentDomain(forName: suite)
        return defaults
    }

    private func makeStore(defaults: UserDefaults, config: LoopflowConfig? = nil) -> ConnectionStore {
        makeStore(defaults: defaults, configLoader: { config })
    }

    private func makeStore(defaults: UserDefaults, configLoader: @escaping () -> LoopflowConfig?) -> ConnectionStore {
        ConnectionStore(defaults: defaults, configLoader: configLoader)
    }

    private func remoteConfig(host: String, token: String? = nil) -> LoopflowConfig {
        LoopflowConfig(connection: RemoteConnectionConfig(host: host, port: 443, token: token))
    }
}
