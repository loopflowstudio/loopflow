import Foundation
import Testing
@testable import LoopflowCore

@Suite("ServerConnection")
struct ServerConnectionTests {
    @Test("local connection uses HTTP and plain WebSocket")
    func localConnectionURLs() {
        let connection = ServerConnection.local

        #expect(connection.httpBaseURL.absoluteString == "http://127.0.0.1:2486")
        #expect(connection.wsBaseURL.absoluteString == "ws://127.0.0.1:2486/ws")
        #expect(connection.isLocal)
        #expect(connection.displayName == "http://127.0.0.1:2486")
    }

    @Test("TLS connection uses HTTPS and secure WebSocket")
    func tlsConnectionURLs() {
        let connection = ServerConnection(
            host: "example.com",
            port: 443,
            useTLS: true,
            authMode: .staticToken,
            staticToken: "token"
        )

        #expect(connection.httpBaseURL.absoluteString == "https://example.com:443")
        #expect(connection.wsBaseURL.absoluteString == "wss://example.com:443/ws")
        #expect(!connection.isLocal)
        #expect(connection.connectionKey == "example.com:443")
    }

    @Test("localhost with TLS or auth is not treated as local")
    func localhostTLSOrAuthIsNotLocal() {
        let localhostTLS = ServerConnection(
            host: "127.0.0.1",
            port: 443,
            useTLS: true,
            authMode: .none
        )
        let localhostAuth = ServerConnection(
            host: "127.0.0.1",
            port: 2486,
            useTLS: false,
            authMode: .staticToken,
            staticToken: "token"
        )

        #expect(!localhostTLS.isLocal)
        #expect(!localhostAuth.isLocal)
    }
}

@Suite("RepoTarget")
struct RepoTargetTests {
    @Test("remote target exposes server path, host, and display name")
    func remoteTargetDisplay() {
        let target = RepoTarget.remote(path: "/srv/repos/loopflow", host: "ec2-host.example.com")

        #expect(target.path == "/srv/repos/loopflow")
        #expect(target.displayName == "loopflow")
        #expect(target.isRemote)
        #expect(target.remoteHost == "ec2-host.example.com")
        #expect(target.localURL == nil)
    }

    @Test("local target exposes URL path")
    func localTargetDisplay() {
        let url = URL(fileURLWithPath: "/Users/jack/src/loopflow")
        let target = RepoTarget.local(url)

        #expect(target.path == "/Users/jack/src/loopflow")
        #expect(target.displayName == "loopflow")
        #expect(!target.isRemote)
        #expect(target.localURL == url)
    }
}
