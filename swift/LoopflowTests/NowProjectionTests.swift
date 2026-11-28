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
        func task(lifecycle: String, recommended: String, graphHasRun: Bool) throws -> RoadmapTask {
            try roadmapTask(condition: "waiting", reason: "ready", lifecycle: lifecycle, recommended: recommended,
                            graphHasRun: graphHasRun)
        }
        let unstarted = try task(lifecycle: "unstarted", recommended: "run", graphHasRun: false)
        // Opened by its link: it has a checkout, so the server calls it idle, and no Graph has run.
        let opened = try task(lifecycle: "idle", recommended: "run", graphHasRun: false)
        let parked = try task(lifecycle: "idle", recommended: "run", graphHasRun: true)
        let merge = try task(lifecycle: "unstarted", recommended: "open_pr", graphHasRun: false)
        #expect(unstarted.need == nil)
        #expect(opened.need == nil)
        #expect(parked.need == .waiting)
        #expect(merge.need == .waiting)
        #expect(try task(lifecycle: "abandoned", recommended: "run", graphHasRun: true).need == nil)
        #expect(nowSections(from: [unstarted, opened, parked], waves: []).flatMap(\.rows).map(\.task) == [parked])
    }

    @Test("Pausing removes a Task from Now while its previous condition is still visible")
    func pauseClearsNeedBeforeTheNextReading() throws {
        var task = try #require(loadConditionFixture()["dirty"])
        #expect(nowSections(from: [task], waves: []).flatMap(\.rows).map(\.id) == [task.id])

        task.lifecycle = .paused
        #expect(task.condition.localProgress.dirty == true)
        #expect(nowSections(from: [task], waves: []).isEmpty)
        #expect(taskConditionAccessibilityLabel(task) == "\(task.task.identifier), \(task.task.name).")
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
        #expect(spoken.contains("Blocked. \(dirty.condition.reason)."))
        #expect(spoken.contains("Next owner: task"))
        // A Task that needs nobody speaks its ID and name, not "ready".
        let backlog = try #require(tasks["clean_backlog"])
        #expect(taskConditionAccessibilityLabel(backlog) == "\(backlog.task.identifier), \(backlog.task.name).")
    }

    // MARK: - Fixtures

    private func loadRoadmapFixture() throws -> RoadmapSnapshot {
        try JSONDecoder().decode(RoadmapSnapshot.self, from: loadFixtureData("roadmap_snapshot.json"))
    }

    private func loadConditionFixture() throws -> [String: RoadmapTask] {
        try JSONDecoder().decode([String: RoadmapTask].self, from: loadFixtureData("task_condition_states.json"))
    }
}
