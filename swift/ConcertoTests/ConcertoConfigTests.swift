import Foundation
import Testing
@testable import LoopflowCore

@Suite("ConcertoConfig")
struct ConcertoConfigTests {
    @Test("loads remote connection from yaml")
    func loadsRemoteConnectionFromYAML() throws {
        let (configURL, cleanup) = makeConfigURL()
        defer { cleanup() }

        try """
        connection:
          host: lfd-dev.loopflow.studio
          port: 443
        """.write(to: configURL, atomically: true, encoding: .utf8)

        let config = try #require(loadConcertoConfig(configURL: configURL))
        let connection = try #require(config.connection)
        let serverConnection = connection.toServerConnection()

        #expect(connection.host == "lfd-dev.loopflow.studio")
        #expect(connection.port == 443)
        #expect(serverConnection.useTLS)
        #expect(serverConnection.authMode == .staticToken)
    }

    @Test("returns nil when config file is missing")
    func returnsNilWhenConfigMissing() {
        let configURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
            .appendingPathComponent("concerto.yaml", isDirectory: false)

        #expect(loadConcertoConfig(configURL: configURL) == nil)
    }

    @Test("returns nil when config file is empty")
    func returnsNilWhenConfigEmpty() throws {
        let (configURL, cleanup) = makeConfigURL()
        defer { cleanup() }

        try " \n\t\n".write(to: configURL, atomically: true, encoding: .utf8)

        #expect(loadConcertoConfig(configURL: configURL) == nil)
    }

    @Test("returns config with no connection when yaml has no connection key")
    func returnsConfigWithoutConnectionWhenMissingKey() throws {
        let (configURL, cleanup) = makeConfigURL()
        defer { cleanup() }

        try """
        somethingElse:
          host: lfd-dev.loopflow.studio
        """.write(to: configURL, atomically: true, encoding: .utf8)

        let config = try #require(loadConcertoConfig(configURL: configURL))
        #expect(config.connection == nil)
    }

    @Test("ignores nested connection key")
    func ignoresNestedConnectionKey() throws {
        let (configURL, cleanup) = makeConfigURL()
        defer { cleanup() }

        try """
        profile:
          connection:
            host: lfd-dev.loopflow.studio
            port: 443
        """.write(to: configURL, atomically: true, encoding: .utf8)

        let config = try #require(loadConcertoConfig(configURL: configURL))
        #expect(config.connection == nil)
    }

    @Test("parses container mounts and image")
    func parsesContainerMountsAndImage() throws {
        let (configURL, cleanup) = makeConfigURL()
        defer { cleanup() }

        try """
        container:
          image: loopflow/lfd:dev
          mounts:
            - ~/Documents/specs:ro
            - ~/data:rw
            - ~/defaults
        """.write(to: configURL, atomically: true, encoding: .utf8)

        let config = try #require(loadConcertoConfig(configURL: configURL))
        let container = try #require(config.container)
        let mounts = try #require(container.mounts)

        #expect(container.image == "loopflow/lfd:dev")
        #expect(mounts == [
            ExtraMount(path: "~/Documents/specs", readOnly: true),
            ExtraMount(path: "~/data", readOnly: false),
            ExtraMount(path: "~/defaults", readOnly: true),
        ])
    }

    @Test("returns nil container when missing keys")
    func returnsNilContainerWhenMissingKeys() throws {
        let (configURL, cleanup) = makeConfigURL()
        defer { cleanup() }

        try """
        container:
          somethingElse: value
        """.write(to: configURL, atomically: true, encoding: .utf8)

        let config = try #require(loadConcertoConfig(configURL: configURL))
        #expect(config.container == nil)
    }

    private func makeConfigURL() -> (URL, () -> Void) {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let configURL = directory.appendingPathComponent("concerto.yaml", isDirectory: false)
        let cleanup: () -> Void = {
            _ = try? FileManager.default.removeItem(at: directory)
        }
        return (configURL, cleanup)
    }
}
