import Foundation
import Testing

@testable import Loopflow

/// Wire-shape fixture tests for DTOs mirrored across Rust / Python / Swift.
///
/// Each fixture under tests/fixtures/dto/ is parsed here and in the Rust and
/// Python test suites. If any mirror drifts, one of the three fails.
@Suite("DTO Fixtures")
struct DTOFixtureTests {
    @Test("session fixture parses palette shape")
    func sessionFixtureParsesPaletteShape() throws {
        let json = try loadFixture("session.json")
        let session = try #require(WaveService.parseSessionFromJSON(json))

        #expect(session.skill == "ship")
        #expect(session.agent == "codex")
        #expect(session.source == "palette")
        #expect(session.sessionUse == .palette)
        #expect(session.status == .running)
        #expect(session.runId == nil)
        #expect(session.parentSessionId == nil)
        #expect(session.argv.contains("-m"))
    }

    @Test("create session request fixture keeps required keys")
    func createSessionRequestFixtureHasRequiredKeys() throws {
        let json = try loadFixture("create_session_request.json")

        #expect(json["wave_id"] as? String == "lfdwave_01HNX7XYZ0AZ1B2C3D4E5F6G7H")
        #expect(json["flow"] as? String == "ship")
        #expect(json["worktree"] as? String == "/tmp/repo.Desktop")
        #expect(json["agent"] as? String == "codex")
    }

    private func loadFixture(_ name: String, sourceFile: String = #filePath) throws -> [String: Any] {
        let testFile = URL(fileURLWithPath: sourceFile)
        let fixtures = testFile
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
            .appendingPathComponent(name)
        let data = try Data(contentsOf: fixtures)
        let json = try JSONSerialization.jsonObject(with: data)
        return try #require(json as? [String: Any])
    }
}
