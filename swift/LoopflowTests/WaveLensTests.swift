import Foundation
import Testing
@testable import Loopflow

@Suite("Wave Lens")
struct WaveLensTests {
    // MARK: - Wave row (list runtime projection)

    @Test("green when a body is live")
    func greenWhenLive() {
        let lens = WaveLens.forWave(live: true, activeTasks: 0, activeProjects: 0)
        #expect(lens.color == .green)
        #expect(lens.reason.contains("listener answered"))
    }

    @Test("a verified Run without a listener is red, not invented green")
    func activeRunWithoutListenerIsRed() {
        let lens = WaveLens.forWave(
            live: false,
            current: current(.working, reason: "Run process answered"),
            activeTasks: 0,
            activeProjects: 0
        )
        #expect(lens.color == .red)
        #expect(lens.reason == "Run process answered")
    }

    @Test("a live listener cannot erase canonically stopped Work")
    func liveListenerPreservesStoppedWork() {
        let lens = WaveLens.forWave(
            live: true,
            current: current(.stopped, reason: "the owning Home proved the Run is gone"),
            activeTasks: 0,
            activeProjects: 0
        )
        #expect(lens.color == .red)
        #expect(lens.reason == "the owning Home proved the Run is gone · Wave listener still answered")
    }

    @Test("a live listener cannot make unobservable Work green")
    func liveListenerPreservesUnobservableWork() {
        let lens = WaveLens.forWave(
            live: true,
            current: current(.unobservable, reason: "the owning Home could not verify the Run"),
            activeTasks: 0,
            activeProjects: 0
        )
        #expect(lens.color == .unknown)
        #expect(lens.reason == "the owning Home could not verify the Run · Wave listener answered")
    }

    @Test("a recorded running status is not live evidence")
    func recordedRunningStatusIsNotLiveEvidence() {
        let lens = WaveLens.forWave(
            live: false,
            current: nil,
            activeTasks: 0,
            activeProjects: 0
        )
        #expect(lens.color == .red)
        #expect(lens.reason == "Expected live · Wave listener did not answer")
    }

    @Test("red when stopped with outstanding work")
    func redWhenOutstanding() {
        let lens = WaveLens.forWave(live: false, activeTasks: 2, activeProjects: 1)
        #expect(lens.color == .red)
        #expect(lens.reason.contains("3"))
    }

    @Test("black only when disabled")
    func disabledIsBlack() {
        let lens = WaveLens.forWave(
            live: false,
            enabled: false,
            activeTasks: 0,
            activeProjects: 0
        )
        #expect(lens.color == .black)
        #expect(lens.reason == "Disabled on this Home")
        #expect(!lens.color.isLit)
    }

    @Test("retired Wave renders history rather than disabled current state")
    func retiredWaveIsHistorical() {
        let wave = Wave(
            id: "wave_old",
            name: "infra",
            repo: "/tmp/old",
            status: .abandoned,
            retiredAt: "2026-08-20T12:00:00Z",
            supersededByWaveId: "wave_current"
        )
        let lens = WaveViewModel(api: wave).lens

        #expect(lens.color == .black)
        #expect(lens.reason == "Retired at 2026-08-20T12:00:00Z · superseded by wave_current")
    }

    @Test("default-on without a listener is red")
    func runningWithoutListenerIsRed() {
        let lens = WaveLens.forWave(
            live: false,
            activeTasks: 0,
            activeProjects: 0
        )
        #expect(lens.color == .red)
        #expect(lens.reason == "Expected live · Wave listener did not answer")
    }

    @Test("paused turn intent is blue while listener evidence stays explicit")
    func pausedIsBlueWithListenerEvidence() {
        let serving = WaveLens.forWave(
            live: true,
            paused: true,
            activeTasks: 1,
            activeProjects: 1
        )
        #expect(serving.color == .blue)
        #expect(serving.reason == "Paused · listener is serving and queueing input")

        let stopped = WaveLens.forWave(
            live: false,
            paused: true,
            activeTasks: 0,
            activeProjects: 0
        )
        #expect(stopped.color == .blue)
        #expect(stopped.reason == "Paused · listener is stopped")
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

    @Test("a User Ask is blue and wins over a live Project body")
    func projectBlueWinsOverLiveBody() throws {
        let lens = WaveLens.forProject(
            runtime: try makeRuntime(alive: true, reason: "implementing"),
            tasks: [try makeTask(level: "blue", reason: "Waiting for your answer")]
        )
        #expect(lens.color == .blue)
        #expect(lens.reason == "Waiting for your answer")
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
        let attention = try makeAttention(level: "red", reason: "merge head abc1234 on GitHub")
        let lens = WaveLens.forTask(attention)
        #expect(lens.color == .red)
        #expect(lens.reason == "merge head abc1234 on GitHub")
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
            runtime: try makeRuntime(alive: true, reason: "implementing the projection"),
            tasks: [try makeTask(level: "green", reason: "advancing")]
        )
        #expect(lens.color == .green)
        #expect(lens.reason == "implementing the projection")
    }

    @Test("a task needing attention wins over a live project body")
    func projectRedWinsOverLiveBody() throws {
        let lens = WaveLens.forProject(
            runtime: try makeRuntime(alive: true, reason: "implementing"),
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
                activeTasks: 0,
                activeProjects: 0
            ),
            WaveLens.forWave(live: false, activeTasks: 1, activeProjects: 0),
            WaveLens.forWave(live: false, activeTasks: 0, activeProjects: 0),
            WaveLens.forTask(try makeAttention(level: "unknown", reason: "unread")),
            WaveLens.forProject(runtime: nil, tasks: []),
        ]
        for lens in lenses {
            #expect(!lens.reason.isEmpty)
        }
    }

    // MARK: - Fixtures

    private func current(
        _ state: CurrentWorkState,
        reason: String
    ) -> CurrentWorkObservation {
        CurrentWorkObservation(
            state: state,
            reason: reason,
            owner: .work,
            controls: [],
            progressAgeSeconds: 0,
            deadlineInSeconds: 30,
            step: nil,
            liveness: RunLivenessEvidence(
                state: .present,
                observedAt: "2026-07-15T00:00:00Z",
                fresh: true
            )
        )
    }

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
        {"level":"\(level)","reason":"\(reason)","observed_at":"2026-07-15T00:00:00Z","evidence_age_secs":null,"next_owner":"task","actions":{"recommended":null,"reason":"Task is ready to start"},"pm_completed":false,"work_status":null,"process":{"state":"not_applicable","alive":null,"reason":null},"local_progress":{"state":"not_applicable","unsettled":false,"dirty":null,"authored_commits":null,"recovery_required":null,"reason":null},"active_pr_phase":null}
        """
        return try JSONDecoder().decode(TaskAttentionSnapshot.self, from: Data(json.utf8))
    }

    private func makeTask(level: String, reason: String) throws -> WaveTaskWork {
        let json = """
        {"task":{"id":"\(reason)","identifier":"W2-1","name":"n","description":"","rank":1,"completed":false,"assignee":null},
        "reference":{"issue_url":null,"workspace":null},"runtime":null,"directive":null,
        "next_move":{"owner":"task","reason":"\(reason)"},
        "attention":{"level":"\(level)","reason":"\(reason)","observed_at":"2026-07-15T00:00:00Z","evidence_age_secs":null,"next_owner":"task","actions":{"recommended":null,"reason":"Task is ready to start"},"pm_completed":false,"work_status":null,"process":{"state":"not_applicable","alive":null,"reason":null},"local_progress":{"state":"not_applicable","unsettled":false,"dirty":null,"authored_commits":null,"recovery_required":null,"reason":null},"active_pr_phase":null},
        "prs":[],"active_pr":null}
        """
        return try JSONDecoder().decode(WaveTaskWork.self, from: Data(json.utf8))
    }

    private func makeRuntime(alive: Bool, reason: String) throws -> ProjectRuntimeSnapshot {
        let json = """
        {"work_id":"project-1","status":{"running":{"run_id":"run_00000000000000000000000000000006"}},"reason":"\(reason)","updated_at":"2026-07-15T00:00:00Z","iteration":1,"pending_observations":0,"provider":"codex","current":{"state":"\(alive ? "working" : "stopped")","reason":"\(reason)","owner":"\(alive ? "work" : "loopflow")","controls":\(alive ? "[\"attach\",\"steer\",\"interrupt\",\"stop\"]" : "[\"resume\",\"stop\"]"),"progress_age_secs":\(alive ? "60" : "null"),"deadline_in_secs":\(alive ? "1740" : "null"),"step":"iteration 1","liveness":{"state":"\(alive ? "present" : "absent")","observed_at":"2026-07-15T00:00:00Z","fresh":true}},"last_failure":null}
        """
        return try JSONDecoder().decode(ProjectRuntimeSnapshot.self, from: Data(json.utf8))
    }
}
