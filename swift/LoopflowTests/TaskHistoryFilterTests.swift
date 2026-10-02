#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Task history")
@MainActor
struct TaskHistoryFilterTests {
    private let now = ISO8601DateFormatter().date(from: "2026-10-02T12:00:00Z")!
    private var fixtures: URL {
        URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto")
    }

    private func roadmapWire() throws -> [String: Any] {
        var root = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json"))) as? [String: Any])
        var waves = try #require(root["waves"] as? [[String: Any]])
        var evidence = try #require(waves[0]["tasks"] as? [String: Any])
        var rows = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: fixtures.appendingPathComponent("task_history_rows.json"))) as? [[String: Any]])
        if let index = rows.firstIndex(where: { ($0["task"] as? [String: Any])?["id"] as? String == "unresolved" }) {
            rows[index]["runtime"] = ["work_id": "task-unresolved", "status": "ready", "reason": "Running", "updated_at": "2026-10-02T12:00:00Z", "provider": "codex", "started": true] as [String: Any]
        }
        evidence["items"] = rows
        waves[0]["tasks"] = evidence
        root["waves"] = [waves[0]]
        return root
    }

    private func task(_ state: String, date: String?, runtime: String? = nil, missing: Bool = false) throws -> RoadmapTask {
        let wire = try roadmapWire()
        let waves = try #require(wire["waves"] as? [[String: Any]])
        let evidence = try #require(waves[0]["tasks"] as? [String: Any])
        var row = try #require((evidence["items"] as? [[String: Any]])?.first)
        var plan = try #require(row["task"] as? [String: Any])
        plan["state"] = state
        plan["completed"] = state == "completed"
        plan["completed_at"] = date.map { $0 as Any } ?? NSNull()
        row["task"] = plan
        if let runtime {
            row["runtime"] = ["work_id": "task", "status": runtime, "reason": "settled", "updated_at": "2026-10-02T12:00:00Z", "provider": "codex", "started": true] as [String: Any]
        }
        row["condition"] = ["state": missing ? "blocked" : "clear", "reason": "historical checkout", "observed_at": "2026-10-02T12:00:00Z", "evidence_age_secs": 0,
            "local_progress": ["state": missing ? "missing" : "not_applicable", "unsettled": missing, "dirty": NSNull(), "authored_commits": NSNull(), "recovery_required": missing, "reason": "checkout removed"]] as [String: Any]
        return try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row))
    }

    private func visible(_ row: RoadmapTask, _ filter: TaskHistoryFilter) -> Bool {
        filter.includes(row.task, runtime: row.runtime, condition: row.condition, flow: row.flow, now: now)
    }

    @Test func rollingBoundariesAndValidation() throws {
        var filter = TaskHistoryFilter()
        filter.showCompleted = true
        for date in ["2026-09-25T12:00:00Z", "2026-09-25T05:00:00-07:00", "2026-10-02T12:00:00.000Z"] {
            #expect(visible(try task("completed", date: date), filter))
        }
        for date in ["2026-09-25T11:59:59.9999Z", "2026-10-02T12:00:00.0001Z", "2026-09-12T12:00:00Z"] {
            #expect(!visible(try task("completed", date: date), filter))
        }
        #expect(!visible(try task("completed", date: nil), filter))
        filter.editDays("30")
        #expect(visible(try task("completed", date: "2026-09-12T12:00:00Z"), filter))
        for invalid in ["", "0", "-1", "1.5", "999999999999999999999999999"] {
            filter.editDays(invalid)
            #expect(filter.validationMessage != nil)
            #expect(filter.days == 30)
        }
        filter.allTime = true
        #expect(visible(try task("completed", date: nil), filter))
        for state in ["canceled", "duplicate"] {
            #expect(!visible(try task(state, date: nil), filter))
        }
        #expect(visible(try task("unstarted", date: "2026-01-01T00:00:00Z", runtime: "abandoned"), TaskHistoryFilter()))
        #expect(visible(try task("unknown", date: nil), TaskHistoryFilter()))
    }

    @Test func settledMissingCheckoutDoesNotBypassHistoryWindow() throws {
        let row = try task("completed", date: "2026-09-12T12:00:00Z", runtime: "done", missing: true)
        var filter = TaskHistoryFilter()
        #expect(!visible(row, filter))
        filter.showCompleted = true
        #expect(!visible(row, filter))
        filter.editDays("30")
        #expect(visible(row, filter))
        #expect(visible(try task("canceled", date: nil, runtime: "ready", missing: true), TaskHistoryFilter()))
    }

    @Test func unresolvedReviewAndRetainedSessionSurviveHiddenHistory() throws {
        let row = try task("canceled", date: nil, runtime: "done", missing: true)
        let pinned = PinnedTaskFlow(invocationId: "review", graph: FlowGraph(name: "feature", steps: [], interactions: InteractionGraph(stages: [], transitions: [])), current: nil, completed: [], returns: [], iterations: [], execution: .human, reason: "Review remains open", restartRequired: false)
        let flow = TaskFlowSnapshot(recommended: "feature", record: .pinned(pinned), controls: [])
        #expect(TaskHistoryFilter().includes(row.task, runtime: row.runtime, condition: row.condition, flow: flow, now: now))
        let sessions = try JSONDecoder().decode([SessionRecord].self, from: Data(contentsOf: fixtures.appendingPathComponent("sessions.json")))
        let retained = WorkspaceTask(id: WorkspaceNodeKey(repo: "/repo", work: .task(id: row.id)), task: row, sessions: [try #require(sessions.first)])
        #expect(!visible(row, TaskHistoryFilter()))
        #expect(retained.inWorkingSet)
        #expect(retained.sessions.first?.id == sessions.first?.id)
    }

    @Test func rollingDurationCrossesDSTWithoutCalendarDayArithmetic() throws {
        var filter = TaskHistoryFilter()
        filter.showCompleted = true
        let boundary = try task("completed", date: "2026-03-05T07:00:00-08:00")
        let before = try task("completed", date: "2026-03-05T06:59:59.9999-08:00")
        let springNow = ISO8601DateFormatter().date(from: "2026-03-12T08:00:00-07:00")!
        #expect(filter.includes(boundary.task, runtime: nil, condition: boundary.condition, flow: boundary.flow, now: springNow))
        #expect(!filter.includes(before.task, runtime: nil, condition: before.condition, flow: before.flow, now: springNow))
    }

    @Test func waveControlsFilterRowsAndCountTogether() async throws {
        let wire = try roadmapWire()
        let payload = String(decoding: try JSONSerialization.data(withJSONObject: wire), as: UTF8.self)
        let query = RegistryQuery { _, _ in payload }
        let roadmap = try await query.roadmap()
        let wave = try #require(roadmap.waves.first)
        let model = PodiumModel(query: query)
        model.applyFixture(roadmap: .available(roadmap), waves: .available([wave.wave.toWave()]), processActivity: .loading, workActivity: .loading, repos: [])
        model.select(.wave(id: wave.wave.id))
        model.taskHistoryNow = now
        let view = WorkSurfaceView(model: model)
        func rows() throws -> [String] {
            try view.inspect().findAll(ViewType.Button.self).compactMap { button in
                guard let id = try? button.accessibilityIdentifier(), id.hasPrefix("podium-task-") else { return nil }
                return String(id.dropFirst("podium-task-".count))
            }
        }
        func checkCount() throws {
            let heading = try view.inspect().find(viewWithAccessibilityIdentifier: "wave-task-count")
            let count = try heading.findAll(ViewType.Text.self).map { try $0.string() }
            #expect(count.contains(String(try rows().count)))
        }
        try checkCount()
        _ = try view.inspect().find(text: "Canceled")
        #expect(try rows() == ["LOO-309", "LOO-316", "LOO-312", "LOO-306", "reopened", "unresolved"])
        try view.inspect().find(viewWithAccessibilityIdentifier: "task-history-show").toggle().tap()
        model.taskHistoryNow = now
        #expect(try rows().contains("recent"))
        try checkCount()
        #expect(try !rows().contains("old"))
        try view.inspect().find(viewWithAccessibilityIdentifier: "task-history-days").textField().setInput("30")
        model.taskHistoryNow = now
        #expect(try rows().contains("old"))
        try checkCount()
        try view.inspect().find(viewWithAccessibilityIdentifier: "task-history-days").textField().setInput("0")
        #expect(model.taskHistoryFilters[wave.wave.id]?.days == 30)
        try view.inspect().find(viewWithAccessibilityIdentifier: "task-history-all").toggle().tap()
        #expect(try rows().contains("unknown"))
        try checkCount()
        _ = try view.inspect().find(text: "Completed · date unavailable")
        for id in [318, 315, 314, 313, 310, 308, 307] {
            #expect(try !rows().contains("LOO-\(id)"))
        }
        #expect(try !rows().contains("LOO-318"))
        #expect(model.taskHistoryFilters[wave.wave.id]?.showCompleted == true)
    }
}
#endif
