import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

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

    @Test("seeds remote mode from concerto config when defaults are empty")
    func seedsFromConcertoConfig() {
        let defaults = makeDefaults()
        let config = remoteConfig(host: "lfd-dev.loopflow.studio")

        let store = makeStore(defaults: defaults, config: config)

        #expect(store.mode == .remote)
        #expect(store.activeConnection.host == "lfd-dev.loopflow.studio")
        #expect(store.activeConnection.port == 443)
        #expect(store.activeConnection.useTLS)
        #expect(store.activeConnection.authMode == .staticToken)
    }

    @Test("user defaults settings take priority over concerto config")
    func userDefaultsWinsOverConcertoConfig() {
        let defaults = makeDefaults()
        let persisted = ServerConnection(
            host: "saved.loopflow.studio",
            port: 443,
            useTLS: true,
            authMode: .none
        )
        let seeded = makeStore(defaults: defaults)
        seeded.setRemoteConnection(persisted)

        let config = remoteConfig(host: "lfd-dev.loopflow.studio")
        let reloaded = makeStore(defaults: defaults, config: config)

        #expect(reloaded.mode == .remote)
        #expect(reloaded.activeConnection.host == "saved.loopflow.studio")
        #expect(reloaded.activeConnection.port == 443)
    }

    @Test("concerto config seeds user defaults on first launch")
    func concertoConfigPersistsAfterFirstLaunch() {
        let defaults = makeDefaults()
        _ = makeStore(defaults: defaults, config: remoteConfig(host: "lfd-dev.loopflow.studio"))

        let reloaded = makeStore(defaults: defaults, config: remoteConfig(host: "changed.loopflow.studio"))

        #expect(reloaded.mode == .remote)
        #expect(reloaded.activeConnection.host == "lfd-dev.loopflow.studio")
    }

    @Test("loopback concerto config is ignored")
    func ignoresLoopbackConcertoConfig() {
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

    private func makeStore(defaults: UserDefaults, config: ConcertoConfig? = nil) -> ConnectionStore {
        ConnectionStore(defaults: defaults, configLoader: { config })
    }

    private func remoteConfig(host: String) -> ConcertoConfig {
        ConcertoConfig(connection: RemoteConnectionConfig(host: host, port: 443))
    }
}
