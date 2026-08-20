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

        // Running with a live process → Working.
        #expect(nowGroup(for: tasks[0]) == .working)
        // An explicit User merge request is input only that User can supply.
        #expect(nowGroup(for: tasks[1]) == .needsInput)
        // No Work record at all → Available, not a NOW row.
        #expect(nowGroup(for: tasks[2]) == nil)
        // Completed → done, not a NOW row.
        #expect(nowGroup(for: tasks[3]) == nil)
    }

    @Test("NOW flattens across waves, drops empty groups, and orders by age")
    func nowSectionsAreFlatGroupedAndOldestFirst() throws {
        let snapshot = try loadRoadmapFixture()
        let sections = nowSections(from: snapshot.waves)

        // Only the two non-empty groups survive, in canonical order.
        #expect(sections.map(\.group) == [.needsInput, .working])
        // The unavailable `context` wave contributes no rows.
        #expect(sections.flatMap(\.rows).allSatisfy { $0.wave.name == "product" })
        // Rows carry their Wave/Project as context.
        let input = try #require(sections.first)
        #expect(input.rows.first?.task.task.identifier == "W2-131")
        #expect(input.rows.first?.projectName == "Loopflow API")
    }

    @Test("Age order is oldest updated_at first within a group")
    func ageOrderOldestFirst() throws {
        // The newer Work item is listed first in the snapshot; NOW must reorder it
        // after the older one.
        let wave = try decodeWave(workingTasks: [
            ("W2-2", "2026-07-14T23:00:00Z"),
            ("W2-1", "2026-07-14T20:00:00Z"),
        ])
        let sections = nowSections(from: [wave])
        let working = try #require(sections.first { $0.group == .working })
        #expect(working.rows.map { $0.task.task.identifier } == ["W2-1", "W2-2"])
    }

    @Test("Shared fixture preserves all attention states and the spoken reason")
    func sharedAttentionFixturePreservesProjection() throws {
        let tasks = try loadAttentionFixture()

        #expect(tasks.count == 8)
        #expect(tasks["live_advancing"]?.attention.level == .green)
        #expect(nowGroup(for: try #require(tasks["live_advancing"])) == .working)
        #expect(nowGroup(for: try #require(tasks["live_user_wait"])) == .needsInput)
        #expect(tasks["dead_dirty"]?.attention.localProgress.dirty == true)
        #expect(tasks["dead_authored_commits"]?.attention.localProgress.authoredCommits == true)
        #expect(nowGroup(for: try #require(tasks["clean_backlog"])) == nil)
        #expect(nowGroup(for: try #require(tasks["completed"])) == nil)
        #expect(tasks["stale"]?.attention.localProgress.recoveryRequired == true)
        #expect(nowGroup(for: try #require(tasks["unavailable"])) == .unknown)

        let dirty = try #require(tasks["dead_dirty"])
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

    private func decodeWave(workingTasks: [(String, String)]) throws -> WaveRoadmap {
        let tasks = workingTasks.map { identifier, updatedAt in
            """
            {
              "task": {"id":"\(identifier)","identifier":"\(identifier)","name":"n","description":"","rank":1,"completed":false,"assignee":null},
              "reference": {"issue_url":null,"workspace":null},
              "runtime": {"work_id":"\(identifier)","project_id":"p","status":{"running":{"run_id":"run_00000000000000000000000000000004"}},"reason":"working","updated_at":"\(updatedAt)","provider":"claude","process_alive":true,"observation":{"category":"working","reason":"working","owner":"work","controls":["attach","steer","interrupt","stop"],"progress_age_secs":60,"deadline_in_secs":1740,"step":"iterate"}},
              "next_move": {"owner":"task","reason":"working"},
              "attention": {"level":"green","reason":"working","observed_at":"2026-07-15T00:00:00Z","evidence_age_secs":60,"next_owner":"task","actions":{"recommended":"no_action","reason":"Task body is working"},"pm_completed":false,"work_status":{"running":{"run_id":"run_00000000000000000000000000000004"}},"process":{"state":"observed","alive":true,"reason":null},"local_progress":{"state":"observed","unsettled":false,"dirty":false,"authored_commits":false,"recovery_required":false,"reason":null},"active_pr_phase":null},
              "active_pr": null,
              "section": "now"
            }
            """
        }.joined(separator: ",")
        let json = """
        {
          "wave": {"id":"w","name":"product","status":{"running":{"run_id":"run_00000000000000000000000000000003"}},"goal":"g","repo":"/src/loopflow","active_tasks":\(workingTasks.count),"active_projects":1,"live":true,"paused":false,"enabled":true,"endpoint":null,"created_at":null,"parent_wave_id":null,"home":{"id":"home_00000000000000000000000000000001","route":"local","created_at":"1970-01-01T00:00:00Z","observed_at":"1970-01-01T00:00:00Z"}},
          "projects": {"state":"ok","truncated":false,"items":[
            {"project":{"id":"p","slug":"api","name":"API","summary":"s","definition":"d","flows":{"first":null,"loop":null,"finally":null},"krs":[]},"runtime":null,"next_move":{"owner":"project","reason":"r"},"section":"now","tasks":[\(tasks)]}
          ]},
          "unavailable_projects": []
        }
        """
        return try JSONDecoder().decode(WaveRoadmap.self, from: Data(json.utf8))
    }
}
