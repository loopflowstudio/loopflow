#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Wave standup")
@MainActor
struct WaveStandupTests {
    private let now = ISO8601DateFormatter().date(from: "2026-10-26T12:00:00Z")!
    private var fixtures: URL {
        URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto")
    }

    private func task(_ id: String, section: String, state: String? = nil, completedAt: String? = nil,
                      rank: Int = 0, owner: String? = nil, pr: String? = nil, reason: String = "Next move") throws -> RoadmapTask {
        let rows = try #require(JSONSerialization.jsonObject(
            with: Data(contentsOf: fixtures.appendingPathComponent("task_history_rows.json"))) as? [[String: Any]])
        var row = rows[0]
        var plan = try #require(row["task"] as? [String: Any])
        plan["id"] = id
        plan["identifier"] = id
        plan["name"] = "Task \(id)"
        plan["rank"] = rank
        plan["state"] = state.map { $0 as Any } ?? NSNull()
        plan["completed"] = state == "completed"
        plan["completed_at"] = completedAt.map { $0 as Any } ?? NSNull()
        row["task"] = plan
        row["section"] = section
        row["next_move"] = ["owner": "task", "reason": reason]
        row["runtime"] = owner.map { person -> Any in
            ["work_id": "work-\(id)", "status": "ready", "reason": "Running", "updated_at": "2026-10-26T12:00:00Z",
             "provider": "claude", "completion_pending": NSNull(),
             "owner": ["person": person, "person_key": "\(person.lowercased().replacingOccurrences(of: " ", with: "."))@example.com",
                       "machine": ["id": NSNull(), "label": NSNull()]] as [String: Any]] as [String: Any]
        } ?? NSNull()
        row["pr"] = pr.map { phase -> Any in
            ["id": "pr-\(id)", "sequence": 1, "slug": id, "branch": id, "base_commit": "abc", "phase": phase, "empty": NSNull(),
             "publication": NSNull(), "merge_commit": NSNull(), "abandoned_at": NSNull()] as [String: Any]
        } ?? NSNull()
        return try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row))
    }

    @Test func bucketsDoneActiveAndBacklog() throws {
        let tasks = [
            try task("DONE-1", section: "later", state: "completed", completedAt: "2026-10-25T09:00:00Z"),
            try task("DONE-OLD", section: "later", state: "completed", completedAt: "2026-10-01T09:00:00Z"),
            try task("CANCELED", section: "later", state: "canceled", completedAt: "2026-10-25T09:00:00Z"),
            try task("NOW", section: "now", rank: 2, owner: "Jack Heart"),
            try task("WAIT", section: "waiting", rank: 1, owner: "Jack Heart", reason: "Waiting on CI"),
            try task("MERGED", section: "now", rank: 3, owner: "Jack Heart", pr: "merged"),
            try task("LATER", section: "later", rank: 0),
            try task("FRESH", section: "not_started", rank: 5),
        ]
        let standup = WaveStandup(tasks: tasks, closed: tasks, doneDays: 7, now: now)
        #expect(standup.done.map(\.id) == ["DONE-1"])
        #expect(standup.active.map(\.id) == ["WAIT", "NOW", "MERGED"])
        #expect(standup.active[0].note == "Waiting on CI")
        #expect(standup.active[2].note == "merged")
        // Not started comes first; terminal Tasks never sit in Backlog.
        #expect(standup.backlog.map(\.id) == ["FRESH", "LATER"])
        // One person owns everything: a solo Wave shows no pictures.
        #expect(!standup.showsPeople)
        #expect(WaveStandup(tasks: tasks, closed: tasks, doneDays: 0, now: now).done.map(\.id).sorted() == ["DONE-1", "DONE-OLD"])
    }

    @Test func doneWindowCountsWholeDaysBackFromNow() throws {
        let edge = try task("EDGE", section: "later", state: "completed", completedAt: "2026-10-19T12:00:00Z")
        let past = try task("PAST", section: "later", state: "completed", completedAt: "2026-10-19T11:59:59Z")
        let undated = try task("UNDATED", section: "later", state: "completed")
        #expect(WaveStandup(tasks: [edge, past, undated], closed: [edge, past, undated], doneDays: 7, now: now).done.map(\.id) == ["EDGE"])
        #expect(WaveStandup(tasks: [edge, past], closed: [edge, past], doneDays: 14, now: now).done.count == 2)
        // Done reads only the closed source, so it can move to `lf task list --done`.
        #expect(WaveStandup(tasks: [edge], closed: [], doneDays: 0, now: now).done.isEmpty)
        #expect(WaveStandup(tasks: [], closed: [edge], doneDays: 7, now: now).done.map(\.id) == ["EDGE"])
    }

    @Test func peopleAppearOnceTwoPeopleShareTheWave() throws {
        let tasks = [
            try task("A", section: "now", owner: "Jack Heart"),
            try task("B", section: "now", owner: "Ada Lovelace"),
        ]
        let standup = WaveStandup(tasks: tasks, closed: tasks, doneDays: 7, now: now)
        #expect(standup.showsPeople)
        #expect(standup.active.compactMap(\.person?.initials) == ["JH", "AL"])
        let text = standup.text(roadmap: try roadmap())
        #expect(text.contains("A Task A · Jack Heart"))
        #expect(text.contains("Done (last 7 days) · 0\n  None"))
    }

    @Test func boardRendersHeaderColumnsAndMetrics() throws {
        let roadmap = try roadmap()
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Rendering is passive") },
                              repoPath: roadmap.wave.repo)
        model.applyFixture(roadmap: .available(try snapshot()), waves: .available([]), workActivity: .loading, repos: [])
        let board = WaveStandupBoard(model: model, waveID: roadmap.wave.id)
        #expect(try board.inspect().find(viewWithAccessibilityIdentifier: "wave-title").text().string() == roadmap.wave.displayName)
        #expect(try board.inspect().find(viewWithAccessibilityIdentifier: "wave-metric-headline").text().string()
            == WaveMetricPortfolioPresentation(portfolio: roadmap.metricPortfolio).headline)
        for column in ["done", "active", "backlog"] {
            _ = try board.inspect().find(viewWithAccessibilityIdentifier: "wave-column-\(column)")
        }
        for notice in roadmap.unavailableTasks {
            _ = try board.inspect().find(viewWithAccessibilityIdentifier: "wave-notice-\(notice.taskId)")
        }
        if let project = roadmap.currentProject {
            #expect(try board.inspect().find(viewWithAccessibilityIdentifier: "wave-current-project").text().string()
                .hasPrefix(project.name))
        }
    }

    private func snapshot() throws -> RoadmapSnapshot {
        try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")))
    }

    private func roadmap() throws -> WaveRoadmap { try #require(try snapshot().waves.first) }
}
#endif
