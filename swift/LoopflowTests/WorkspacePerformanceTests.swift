#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// Supporting algorithm measurements; no rendering or provider launch.
@Suite("Workspace projection measurements", .serialized)
struct WorkspacePerformanceTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["LF_WORKSPACE_PERF_OUTPUT"] != nil))
    func projection() throws {
        let output = try #require(ProcessInfo.processInfo.environment["LF_WORKSPACE_PERF_OUTPUT"])
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json"))
        var snapshot = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var wave = try #require((snapshot["waves"] as? [[String: Any]])?.first)
        var tasks = try #require(wave["tasks"] as? [String: Any])
        let template = try #require((tasks["items"] as? [[String: Any]])?.first)
        var observations: [[String: Any]] = []
        for count in [256, 2048] {
            tasks["items"] = try (0..<count).map { index in
                var task = template
                var planning = try #require(task["task"] as? [String: Any])
                planning["id"] = "task-\(index)"
                planning["rank"] = Double(index)
                task["task"] = planning
                var runtime = try #require(task["runtime"] as? [String: Any])
                runtime["work_id"] = "work-\(index)"
                task["runtime"] = runtime
                return task
            }
            wave["tasks"] = tasks
            snapshot["waves"] = [wave]
            let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self,
                from: JSONSerialization.data(withJSONObject: snapshot))
            let sessions = try (0..<count).map { index in
                try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: [
                    "id": "session-\(index)", "run_id": "run-\(index)", "interactive": true,
                    "work": ["kind": "task", "id": "work-\(index)"],
                    "title": "Session \(index)", "detail": "fixture", "cwd": "/src/loopflow",
                    "wave_id": "wave-1", "state": "active", "ready_summary": NSNull(),
                    "work_path": NSNull(), "title_source": "generated", "task_primary": false,
                    "actions": sessionActionFixture(state: "active"),
                    "flow_membership": ["kind": "independent"],
                    "task_ids": ["work-\(index)"], "agent_process_id": "44444444-4444-4444-8444-444444444444", "terminal_ids": [], "open_argv": ["must-not-launch"],
                ]))
            }
            for attempt in 0..<21 {
                let start = ContinuousClock.now
                let projection = WorkProjection(roadmaps: roadmap.waves, sessions: sessions)
                let elapsed = start.duration(to: .now)
                let milliseconds = Double(elapsed.components.seconds) * 1000
                    + Double(elapsed.components.attoseconds) / 1e15
                #expect(projection.waves.flatMap(\.tasks).flatMap(\.sessions).map(\.id) == sessions.map(\.id))
                #expect(projection.unmatchedSessions.isEmpty)
                observations.append(["tasks": count, "sessions": count, "attempt": attempt,
                                     "duration_ms": milliseconds])
            }
        }
        try JSONSerialization.data(withJSONObject: observations, options: [.prettyPrinted, .sortedKeys])
            .write(to: URL(fileURLWithPath: output))
    }
}
#endif
