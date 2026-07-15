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
        // Idle Session but an open PR → the PR is what waits, so Ready for review.
        #expect(nowGroup(for: tasks[1]) == .readyForReview)
        // No Session at all → Available, not a NOW row.
        #expect(nowGroup(for: tasks[2]) == nil)
        // Completed → done, not a NOW row.
        #expect(nowGroup(for: tasks[3]) == nil)
    }

    @Test("NOW flattens across waves, drops empty groups, and orders by age")
    func nowSectionsAreFlatGroupedAndOldestFirst() throws {
        let snapshot = try loadRoadmapFixture()
        let sections = nowSections(from: snapshot.waves)

        // Only the two non-empty groups survive, in canonical order.
        #expect(sections.map(\.group) == [.readyForReview, .working])
        // The unavailable `context` wave contributes no rows.
        #expect(sections.flatMap(\.rows).allSatisfy { $0.wave.name == "product" })
        // Rows carry their Wave/Project as context.
        let review = try #require(sections.first)
        #expect(review.rows.first?.task.task.identifier == "W2-131")
        #expect(review.rows.first?.projectName == "Loopflow API")
    }

    @Test("Age order is oldest status_at first within a group")
    func ageOrderOldestFirst() throws {
        // The newer Session is listed first in the snapshot; NOW must reorder it
        // after the older one.
        let wave = try decodeWave(workingTasks: [
            ("W2-2", "2026-07-14T23:00:00Z"),
            ("W2-1", "2026-07-14T20:00:00Z"),
        ])
        let sections = nowSections(from: [wave])
        let working = try #require(sections.first { $0.group == .working })
        #expect(working.rows.map { $0.task.task.identifier } == ["W2-1", "W2-2"])
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

    private func decodeWave(workingTasks: [(String, String)]) throws -> WaveRoadmap {
        let tasks = workingTasks.map { identifier, statusAt in
            """
            {
              "task": {"id":"\(identifier)","identifier":"\(identifier)","name":"n","description":"","rank":1,"completed":false,"assignee":null},
              "reference": {"issue_url":null,"workspace":null},
              "runtime": {"session_id":"ts_\(identifier)","project_session_id":"ps_1","status":"running","reason":"working","status_at":"\(statusAt)","provider":"claude","process_alive":true},
              "next_move": {"owner":"task","reason":"working"},
              "active_pr": null,
              "section": "now"
            }
            """
        }.joined(separator: ",")
        let json = """
        {
          "wave": {"id":"w","name":"product","status":"running","paused":false,"goal":"g","repo":"/src/loopflow","active_tasks":\(workingTasks.count),"active_projects":1,"live":true,"endpoint":null,"created_at":null,"parent_wave_id":null,"home":{"address":"jack@local","owner":"jack","location":{"kind":"local"}}},
          "projects": {"state":"ok","truncated":false,"items":[
            {"project":{"id":"p","slug":"api","name":"API","summary":"s","definition":"d","krs":[]},"runtime":null,"next_move":{"owner":"project","reason":"r"},"section":"now","tasks":[\(tasks)]}
          ]}
        }
        """
        return try JSONDecoder().decode(WaveRoadmap.self, from: Data(json.utf8))
    }
}
