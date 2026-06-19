import Foundation
import Testing

@testable import LoopflowCore

/// Wire-shape fixture tests for DTOs mirrored across Rust / Python / Swift.
///
/// Each fixture under tests/fixtures/dto/ is parsed here and in the Rust and
/// Python test suites. If any mirror drifts, one of the three fails.
@Suite("DTO Fixtures")
struct DTOFixtureTests {
    @Test("session fixture parses with input supported true")
    func sessionFixtureParsesWithInputSupportedTrue() throws {
        let json = try loadFixture("session.json")
        let session = try #require(WaveService.parseSessionFromJSON(json))

        #expect(session.harness == "codex")
        #expect(session.status == "active")
        #expect(session.inputSupported == true)
        #expect(session.waveRunId == "run-abc")
        #expect(session.providerSessionId == "provider-xyz")
        #expect(session.config.step == "design")
        #expect(session.config.repoRoot == "/tmp/repo")
        #expect(session.config.yoloMode == false)
    }

    @Test("session unsupported input fixture parses with input supported false")
    func sessionUnsupportedInputFixtureParsesWithInputSupportedFalse() throws {
        let json = try loadFixture("session_unsupported_input.json")
        let session = try #require(WaveService.parseSessionFromJSON(json))

        #expect(session.harness == "claude")
        #expect(session.status == "failed")
        #expect(session.inputSupported == false)
        #expect(session.endedAt != nil)
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
