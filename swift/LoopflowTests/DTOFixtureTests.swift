import Foundation
import Testing

@testable import Loopflow

/// Wire-shape fixture tests for DTOs mirrored across Rust / Python / Swift.
///
/// Each fixture under tests/fixtures/dto/ is parsed here and in the Rust and
/// Python test suites. If any mirror drifts, one of the three fails.
@Suite("DTO Fixtures")
struct DTOFixtureTests {
    @Test("wave detail fixture preserves Project and Task identity")
    func waveDetailFixturePreservesHierarchy() throws {
        let data = try loadFixtureData("wave_detail.json")
        let detail = try JSONDecoder().decode(WaveStatusSnapshot.self, from: data)

        #expect(detail.projects[0].project.slug == "release-feedback")
        #expect(detail.projects[0].tasks.map(\.task.identifier) == ["INF-123", "INF-124"])
        #expect(detail.projects[0].tasks[0].delivery?.prNumber == 912)
        #expect(detail.projects[0].directive?.version == 1)
        #expect(detail.projects[0].tasks[0].directive?.version == 2)
        #expect(detail.projects[0].tasks[0].directive?.incorporatedAt != nil)
        #expect(detail.projects[0].tasks[1].runtime == nil)
    }

    @Test("child control activity preserves directive evidence")
    func childControlActivityPreservesDirectiveEvidence() throws {
        let data = try loadFixtureData("child_control_activity.json")
        let activity = try JSONDecoder().decode(ChildControlActivity.self, from: data)

        #expect(activity.subject == .task)
        #expect(activity.subjectId == "INF-123")
        #expect(activity.kind == .incorporated)
        #expect(activity.directiveVersion == 2)
        #expect(activity.effect == "live_steer")
    }

    private func loadFixture(_ name: String, sourceFile: String = #filePath) throws -> [String: Any] {
        let data = try loadFixtureData(name, sourceFile: sourceFile)
        let json = try JSONSerialization.jsonObject(with: data)
        return try #require(json as? [String: Any])
    }

    private func loadFixtureData(_ name: String, sourceFile: String = #filePath) throws -> Data {
        let testFile = URL(fileURLWithPath: sourceFile)
        let fixtures = testFile
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
            .appendingPathComponent(name)
        return try Data(contentsOf: fixtures)
    }
}
