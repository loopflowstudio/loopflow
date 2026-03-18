// Tests for WaveViewModel and Trigger struct.

import Foundation
import SwiftUI
import Testing
@testable import Concerto
@testable import LoopflowCore

@Suite("Wave View Model")
struct WaveModelTests {
    private func makeWave(
        id: String = "test-id",
        name: String = "",
        repo: String = "/tmp/repo",
        flow: String = "design",
        direction: [String] = [],
        area: [String] = [],
        triggers: [Trigger] = [],
        status: WaveStatus = .idle,
        iteration: Int = 0,
        recentSteps: [StepRun] = [],
        waitingReason: WaitingReason? = nil
    ) -> WaveViewModel {
        WaveViewModel(
            api: Wave(
                id: id,
                name: name,
                repo: repo,
                flow: flow,
                direction: direction,
                area: area,
                triggers: triggers,
                status: status,
                iteration: iteration
            ),
            recentSteps: recentSteps,
            waitingReason: waitingReason
        )
    }

    // MARK: - Display Name

    @Test("displayName uses name when set")
    func displayNameUsesName() {
        let wave = makeWave(name: "swift-falcon")

        #expect(wave.displayName == "swift-falcon")
    }

    @Test("displayName generates from area and flow when name is empty")
    func displayNameGeneratesFromConfig() {
        let wave = makeWave(flow: "build", area: ["src/auth"])

        #expect(wave.displayName == "src/auth · build")
    }

    @Test("displayName shows 'root' for dot area")
    func displayNameRootForDotArea() {
        let wave = makeWave(flow: "debug", area: ["."])

        #expect(wave.displayName == "root · debug")
    }

    @Test("displayName shows default flow when empty")
    func displayNameDefaultFlow() {
        let wave = makeWave(flow: "", area: [])

        #expect(wave.displayName == "root · default")
    }

    // MARK: - Status Indicator

    @Test("statusIndicator returns forest green circle for running")
    func statusIndicatorRunning() {
        let wave = makeWave(id: "test", repo: "/tmp", status: .running)
        let indicator = wave.statusIndicator

        #expect(indicator.icon == "circle.fill")
        #expect(indicator.color == .statusSuccess)
    }

    @Test("statusIndicator returns goldenrod half-circle for waiting")
    func statusIndicatorWaiting() {
        let wave = makeWave(id: "test", repo: "/tmp", status: .waiting)
        let indicator = wave.statusIndicator

        #expect(indicator.icon == "circle.lefthalf.filled")
        #expect(indicator.color == .statusWarning)
    }

    @Test("statusIndicator returns neutral circle for idle")
    func statusIndicatorIdle() {
        let wave = makeWave(id: "test", repo: "/tmp", status: .idle)
        let indicator = wave.statusIndicator

        #expect(indicator.icon == "circle")
        #expect(indicator.color == .statusNeutral)
    }

    @Test("statusIndicator returns burnt orange X for failed")
    func statusIndicatorFailed() {
        let wave = makeWave(id: "test", repo: "/tmp", status: .failed)
        let indicator = wave.statusIndicator

        #expect(indicator.icon == "xmark.circle.fill")
        #expect(indicator.color == .statusError)
    }

    // MARK: - Computed Properties

    @Test("areaDisplay joins multiple areas")
    func areaDisplayJoins() {
        let wave = makeWave(id: "test", repo: "/tmp", area: ["src/", "lib/"])

        #expect(wave.areaDisplay == "src/, lib/")
    }

    @Test("areaDisplay returns dot for root area")
    func areaDisplayDot() {
        let wave = makeWave(id: "test", repo: "/tmp", area: ["."])

        #expect(wave.areaDisplay == ".")
    }

    @Test("areaDisplay returns empty for nil area")
    func areaDisplayNil() {
        let wave = makeWave(id: "test", repo: "/tmp", area: [])

        #expect(wave.areaDisplay == "")
    }

    @Test("directionDisplay joins multiple goals")
    func directionDisplayJoins() {
        let wave = makeWave(
            id: "test",
            repo: "/tmp",
            direction: ["clarity", "ux"]
        )

        #expect(wave.directionDisplay == "clarity, ux")
    }

    @Test("directionDisplay returns empty for nil direction")
    func directionDisplayNil() {
        let wave = makeWave(id: "test", repo: "/tmp", direction: [])

        #expect(wave.directionDisplay == "")
    }

    @Test("worktree and branch fall back to wave when no active run")
    func worktreeBranchFallbackToWave() {
        let model = WaveViewModel(
            api: Wave(
                id: "test",
                name: "test",
                repo: "/tmp/repo",
                flow: "design",
                direction: [],
                area: [],
                status: .idle,
                iteration: 0,
                localWorktree: "/tmp/repo.test",
                remoteBranch: "jack/test"
            )
        )

        #expect(model.worktreePath == "/tmp/repo.test")
        #expect(model.branch == "jack/test")
    }

    @Test("shortId returns first 7 characters")
    func shortIdTruncates() {
        let wave = makeWave(id: "abcdefghijklmnop", repo: "/tmp")

        #expect(wave.shortId == "abcdefg")
    }

    // MARK: - Detail Text

    @Test("detailText combines area, flow, and trigger signal")
    func detailTextCombines() {
        let wave = makeWave(
            id: "test",
            repo: "/tmp",
            flow: "build",
            area: ["src/"],
            triggers: [Trigger(signal: .repo)]
        )

        #expect(wave.detailText == "src/ · build · repo")
    }

    @Test("detailText omits trigger when none active")
    func detailTextOmitsWhenNoTrigger() {
        let wave = makeWave(
            id: "test",
            repo: "/tmp",
            flow: "debug",
            area: ["."]
        )

        #expect(wave.detailText == ". · debug")
    }

    @Test("displayFlow prefers active run flow")
    func displayFlowPrefersActiveRunFlow() {
        let run = WaveRun(
            id: "run-1",
            waveId: "wave-1",
            flow: "design",
            area: ".",
            repo: "/tmp/repo"
        )
        let wave = WaveViewModel(
            api: Wave(
                id: "wave-1",
                repo: "/tmp/repo",
                flow: "ship-roadmap",
                area: ["."],
                status: .running,
                activeRun: run
            )
        )

        #expect(wave.displayFlow == "design")
        #expect(wave.detailText == ". · design")
    }

    @Test("displayFlowSteps collapse to active run flow when override is running")
    func displayFlowStepsPreferActiveRunOverride() {
        let run = WaveRun(
            id: "run-1",
            waveId: "wave-1",
            flow: "design",
            area: ".",
            repo: "/tmp/repo"
        )
        let wave = WaveViewModel(
            api: Wave(
                id: "wave-1",
                repo: "/tmp/repo",
                flow: "ship-roadmap",
                status: .running,
                flowSteps: ["ingest", "kickoff", "build"],
                activeRun: run
            )
        )

        #expect(wave.displayFlowSteps == ["design"])
    }

    @Test("displayFlowSteps ignore equivalent active run flow formatting")
    func displayFlowStepsIgnoreEquivalentRunFlowFormatting() {
        let run = WaveRun(
            id: "run-1",
            waveId: "wave-1",
            flow: " ship-roadmap ",
            area: ".",
            repo: "/tmp/repo"
        )
        let wave = WaveViewModel(
            api: Wave(
                id: "wave-1",
                repo: "/tmp/repo",
                flow: "ship-roadmap",
                status: .running,
                flowSteps: ["ingest", "kickoff", "build"],
                activeRun: run
            )
        )

        #expect(wave.runFlowOverride == nil)
        #expect(wave.displayFlow == "ship-roadmap")
        #expect(wave.displayFlowSteps == ["ingest", "kickoff", "build"])
    }

    // MARK: - Iteration Text

    @Test("iterationText shows iter count when positive")
    func iterationTextPositive() {
        let wave = makeWave(id: "test", repo: "/tmp", iteration: 5)

        #expect(wave.iterationText == "iter 5")
    }

    @Test("iterationText is empty when zero")
    func iterationTextZero() {
        let wave = makeWave(id: "test", repo: "/tmp", iteration: 0)

        #expect(wave.iterationText == "")
    }

    // MARK: - Activity Tracking

    @Test("lastActivityAt returns nil when no recent steps")
    func lastActivityAtNilWithNoSteps() {
        let wave = makeWave(id: "test", repo: "/tmp", recentSteps: [])

        #expect(wave.lastActivityAt == nil)
    }

    @Test("lastActivityAt returns endedAt when present")
    func lastActivityAtUsesEndedAt() {
        let startDate = Date().addingTimeInterval(-120)
        let endDate = Date().addingTimeInterval(-60)
        let step = StepRun(
            id: "step-1",
            step: "implement",
            repo: "/tmp",
            worktree: "/tmp/wt",
            status: "completed",
            startedAt: startDate,
            endedAt: endDate,
            agent: "claude",
            runMode: "auto"
        )
        let wave = makeWave(id: "test", repo: "/tmp", recentSteps: [step])

        #expect(wave.lastActivityAt == endDate)
    }

    @Test("lastActivityAt falls back to startedAt when endedAt is nil")
    func lastActivityAtFallsBackToStartedAt() {
        let startDate = Date().addingTimeInterval(-120)
        let step = StepRun(
            id: "step-1",
            step: "implement",
            repo: "/tmp",
            worktree: "/tmp/wt",
            status: "running",
            startedAt: startDate,
            endedAt: nil,
            agent: "claude",
            runMode: "auto"
        )
        let wave = makeWave(id: "test", repo: "/tmp", recentSteps: [step])

        #expect(wave.lastActivityAt == startDate)
    }

    @Test("lastActivityDescription returns nil when no recent steps")
    func lastActivityDescriptionNilWithNoSteps() {
        let wave = makeWave(id: "test", repo: "/tmp", recentSteps: [])

        #expect(wave.lastActivityDescription == nil)
    }

    @Test("lastActivityDescription includes step name")
    func lastActivityDescriptionIncludesStepName() {
        let step = StepRun(
            id: "step-1",
            step: "implement",
            repo: "/tmp",
            worktree: "/tmp/wt",
            status: "completed",
            startedAt: Date().addingTimeInterval(-60),
            endedAt: Date(),
            agent: "claude",
            runMode: "auto"
        )
        let wave = makeWave(id: "test", repo: "/tmp", recentSteps: [step])
        let description = wave.lastActivityDescription

        #expect(description != nil)
        #expect(description!.hasPrefix("implement"))
    }
}

@Suite("WaitingReason")
struct WaitingReasonTests {

    private func makeViewModel(
        status: WaveStatus = .idle,
        waitingReason: WaitingReason? = nil
    ) -> WaveViewModel {
        WaveViewModel(
            api: Wave(id: "test", repo: "/tmp", status: status),
            waitingReason: waitingReason
        )
    }

    @Test("description shows count fraction")
    func descriptionShowsCountFraction() {
        let reason = WaitingReason.prLimitReached(open: 2, limit: 5)

        #expect(reason.description == "2/5 PRs open")
    }

    @Test("accessibilityDescription shows full text")
    func accessibilityDescriptionShowsFullText() {
        let reason = WaitingReason.prLimitReached(open: 3, limit: 5)

        #expect(reason.accessibilityDescription == "3 of 5 PRs open")
    }

    @Test("Wave with waitingReason stores it correctly")
    func waveStoresWaitingReason() {
        let wave = makeViewModel(
            status: .waiting,
            waitingReason: .prLimitReached(open: 2, limit: 3)
        )

        #expect(wave.waitingReason != nil)
        if case .prLimitReached(let open, let limit) = wave.waitingReason {
            #expect(open == 2)
            #expect(limit == 3)
        } else {
            Issue.record("Expected prLimitReached")
        }
    }

    @Test("Wave without waitingReason has nil")
    func waveWithoutWaitingReasonHasNil() {
        let wave = makeViewModel(status: .idle)

        #expect(wave.waitingReason == nil)
    }
}

@Suite("CombinePRsResult")
struct CombinePRsResultTests {

    @Test("initializes with URL and closed PRs")
    func initializesWithUrlAndClosedPRs() {
        let result = CombinePRsResult(
            newPRUrl: "https://github.com/owner/repo/pull/100",
            closedPRs: [1, 2, 3]
        )

        #expect(result.newPRUrl == "https://github.com/owner/repo/pull/100")
        #expect(result.closedPRs == [1, 2, 3])
    }

    @Test("initializes with nil URL")
    func initializesWithNilUrl() {
        let result = CombinePRsResult(newPRUrl: nil, closedPRs: [])

        #expect(result.newPRUrl == nil)
        #expect(result.closedPRs.isEmpty)
    }
}

@Suite("Flow step progress")
struct FlowStepProgressTests {

    @Test("stepIndex reflects active run step_index")
    func stepIndexFromActiveRun() {
        let run = WaveRun(
            id: "run-1",
            waveId: "wave-1",
            flow: "start",
            area: ".",
            repo: "/tmp/repo",
            stepIndex: 1
        )
        let wave = WaveViewModel(
            api: Wave(
                id: "wave-1",
                repo: "/tmp/repo",
                flow: "start",
                status: .running,
                flowSteps: ["ingest", "kickoff"],
                activeRun: run
            )
        )

        #expect(wave.stepIndex == 1)
        #expect(wave.flowSteps == ["ingest", "kickoff"])
    }

    @Test("stepIndex defaults to 0 when no active run")
    func stepIndexDefaultsToZero() {
        let wave = WaveViewModel(
            api: Wave(
                id: "wave-1",
                repo: "/tmp/repo",
                flow: "start",
                status: .idle,
                flowSteps: ["ingest", "kickoff"]
            )
        )

        #expect(wave.stepIndex == 0)
    }

    @Test("stepIndex advances when active run updates")
    func stepIndexAdvancesWithRun() {
        let run0 = WaveRun(
            id: "run-1",
            waveId: "wave-1",
            flow: "build",
            area: ".",
            repo: "/tmp/repo",
            stepIndex: 0
        )
        let run1 = WaveRun(
            id: "run-1",
            waveId: "wave-1",
            flow: "build",
            area: ".",
            repo: "/tmp/repo",
            stepIndex: 2
        )

        let wave0 = WaveViewModel(
            api: Wave(
                id: "wave-1",
                repo: "/tmp/repo",
                flow: "build",
                status: .running,
                flowSteps: ["implement", "compress", "gate", "update-wave"],
                activeRun: run0
            )
        )
        let wave1 = WaveViewModel(
            api: Wave(
                id: "wave-1",
                repo: "/tmp/repo",
                flow: "build",
                status: .running,
                flowSteps: ["implement", "compress", "gate", "update-wave"],
                activeRun: run1
            )
        )

        #expect(wave0.stepIndex == 0)
        #expect(wave1.stepIndex == 2)
    }
}

@Suite("WaveStore reactivity")
struct WaveStoreReactivityTests {

    @MainActor
    @Test("WaveStore.set updates stepIndex when wave refreshed with new active run")
    func storeReflectsUpdatedStepIndex() {
        let store = WaveStore()

        // Initial wave at step 0
        let run0 = WaveRun(
            id: "run-1", waveId: "wave-1", flow: "start", area: ".", repo: "/tmp/repo",
            stepIndex: 0
        )
        let wave0 = WaveViewModel(
            api: Wave(
                id: "wave-1", repo: "/tmp/repo", flow: "start", status: .running,
                flowSteps: ["ingest", "kickoff"], activeRun: run0
            )
        )
        store.set(wave0)

        #expect(store.wave(for: "wave-1")?.stepIndex == 0)

        // Simulate wave_updated event: re-fetch returns step 1
        let run1 = WaveRun(
            id: "run-1", waveId: "wave-1", flow: "start", area: ".", repo: "/tmp/repo",
            stepIndex: 1
        )
        let wave1 = WaveViewModel(
            api: Wave(
                id: "wave-1", repo: "/tmp/repo", flow: "start", status: .running,
                flowSteps: ["ingest", "kickoff"], activeRun: run1
            )
        )
        store.set(wave1)

        #expect(store.wave(for: "wave-1")?.stepIndex == 1)
        #expect(store.wave(for: "wave-1")?.flowSteps == ["ingest", "kickoff"])
    }
}

@Suite("parseWaveFromJSON")
struct ParseWaveFromJSONTests {

    @Test("parses active_run with step_index")
    func parsesActiveRunStepIndex() {
        let json: [String: Any] = [
            "id": "wave-1",
            "name": "ux",
            "repo": "/tmp/repo",
            "flow": "start",
            "direction": ["ux"],
            "area": ["."],
            "status": "running",
            "iteration": 0,
            "flow_steps": ["ingest", "kickoff"],
            "open_pr_count": 3,
            "active_run": [
                "id": "run-1",
                "wave_id": "wave-1",
                "flow": "start",
                "repo": "/tmp/repo",
                "direction": ["ux"],
                "area": ["."],
                "iteration": 0,
                "step_index": 1,
                "status": "running",
                "local_worktree": "/tmp/wt",
                "remote_branch": "jack/ux"
            ] as [String: Any]
        ]

        let wave = WaveService.parseWaveFromJSON(json)

        #expect(wave.flowSteps == ["ingest", "kickoff"])
        #expect(wave.openPRCount == 3)
        #expect(wave.activeRun?.stepIndex == 1)

        let vm = WaveViewModel(api: wave)
        #expect(vm.stepIndex == 1)
    }

    @Test("parses wave without active_run defaults stepIndex to 0")
    func parsesWaveWithoutActiveRun() {
        let json: [String: Any] = [
            "id": "wave-1",
            "name": "ux",
            "repo": "/tmp/repo",
            "flow": "start",
            "status": "idle",
            "flow_steps": ["ingest", "kickoff"],
            "open_pr_count": 0
        ]

        let wave = WaveService.parseWaveFromJSON(json)
        let vm = WaveViewModel(api: wave)

        #expect(vm.stepIndex == 0)
        #expect(wave.activeRun == nil)
    }
}

@Suite("WaveRun helpers")
struct WaveRunHelpersTests {
    @Test("duration formats minutes and seconds")
    func durationFormatsMinutesAndSeconds() {
        let start = Date(timeIntervalSince1970: 0)
        let end = Date(timeIntervalSince1970: 125)
        let run = WaveRun(
            id: "run-1",
            waveId: "wave-1",
            flow: "build",
            area: ".",
            repo: "/tmp/repo",
            startedAt: start,
            endedAt: end
        )

        #expect(run.duration == "2m05s")
    }

    @Test("relativeTime returns non-empty value")
    func relativeTimeNonEmpty() {
        let run = WaveRun(
            id: "run-1",
            waveId: "wave-1",
            flow: "build",
            area: ".",
            repo: "/tmp/repo",
            createdAt: Date().addingTimeInterval(-60)
        )

        #expect(!run.relativeTime.isEmpty)
    }
}

@Suite("Trigger")
struct TriggerTests {

    @Test("description returns signal name")
    func descriptionSignal() {
        #expect(Trigger(signal: .repo).description == "repo")
        #expect(Trigger(signal: .wave).description == "wave")
        #expect(Trigger(signal: .ciFailure).description == "ci_failure")
    }

    @Test("description includes flow when set")
    func descriptionWithFlow() {
        let trigger = Trigger(signal: .repo, flow: "integrate")

        #expect(trigger.description == "repo → integrate")
    }

    @Test("icon returns correct SF Symbol for each signal")
    func iconForSignal() {
        #expect(Trigger(signal: .repo).icon == "arrow.triangle.branch")
        #expect(Trigger(signal: .wave).icon == "waveform")
        #expect(Trigger(signal: .ciFailure).icon == "exclamationmark.triangle")
    }
}

@Suite("WaveStatus")
struct WaveStatusTests {

    @Test("color returns correct SwiftUI color")
    func colorForStatus() {
        #expect(WaveStatus.running.color == .statusSuccess)
        #expect(WaveStatus.waiting.color == .statusWarning)
        #expect(WaveStatus.idle.color == .statusNeutral)
        #expect(WaveStatus.failed.color == .statusError)
    }

    @Test("icon returns correct SF Symbol")
    func iconForStatus() {
        #expect(WaveStatus.running.icon == "circle.fill")
        #expect(WaveStatus.waiting.icon == "circle.lefthalf.filled")
        #expect(WaveStatus.idle.icon == "circle")
        #expect(WaveStatus.failed.icon == "xmark.circle.fill")
    }
}

@Suite("InteractiveSession")
struct InteractiveSessionTests {

    @Test("command returns step without prompt")
    func commandWithoutPrompt() {
        let session = InteractiveSession(
            waveId: "wave-1",
            step: "design",
            worktreePath: "/tmp/wt"
        )
        #expect(session.command == "lf design && lf ops commit --push")
    }

    @Test("command includes shell-escaped prompt")
    func commandWithPrompt() {
        let session = InteractiveSession(
            waveId: "wave-1",
            step: "design",
            worktreePath: "/tmp/wt",
            prompt: "add rate limiting"
        )
        #expect(session.command == "lf design 'add rate limiting' && lf ops commit --push")
    }

    @Test("command escapes single quotes in prompt")
    func commandEscapesSingleQuotes() {
        let session = InteractiveSession(
            waveId: "wave-1",
            step: "debug",
            worktreePath: "/tmp/wt",
            prompt: "fix the user's auth flow"
        )
        #expect(session.command == "lf debug 'fix the user'\\''s auth flow' && lf ops commit --push")
    }

    @Test("command handles special shell characters")
    func commandHandlesSpecialChars() {
        let session = InteractiveSession(
            waveId: "wave-1",
            step: "implement",
            worktreePath: "/tmp/wt",
            prompt: "add $HOME expansion & pipes | redirects > /dev/null"
        )
        // Single quotes protect all special characters except single quotes themselves
        #expect(session.command == "lf implement 'add $HOME expansion & pipes | redirects > /dev/null' && lf ops commit --push")
    }
}

@Suite("Shell Escape")
struct ShellEscapeTests {

    @Test("shellEscape wraps in single quotes")
    func wrapsInSingleQuotes() {
        #expect(shellEscape("hello") == "'hello'")
    }

    @Test("shellEscape escapes internal single quotes")
    func escapesInternalQuotes() {
        #expect(shellEscape("it's") == "'it'\\''s'")
    }

    @Test("shellEscape handles multiple single quotes")
    func handlesMultipleSingleQuotes() {
        // Input: 'a' and 'b'
        // Expected: ''\'a'\'' and '\''b'\''
        // Each ' becomes '\'' (end quote, escaped quote, start quote)
        #expect(shellEscape("'a' and 'b'") == "''\\''a'\\'' and '\\''b'\\'''")
    }

    @Test("shellEscape preserves special characters")
    func preservesSpecialChars() {
        #expect(shellEscape("$HOME & | > < ; `") == "'$HOME & | > < ; `'")
    }

    @Test("shellEscape handles empty string")
    func handlesEmptyString() {
        #expect(shellEscape("") == "''")
    }
}
