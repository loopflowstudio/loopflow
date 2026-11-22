import Foundation
import Testing
@testable import Loopflow

@Suite("NOW condition projection")
struct NowProjectionTests {
    @Test("Each Task lands in the group its shared evidence dictates")
    func nowGroupFollowsSharedEvidence() throws {
        let snapshot = try loadRoadmapFixture()
        let wave = try #require(snapshot.waves.first)
        let tasks = wave.tasks.items

        // A clear Task condition is not a NOW row.
        #expect(nowGroup(for: tasks[0]) == nil)
        // An explicit User merge request is waiting on another actor.
        #expect(nowGroup(for: tasks[1]) == .waiting)
        // No Work record at all → Available, not a NOW row.
        #expect(nowGroup(for: tasks[2]) == nil)
        // Completed → done, not a NOW row.
        #expect(nowGroup(for: tasks[3]) == nil)
    }

    @Test("A Task that waits only to be started is in no group; one waiting on someone is")
    func awaitingStartIsNotANowRow() throws {
        let wire = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: fixture("roadmap_snapshot.json"))) as? [String: Any])
        let items = try #require((wire["tasks"] as? [String: Any])?["items"] as? [[String: Any]])
        func task(lifecycle: String, recommended: String) throws -> RoadmapTask {
            var row = try #require(items.first)
            var condition = try #require(row["condition"] as? [String: Any])
            condition["state"] = "waiting"
            condition["reason"] = "ready"
            row["condition"] = condition
            var actions = try #require(row["actions"] as? [String: Any])
            actions["recommended"] = recommended
            row["actions"] = actions
            row["lifecycle"] = lifecycle
            return try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row))
        }
        let unstarted = try task(lifecycle: "unstarted", recommended: "run")
        let parked = try task(lifecycle: "idle", recommended: "run")
        let merge = try task(lifecycle: "unstarted", recommended: "open_pr")
        #expect(nowGroup(for: unstarted) == nil)
        #expect(nowGroup(for: parked) == .waiting)
        #expect(nowGroup(for: merge) == .waiting)
        #expect(nowGroup(for: try task(lifecycle: "abandoned", recommended: "run")) == nil)
        #expect(nowSections(from: [unstarted, parked], waves: []).flatMap(\.rows).map(\.task) == [parked])
    }

    @Test("NOW flattens across waves and drops empty groups")
    func nowSectionsAreFlatAndGrouped() throws {
        let snapshot = try loadRoadmapFixture()
        let sections = nowSections(from: snapshot.tasks.items, waves: snapshot.waves)

        #expect(sections.map(\.group) == [.waiting])
        // The unavailable `context` wave contributes no rows.
        #expect(sections.flatMap(\.rows).allSatisfy { $0.wave!.name == "product" })
        // Rows carry their Wave/Project as context.
        let input = try #require(sections.first)
        #expect(input.rows.first?.task.task.identifier == "W2-131")
        #expect(input.rows.first?.wave!.name == "product")
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

    private func fixture(_ name: String, sourceFile: String = #filePath) -> URL {
        URL(fileURLWithPath: sourceFile).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto/" + name)
    }

    private func loadRoadmapFixture() throws -> RoadmapSnapshot {
        try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(contentsOf: fixture("roadmap_snapshot.json")))
    }

    private func loadConditionFixture() throws -> [String: RoadmapTask] {
        try JSONDecoder().decode([String: RoadmapTask].self, from: Data(contentsOf: fixture("task_condition_states.json")))
    }
}
