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
        let outputURL = URL(fileURLWithPath: output)
        try Data("[]".utf8).write(to: outputURL, options: .withoutOverwriting)
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json"))
        var snapshot = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let wave = try #require((snapshot["waves"] as? [[String: Any]])?.first)
        var tasks = try #require(snapshot["tasks"] as? [String: Any])
        let template = try #require((tasks["items"] as? [[String: Any]])?.first)
        var observations: [[String: Any]] = []
        for count in [256, 2048, 10_000] {
            tasks["items"] = try (0..<count).map { index in
                var task = template
                var planning = try #require(task["task"] as? [String: Any])
                planning["id"] = "task-\(index)"
                planning["rank"] = Double(index)
                task["task"] = planning
                var runtime = try #require(task["runtime"] as? [String: Any])
                runtime["work_id"] = "task-\(index)"
                task["runtime"] = runtime
                return task
            }
            snapshot["tasks"] = tasks
            snapshot["waves"] = [wave]
            let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self,
                from: JSONSerialization.data(withJSONObject: snapshot))
            let sessions = try (0..<count).map { index in
                try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: [
                    "id": "session-\(index)", "run_id": "run-\(index)", "interactive": true,
                    "work": ["kind": "task", "id": "task-\(index)"],
                    "title": "Session \(index)", "detail": "fixture", "cwd": "/src/loopflow",
                    "wave_id": "wave-1", "state": "active",
                    "work_path": NSNull(), "blocked": false, "title_source": "generated", "ended": NSNull(), "start": NSNull(), "outlook": NSNull(), "status_line": NSNull(),
                    "action": sessionActionFixture(state: "active"),
                    "graph_membership": ["kind": "independent"],
                    "task_ids": ["task-\(index)"], "agent_process_id": "44444444-4444-4444-8444-444444444444", "turn": NSNull(), "raw": false, "terminal_ids": [], "open_argv": ["must-not-launch"],
                ]))
            }
            for attempt in 0..<21 {
                let start = ContinuousClock.now
                let projection = WorkProjection(roadmaps: roadmap.waves, inventory: roadmap.tasks.items, sessions: sessions)
                let elapsed = start.duration(to: .now)
                let milliseconds = Double(elapsed.components.seconds) * 1000
                    + Double(elapsed.components.attoseconds) / 1e15
                #expect(projection.waves.flatMap(\.tasks).flatMap(\.sessions).map(\.id) == sessions.map(\.id))
                #expect(projection.unmatchedSessions.isEmpty)
                observations.append(["tasks": count, "sessions": count, "attempt": attempt,
                                     "duration_ms": milliseconds,
                                     "state": attempt == 0 ? "first_projection" : "warm",
                                     "endpoint": "projection_cpu_wall_only_no_native_views",
                                     "fixture": "roadmap_snapshot.json synthetic unique Task/Work/Session IDs",
                                     "build": "SwiftPM test configuration; record optimization in run metadata"])
                // Checkpoint outside the measured interval so an interrupted
                // large population retains its earlier observations.
                try JSONSerialization.data(withJSONObject: observations, options: [.prettyPrinted, .sortedKeys])
                    .write(to: outputURL, options: .atomic)
            }
        }
    }
}
#endif
