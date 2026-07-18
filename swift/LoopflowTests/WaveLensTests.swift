import Foundation
import Testing
@testable import Loopflow

@Suite("Wave Lens")
struct WaveLensTests {
    // MARK: - Wave row (list runtime projection)

    @Test("green when a body is live")
    func greenWhenLive() {
        let lens = WaveLens.forWave(live: true, status: .ready, activeTasks: 0, activeProjects: 0)
        #expect(lens.color == .green)
        #expect(lens.reason.contains("Running"))
    }

    @Test("green when running even without an explicit live flag")
    func greenWhenRunning() {
        let lens = WaveLens.forWave(
            live: false,
            status: .running(runID: "run_test"),
            activeTasks: 0,
            activeProjects: 0
        )
        #expect(lens.color == .green)
    }

    @Test("red when stopped with outstanding work")
    func redWhenOutstanding() {
        let lens = WaveLens.forWave(live: false, status: .ready, activeTasks: 2, activeProjects: 1)
        #expect(lens.color == .red)
        #expect(lens.reason.contains("3"))
    }

    @Test("black when off and clean")
    func blackWhenClean() {
        let lens = WaveLens.forWave(live: false, status: .ready, activeTasks: 0, activeProjects: 0)
        #expect(lens.color == .black)
        #expect(!lens.color.isLit)
    }

    // MARK: - The shared level maps 1:1, and unknown is lit (never off/black)

    @Test("attention levels map to lens colors one-to-one")
    func levelsMapOneToOne() {
        #expect(WaveLensColor(.green) == .green)
        #expect(WaveLensColor(.red) == .red)
        #expect(WaveLensColor(.blue) == .blue)
        #expect(WaveLensColor(.black) == .black)
        #expect(WaveLensColor(.unknown) == .unknown)
    }

    @Test("a User Feedback is blue and wins over a live Project body")
    func projectBlueWinsOverLiveBody() throws {
        let lens = WaveLens.forProject(
            runtime: try makeRuntime(status: "running", alive: true, reason: "implementing"),
            tasks: [try makeTask(level: "blue", reason: "Waiting for your feedback")]
        )
        #expect(lens.color == .blue)
        #expect(lens.reason == "Waiting for your feedback")
    }

    @Test("unknown is a lit lens, distinct from the off black")
    func unknownIsLit() {
        #expect(WaveLensColor.unknown.isLit)
        #expect(!WaveLensColor.black.isLit)
        #expect(WaveLensColor.unknown.glow != WaveLensColor.black.glow)
    }

    // MARK: - Task row (shared attention, verbatim)

    @Test("task lens is the shared level and reason, verbatim")
    func taskLensIsVerbatim() throws {
        let attention = try makeAttention(level: "red", reason: "checks passed; awaiting review")
        let lens = WaveLens.forTask(attention)
        #expect(lens.color == .red)
        #expect(lens.reason == "checks passed; awaiting review")
    }

    @Test("unavailable task evidence stays unknown with its reason, never black")
    func unavailableTaskIsUnknown() throws {
        let attention = try makeAttention(level: "unknown", reason: "failed to inspect Task worktree")
        let lens = WaveLens.forTask(attention)
        #expect(lens.color == .unknown)
        #expect(lens.reason == "failed to inspect Task worktree")
    }

    @Test("the shared W2-123 fixture maps every state verbatim")
    func sharedFixtureMapsVerbatim() throws {
        let tasks = try loadAttentionFixture()
        for (_, task) in tasks {
            let lens = WaveLens.forTask(task.attention)
            #expect(lens.color == WaveLensColor(task.attention.level))
            #expect(lens.reason == task.attention.reason)
        }
        // The unavailable row is the one that must not collapse to black.
        let unavailable = try #require(tasks["unavailable"])
        #expect(WaveLens.forTask(unavailable.attention).color == .unknown)
        // Off-and-clean rows are genuinely black — the fixture proves both.
        #expect(WaveLens.forTask(try #require(tasks["clean_backlog"]).attention).color == .black)
        #expect(WaveLens.forTask(try #require(tasks["completed"]).attention).color == .black)
        #expect(WaveLens.forTask(try #require(tasks["live_advancing"]).attention).color == .green)
    }

    // MARK: - Project row (derived from runtime + Task attention)

    @Test("a live project body advancing is green")
    func projectLiveIsGreen() throws {
        let lens = WaveLens.forProject(
            runtime: try makeRuntime(status: "running", alive: true, reason: "implementing the projection"),
            tasks: [try makeTask(level: "green", reason: "advancing")]
        )
        #expect(lens.color == .green)
        #expect(lens.reason == "implementing the projection")
    }

    @Test("a task needing attention wins over a live project body")
    func projectRedWinsOverLiveBody() throws {
        let lens = WaveLens.forProject(
            runtime: try makeRuntime(status: "running", alive: true, reason: "implementing"),
            tasks: [
                try makeTask(level: "green", reason: "advancing"),
                try makeTask(level: "red", reason: "awaiting review"),
            ]
        )
        #expect(lens.color == .red)
        #expect(lens.reason == "awaiting review")
    }

    @Test("with no live body the most demanding task attention wins")
    func projectFoldsTaskAttention() throws {
        let redOverGreen = WaveLens.forProject(runtime: nil, tasks: [
            try makeTask(level: "green", reason: "advancing"),
            try makeTask(level: "red", reason: "stuck"),
        ])
        #expect(redOverGreen.color == .red)

        let greenOverBlack = WaveLens.forProject(runtime: nil, tasks: [
            try makeTask(level: "black", reason: "done"),
            try makeTask(level: "green", reason: "advancing"),
        ])
        #expect(greenOverBlack.color == .green)
    }

    @Test("unreadable task evidence surfaces as unknown, not a silent black")
    func projectUnknownNeverBlack() throws {
        let lens = WaveLens.forProject(runtime: nil, tasks: [
            try makeTask(level: "black", reason: "done"),
            try makeTask(level: "unknown", reason: "failed to inspect Task worktree"),
        ])
        #expect(lens.color == .unknown)
        #expect(lens.reason == "failed to inspect Task worktree")
    }

    @Test("a project with only clean tasks is genuinely black")
    func projectAllCleanIsBlack() throws {
        let lens = WaveLens.forProject(runtime: nil, tasks: [
            try makeTask(level: "black", reason: "Linear Task is complete"),
        ])
        #expect(lens.color == .black)
        #expect(lens.reason == "Linear Task is complete")
    }

    @Test("a project with no runtime and no tasks is off")
    func projectEmptyIsBlack() {
        let lens = WaveLens.forProject(runtime: nil, tasks: [])
        #expect(lens.color == .black)
        #expect(!lens.reason.isEmpty)
    }

    // MARK: - Every lens speaks a reason

    @Test("every lens carries a reason for VoiceOver")
    func everyLensHasReason() throws {
        let lenses = [
            WaveLens.forWave(
                live: true,
                status: .running(runID: "run_test"),
                activeTasks: 0,
                activeProjects: 0
            ),
            WaveLens.forWave(live: false, status: .ready, activeTasks: 1, activeProjects: 0),
            WaveLens.forWave(live: false, status: .ready, activeTasks: 0, activeProjects: 0),
            WaveLens.forTask(try makeAttention(level: "unknown", reason: "unread")),
            WaveLens.forProject(runtime: nil, tasks: []),
        ]
        for lens in lenses {
            #expect(!lens.reason.isEmpty)
        }
    }

    // MARK: - Fixtures

    private func loadAttentionFixture(sourceFile: String = #filePath) throws -> [String: RoadmapTask] {
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

    private func makeAttention(level: String, reason: String) throws -> TaskAttentionSnapshot {
        let json = """
        {"level":"\(level)","reason":"\(reason)","observed_at":"2026-07-15T00:00:00Z","evidence_age_secs":null,"next_owner":"task","actions":{"recommended":null,"actions":[{"action":"recover","available":false,"reason":"no Task Session; start one with `lf task run`"},{"action":"resume","available":false,"reason":"no Task Session; start one with `lf task run`"},{"action":"review","available":false,"reason":"no Task Session; start one with `lf task run`"},{"action":"start_next_pr","available":false,"reason":"no Task Session; start one with `lf task run`"},{"action":"complete","available":false,"reason":"no Task Session; start one with `lf task run`"},{"action":"no_action","available":false,"reason":"no Task Session; start one with `lf task run`"}]},"pm_completed":false,"session_status":null,"process":{"state":"not_applicable","alive":null,"reason":null},"local_progress":{"state":"not_applicable","unsettled":false,"dirty":null,"authored_commits":null,"recovery_required":null,"reason":null},"active_pr_phase":null}
        """
        return try JSONDecoder().decode(TaskAttentionSnapshot.self, from: Data(json.utf8))
    }

    private func makeTask(level: String, reason: String) throws -> WaveTaskWork {
        let json = """
        {"task":{"id":"\(reason)","identifier":"W2-1","name":"n","description":"","rank":1,"completed":false,"assignee":null},
        "reference":{"issue_url":null,"workspace":null},"runtime":null,"directive":null,
        "next_move":{"owner":"task","reason":"\(reason)"},
        "attention":{"level":"\(level)","reason":"\(reason)","observed_at":"2026-07-15T00:00:00Z","evidence_age_secs":null,"next_owner":"task","actions":{"recommended":null,"actions":[{"action":"recover","available":false,"reason":"no Task Session; start one with `lf task run`"},{"action":"resume","available":false,"reason":"no Task Session; start one with `lf task run`"},{"action":"review","available":false,"reason":"no Task Session; start one with `lf task run`"},{"action":"start_next_pr","available":false,"reason":"no Task Session; start one with `lf task run`"},{"action":"complete","available":false,"reason":"no Task Session; start one with `lf task run`"},{"action":"no_action","available":false,"reason":"no Task Session; start one with `lf task run`"}]},"pm_completed":false,"session_status":null,"process":{"state":"not_applicable","alive":null,"reason":null},"local_progress":{"state":"not_applicable","unsettled":false,"dirty":null,"authored_commits":null,"recovery_required":null,"reason":null},"active_pr_phase":null},
        "prs":[],"active_pr":null}
        """
        return try JSONDecoder().decode(WaveTaskWork.self, from: Data(json.utf8))
    }

    private func makeRuntime(status: String, alive: Bool, reason: String) throws -> ProjectRuntimeSnapshot {
        let json = """
        {"session_id":"ps_1","status":"\(status)","reason":"\(reason)","status_at":"2026-07-15T00:00:00Z","iteration":1,"pending_observations":0,"provider":"codex","process_alive":\(alive),"observation":{"category":"working","reason":"\(reason)","owner":"session","controls":["attach","steer","interrupt","stop"],"progress_age_secs":60,"deadline_in_secs":1740,"step":"iteration 1"}}
        """
        return try JSONDecoder().decode(ProjectRuntimeSnapshot.self, from: Data(json.utf8))
    }
}
