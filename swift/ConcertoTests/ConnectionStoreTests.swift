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
        let store = ConnectionStore(defaults: defaults)

        #expect(store.mode == .bundled)
        #expect(store.activeConnection == .local)
    }

    @Test("persists remote connection mode")
    func persistsRemoteMode() {
        let defaults = makeDefaults()
        let store = ConnectionStore(defaults: defaults)
        let remote = ServerConnection(
            host: "lfd.example.com",
            port: 443,
            useTLS: true,
            authMode: .none
        )

        store.setRemoteConnection(remote)

        let reloaded = ConnectionStore(defaults: defaults)
        #expect(reloaded.mode == .remote)
        #expect(reloaded.activeConnection.host == "lfd.example.com")
        #expect(reloaded.activeConnection.port == 443)
        #expect(reloaded.configuredRemoteConnection?.host == "lfd.example.com")
    }

    @Test("bundled runtime connection is not persisted")
    func bundledRuntimeNotPersisted() {
        let defaults = makeDefaults()
        let store = ConnectionStore(defaults: defaults)
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

        let reloaded = ConnectionStore(defaults: defaults)
        #expect(reloaded.mode == .bundled)
        #expect(reloaded.activeConnection == .local)
        #expect(reloaded.configuredRemoteConnection?.host == "lfd.example.com")
    }

    private func makeDefaults() -> UserDefaults {
        let suite = "ConnectionStoreTests.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defaults.removePersistentDomain(forName: suite)
        return defaults
    }
}
