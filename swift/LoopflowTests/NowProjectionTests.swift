import Foundation
import Testing
@testable import Loopflow

@Suite("NOW condition projection")
struct NowProjectionTests {
    @Test("Each Task lands in the group its shared evidence dictates")
    func needFollowsSharedEvidence() throws {
        let snapshot = try loadRoadmapFixture()
        let wave = try #require(snapshot.waves.first)
        let tasks = wave.tasks.items

        // A clear Task condition is not a NOW row.
        #expect(tasks[0].need == nil)
        // An explicit User merge request is waiting on another actor.
        #expect(tasks[1].need == .waiting)
        // No Work record at all → Available, not a NOW row.
        #expect(tasks[2].need == nil)
        // Completed → done, not a NOW row.
        #expect(tasks[3].need == nil)
    }

    @Test("A Task that waits only to be started is in no group; one waiting on someone is")
    func awaitingStartIsNotANowRow() throws {
        func task(lifecycle: String, recommended: String) throws -> RoadmapTask {
            try roadmapTask(condition: "waiting", reason: "ready", lifecycle: lifecycle, recommended: recommended)
        }
        let unstarted = try task(lifecycle: "unstarted", recommended: "run")
        let parked = try task(lifecycle: "idle", recommended: "run")
        let merge = try task(lifecycle: "unstarted", recommended: "open_pr")
        #expect(unstarted.need == nil)
        #expect(parked.need == .waiting)
        #expect(merge.need == .waiting)
        #expect(try task(lifecycle: "abandoned", recommended: "run").need == nil)
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
        #expect(try #require(tasks["user_wait"]).need == .waiting)
        #expect(try #require(tasks["stale"]).need == .blocked)
        #expect(try #require(tasks["dirty"]).need == .blocked)
        #expect(tasks["dirty"]?.condition.localProgress.dirty == true)
        #expect(tasks["authored_commits"]?.condition.localProgress.authoredCommits == true)
        #expect(try #require(tasks["clean_backlog"]).need == nil)
        #expect(try #require(tasks["completed"]).need == nil)
        #expect(tasks["stale"]?.condition.localProgress.recoveryRequired == true)
        #expect(try #require(tasks["unavailable"]).need == .unknown)

        let dirty = try #require(tasks["dirty"])
        let spoken = taskConditionAccessibilityLabel(dirty)
        #expect(spoken.contains(dirty.condition.reason))
        #expect(spoken.contains("Next owner: task"))
    }

    // MARK: - Fixtures

    private func loadRoadmapFixture() throws -> RoadmapSnapshot {
        try JSONDecoder().decode(RoadmapSnapshot.self, from: loadFixtureData("roadmap_snapshot.json"))
    }

    private func loadConditionFixture() throws -> [String: RoadmapTask] {
        try JSONDecoder().decode([String: RoadmapTask].self, from: loadFixtureData("task_condition_states.json"))
    }
}
