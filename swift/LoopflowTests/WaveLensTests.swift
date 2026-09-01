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

    @Test("listener absence is not invented as live evidence")
    func absentListenerIsNotLiveEvidence() {
        let lens = WaveLens.forWave(
            live: false,
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

    // MARK: - Task conditions map 1:1, and unknown is lit (never off/black)

    @Test("Task conditions map to lens colors one-to-one")
    func levelsMapOneToOne() {
        #expect(WaveLensColor(.blocked) == .red)
        #expect(WaveLensColor(.waiting) == .blue)
        #expect(WaveLensColor(.clear) == .black)
        #expect(WaveLensColor(.unknown) == .unknown)
    }

    @Test("a human Task step is blue and wins over Project planning state")
    func projectBlueWinsOverPlanningState() throws {
        let lens = WaveLens.forProject(
            tasks: [try makeTask(state: "waiting", reason: "Waiting for your answer")]
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

    // MARK: - Task row (shared condition, verbatim)

    @Test("task lens is the shared level and reason, verbatim")
    func taskLensIsVerbatim() throws {
        let condition = try makeCondition(
            state: "blocked",
            reason: "local Task progress requires recovery"
        )
        let lens = WaveLens.forTask(condition)
        #expect(lens.color == .red)
        #expect(lens.reason == "local Task progress requires recovery")
    }

    @Test("unavailable task evidence stays unknown with its reason, never black")
    func unavailableTaskIsUnknown() throws {
        let condition = try makeCondition(state: "unknown", reason: "failed to inspect Task worktree")
        let lens = WaveLens.forTask(condition)
        #expect(lens.color == .unknown)
        #expect(lens.reason == "failed to inspect Task worktree")
    }

    @Test("the shared W2-123 fixture maps every state verbatim")
    func sharedFixtureMapsVerbatim() throws {
        let tasks = try loadConditionFixture()
        for (_, task) in tasks {
            let lens = WaveLens.forTask(task.condition)
            #expect(lens.color == WaveLensColor(task.condition.state))
            #expect(lens.reason == task.condition.reason)
        }
        // The unavailable row is the one that must not collapse to black.
        let unavailable = try #require(tasks["unavailable"])
        #expect(WaveLens.forTask(unavailable.condition).color == .unknown)
        // Off-and-clean rows are genuinely black — the fixture proves both.
        #expect(WaveLens.forTask(try #require(tasks["clean_backlog"]).condition).color == .black)
        #expect(WaveLens.forTask(try #require(tasks["completed"]).condition).color == .black)
    }

    // MARK: - Project row (derived from Task conditions)

    @Test("a red Task wins over a black sibling")
    func projectRedWinsOverBlackTask() throws {
        let lens = WaveLens.forProject(
            tasks: [
                try makeTask(state: "clear", reason: "ready"),
                try makeTask(state: "blocked", reason: "awaiting review"),
            ]
        )
        #expect(lens.color == .red)
        #expect(lens.reason == "awaiting review")
    }

    @Test("the most demanding Task condition wins")
    func projectFoldsTaskCondition() throws {
        let redOverBlack = WaveLens.forProject(tasks: [
            try makeTask(state: "clear", reason: "ready"),
            try makeTask(state: "blocked", reason: "stuck"),
        ])
        #expect(redOverBlack.color == .red)
    }

    @Test("unreadable task evidence surfaces as unknown, not a silent black")
    func projectUnknownNeverBlack() throws {
        let lens = WaveLens.forProject(tasks: [
            try makeTask(state: "clear", reason: "done"),
            try makeTask(state: "unknown", reason: "failed to inspect Task worktree"),
        ])
        #expect(lens.color == .unknown)
        #expect(lens.reason == "failed to inspect Task worktree")
    }

    @Test("a project with only clean tasks is genuinely black")
    func projectAllCleanIsBlack() throws {
        let lens = WaveLens.forProject(tasks: [
            try makeTask(state: "clear", reason: "Linear Task is complete"),
        ])
        #expect(lens.color == .black)
        #expect(lens.reason == "Linear Task is complete")
    }

    @Test("a project with no runtime and no tasks is off")
    func projectEmptyIsBlack() {
        let lens = WaveLens.forProject(tasks: [])
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
            WaveLens.forTask(try makeCondition(state: "unknown", reason: "unread")),
            WaveLens.forProject(tasks: []),
        ]
        for lens in lenses {
            #expect(!lens.reason.isEmpty)
        }
    }

    // MARK: - Fixtures

    private func loadConditionFixture(sourceFile: String = #filePath) throws -> [String: RoadmapTask] {
        let fixture = URL(fileURLWithPath: sourceFile)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/task_condition_states.json")
        return try JSONDecoder().decode(
            [String: RoadmapTask].self,
            from: Data(contentsOf: fixture)
        )
    }

    private func makeCondition(state: String, reason: String) throws -> TaskConditionSnapshot {
        let json = """
        {"state":"\(state)","reason":"\(reason)","observed_at":"2026-07-15T00:00:00Z","evidence_age_secs":null,"local_progress":{"state":"not_applicable","unsettled":false,"dirty":null,"authored_commits":null,"recovery_required":null,"reason":null}}
        """
        return try JSONDecoder().decode(TaskConditionSnapshot.self, from: Data(json.utf8))
    }

    private func makeTask(state: String, reason: String) throws -> WaveTaskWork {
        let json = """
        {"task":{"id":"\(reason)","identifier":"W2-1","name":"n","description":"","rank":1,"completed":false,"assignee":null},
        "reference":{"issue_url":null,"workspace":null},"runtime":null,"directive":null,
        "next_move":{"owner":"task","reason":"\(reason)"},
        "condition":{"state":"\(state)","reason":"\(reason)","observed_at":"2026-07-15T00:00:00Z","evidence_age_secs":null,"local_progress":{"state":"not_applicable","unsettled":false,"dirty":null,"authored_commits":null,"recovery_required":null,"reason":null}},
        "actions":{"recommended":null,"reason":"Task is ready to start"},
        "prs":[],"active_pr":null}
        """
        return try JSONDecoder().decode(WaveTaskWork.self, from: Data(json.utf8))
    }

}
