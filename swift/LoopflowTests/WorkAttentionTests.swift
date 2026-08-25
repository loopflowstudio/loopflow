import Foundation
import Testing
@testable import Loopflow

@Suite("NOW attention derivation")
struct WorkAttentionTests {
    @Test("Each Task lands in the group its shared evidence dictates")
    func nowGroupFollowsSharedEvidence() throws {
        let snapshot = try loadRoadmapFixture()
        let project = try #require(snapshot.waves.first?.projects.items.first)
        let tasks = project.tasks

        // Ready planning state without attention evidence is not a NOW row.
        #expect(nowGroup(for: tasks[0]) == nil)
        // An explicit User merge request is input only that User can supply.
        #expect(nowGroup(for: tasks[1]) == .needsInput)
        // No Work record at all → Available, not a NOW row.
        #expect(nowGroup(for: tasks[2]) == nil)
        // Completed → done, not a NOW row.
        #expect(nowGroup(for: tasks[3]) == nil)
    }

    @Test("NOW flattens across waves and drops empty groups")
    func nowSectionsAreFlatAndGrouped() throws {
        let snapshot = try loadRoadmapFixture()
        let sections = nowSections(from: snapshot.waves)

        #expect(sections.map(\.group) == [.needsInput])
        // The unavailable `context` wave contributes no rows.
        #expect(sections.flatMap(\.rows).allSatisfy { $0.wave.name == "product" })
        // Rows carry their Wave/Project as context.
        let input = try #require(sections.first)
        #expect(input.rows.first?.task.task.identifier == "W2-131")
        #expect(input.rows.first?.projectName == "Loopflow API")
    }

    @Test("Shared fixture preserves all attention states and the spoken reason")
    func sharedAttentionFixturePreservesProjection() throws {
        let tasks = try loadAttentionFixture()

        #expect(tasks.count == 7)
        #expect(nowGroup(for: try #require(tasks["user_wait"])) == .needsInput)
        #expect(tasks["dirty"]?.attention.localProgress.dirty == true)
        #expect(tasks["authored_commits"]?.attention.localProgress.authoredCommits == true)
        #expect(nowGroup(for: try #require(tasks["clean_backlog"])) == nil)
        #expect(nowGroup(for: try #require(tasks["completed"])) == nil)
        #expect(tasks["stale"]?.attention.localProgress.recoveryRequired == true)
        #expect(nowGroup(for: try #require(tasks["unavailable"])) == .unknown)

        let dirty = try #require(tasks["dirty"])
        let spoken = taskAttentionAccessibilityLabel(dirty)
        #expect(spoken.contains(dirty.attention.reason))
        #expect(spoken.contains("Next owner: task"))
    }

    // MARK: - Fixtures

    private func loadRoadmapFixture(sourceFile: String = #filePath) throws -> RoadmapSnapshot {
        let fixture = URL(fileURLWithPath: sourceFile)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        return try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(contentsOf: fixture))
    }

    private func loadAttentionFixture(
        sourceFile: String = #filePath
    ) throws -> [String: RoadmapTask] {
        let fixture = URL(fileURLWithPath: sourceFile)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_attention_states.json")
        return try JSONDecoder().decode(
            [String: RoadmapTask].self,
            from: Data(contentsOf: fixture)
        )
    }

}
