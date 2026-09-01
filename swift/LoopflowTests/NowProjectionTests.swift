import Foundation
import Testing
@testable import Loopflow

@Suite("NOW condition projection")
struct NowProjectionTests {
    @Test("Each Task lands in the group its shared evidence dictates")
    func nowGroupFollowsSharedEvidence() throws {
        let snapshot = try loadRoadmapFixture()
        let project = try #require(snapshot.waves.first?.projects.items.first)
        let tasks = project.tasks

        // A clear Task condition is not a NOW row.
        #expect(nowGroup(for: tasks[0]) == nil)
        // An explicit User merge request is waiting on another actor.
        #expect(nowGroup(for: tasks[1]) == .waiting)
        // No Work record at all → Available, not a NOW row.
        #expect(nowGroup(for: tasks[2]) == nil)
        // Completed → done, not a NOW row.
        #expect(nowGroup(for: tasks[3]) == nil)
    }

    @Test("NOW flattens across waves and drops empty groups")
    func nowSectionsAreFlatAndGrouped() throws {
        let snapshot = try loadRoadmapFixture()
        let sections = nowSections(from: snapshot.waves)

        #expect(sections.map(\.group) == [.waiting])
        // The unavailable `context` wave contributes no rows.
        #expect(sections.flatMap(\.rows).allSatisfy { $0.wave.name == "product" })
        // Rows carry their Wave/Project as context.
        let input = try #require(sections.first)
        #expect(input.rows.first?.task.task.identifier == "W2-131")
        #expect(input.rows.first?.projectName == "Loopflow API")
    }

    @Test("Shared fixture preserves all Task conditions and the spoken reason")
    func sharedConditionFixturePreservesProjection() throws {
        let tasks = try loadConditionFixture()

        #expect(tasks.count == 7)
        #expect(nowGroup(for: try #require(tasks["user_wait"])) == .waiting)
        #expect(nowGroup(for: try #require(tasks["stale"])) == .blocked)
        #expect(nowGroup(for: try #require(tasks["dirty"])) == .blocked)
        #expect(tasks["dirty"]?.condition.localProgress.dirty == true)
        #expect(tasks["authored_commits"]?.condition.localProgress.authoredCommits == true)
        #expect(nowGroup(for: try #require(tasks["clean_backlog"])) == nil)
        #expect(nowGroup(for: try #require(tasks["completed"])) == nil)
        #expect(tasks["stale"]?.condition.localProgress.recoveryRequired == true)
        #expect(nowGroup(for: try #require(tasks["unavailable"])) == .unknown)

        let dirty = try #require(tasks["dirty"])
        let spoken = taskConditionAccessibilityLabel(dirty)
        #expect(spoken.contains(dirty.condition.reason))
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

    private func loadConditionFixture(
        sourceFile: String = #filePath
    ) throws -> [String: RoadmapTask] {
        let fixture = URL(fileURLWithPath: sourceFile)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_condition_states.json")
        return try JSONDecoder().decode(
            [String: RoadmapTask].self,
            from: Data(contentsOf: fixture)
        )
    }

}
