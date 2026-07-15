import Foundation
import Testing
@testable import Loopflow

@Suite("Wave endpoint discovery")
struct WaveEndpointTests {
    @Test("the client reads the endpoint from Loopflow's local state")
    func readsEndpointFromLocalState() throws {
        let repo = FileManager.default.temporaryDirectory
            .appendingPathComponent("wave-endpoint-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: repo, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: repo) }

        let endpoint = WaveEndpoint.path(repoPath: repo.path, waveName: "product")
        #expect(
            endpoint.path.hasSuffix(
                "/.lf/journal/waves/product/.wave-endpoint"
            )
        )
        try FileManager.default.createDirectory(
            at: endpoint.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try "127.0.0.1:52340\n".write(to: endpoint, atomically: true, encoding: .utf8)

        #expect(
            WaveEndpoint.read(repoPath: repo.path, waveName: "product")
                == "127.0.0.1:52340"
        )
        let oldEndpoint = repo.appendingPathComponent("wave/product/.wave-endpoint")
        #expect(!FileManager.default.fileExists(atPath: oldEndpoint.path))
    }
}
