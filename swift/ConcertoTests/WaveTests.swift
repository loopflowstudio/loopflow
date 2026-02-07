// Tests for WaveViewModel and Stimulus struct.

import Foundation
import SwiftUI
import Testing
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
        stimulus: Stimulus = Stimulus(kind: .once),
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
                stimulus: stimulus,
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
        let wave = makeWave(area: ["src/auth"], flow: "ship")

        #expect(wave.displayName == "src/auth · ship")
    }

    @Test("displayName shows 'root' for dot area")
    func displayNameRootForDotArea() {
        let wave = makeWave(area: ["."], flow: "debug")

        #expect(wave.displayName == "root · debug")
    }

    @Test("displayName shows default flow when empty")
    func displayNameDefaultFlow() {
        let wave = makeWave(area: [], flow: "")

        #expect(wave.displayName == "root · default")
    }

    // MARK: - Status Indicator

    @Test("statusIndicator returns green circle for running")
    func statusIndicatorRunning() {
        let wave = makeWave(id: "test", repo: "/tmp", status: .running)
        let indicator = wave.statusIndicator

        #expect(indicator.icon == "circle.fill")
        #expect(indicator.color == .green)
    }

    @Test("statusIndicator returns yellow half-circle for waiting")
    func statusIndicatorWaiting() {
        let wave = makeWave(id: "test", repo: "/tmp", status: .waiting)
        let indicator = wave.statusIndicator

        #expect(indicator.icon == "circle.lefthalf.filled")
        #expect(indicator.color == .yellow)
    }

    @Test("statusIndicator returns gray circle for idle")
    func statusIndicatorIdle() {
        let wave = makeWave(id: "test", repo: "/tmp", status: .idle)
        let indicator = wave.statusIndicator

        #expect(indicator.icon == "circle")
        #expect(indicator.color == .gray)
    }

    @Test("statusIndicator returns clock for idle cron wave")
    func statusIndicatorIdleCron() {
        let wave = makeWave(
            id: "test",
            repo: "/tmp",
            stimulus: Stimulus(kind: .cron, cron: "0 9 * * *"),
            status: .idle
        )
        let indicator = wave.statusIndicator

        #expect(indicator.icon == "clock")
        #expect(indicator.color == .gray)
    }

    @Test("statusIndicator returns red X for failed")
    func statusIndicatorFailed() {
        let wave = makeWave(id: "test", repo: "/tmp", status: .failed)
        let indicator = wave.statusIndicator

        #expect(indicator.icon == "xmark.circle.fill")
        #expect(indicator.color == .red)
    }

    @Test("statusIndicator returns green checkmark for completed")
    func statusIndicatorCompleted() {
        let wave = makeWave(id: "test", repo: "/tmp", status: .completed)
        let indicator = wave.statusIndicator

        #expect(indicator.icon == "checkmark.circle.fill")
        #expect(indicator.color == .green)
    }

    // MARK: - Computed Properties

    @Test("areaDisplay joins multiple areas")
    func areaDisplayJoins() {
        let wave = makeWave(id: "test", area: ["src/", "lib/"], repo: "/tmp")

        #expect(wave.areaDisplay == "src/, lib/")
    }

    @Test("areaDisplay returns dot for root area")
    func areaDisplayDot() {
        let wave = makeWave(id: "test", area: ["."], repo: "/tmp")

        #expect(wave.areaDisplay == ".")
    }

    @Test("areaDisplay returns empty for nil area")
    func areaDisplayNil() {
        let wave = makeWave(id: "test", area: [], repo: "/tmp")

        #expect(wave.areaDisplay == "")
    }

    @Test("directionDisplay joins multiple goals")
    func directionDisplayJoins() {
        let wave = makeWave(
            id: "test",
            direction: ["product-engineer", "designer"],
            repo: "/tmp"
        )

        #expect(wave.directionDisplay == "product-engineer, designer")
    }

    @Test("directionDisplay returns empty for nil direction")
    func directionDisplayNil() {
        let wave = makeWave(id: "test", direction: [], repo: "/tmp")

        #expect(wave.directionDisplay == "")
    }

    @Test("flowDisplay returns flow name")
    func flowDisplayName() {
        let wave = makeWave(id: "test", flow: "polish", repo: "/tmp")

        #expect(wave.flowDisplay == "polish")
    }

    @Test("flowDisplay returns ship for empty flow")
    func flowDisplayDefault() {
        let wave = makeWave(id: "test", flow: "", repo: "/tmp")

        #expect(wave.flowDisplay == "ship")
    }

    @Test("shortId returns first 7 characters")
    func shortIdTruncates() {
        let wave = makeWave(id: "abcdefghijklmnop", repo: "/tmp")

        #expect(wave.shortId == "abcdefg")
    }

    // MARK: - isConfigured

    @Test("isConfigured returns true when area is set")
    func isConfiguredWithArea() {
        let wave = makeWave(id: "test", area: ["src/"], repo: "/tmp")

        #expect(wave.isConfigured == true)
    }

    @Test("isConfigured returns false when area is nil")
    func isConfiguredWithoutArea() {
        let wave = makeWave(id: "test", area: [], repo: "/tmp")

        #expect(wave.isConfigured == false)
    }

    // MARK: - Detail Text

    @Test("detailText combines area, flow, and stimulus")
    func detailTextCombines() {
        let wave = makeWave(
            id: "test",
            area: ["src/"],
            flow: "ship",
            repo: "/tmp",
            stimulus: Stimulus(kind: .loop)
        )

        #expect(wave.detailText == "src/ · ship · loop")
    }

    @Test("detailText omits manual stimulus")
    func detailTextOmitsManual() {
        let wave = makeWave(
            id: "test",
            area: ["."],
            flow: "debug",
            repo: "/tmp",
            stimulus: Stimulus(kind: .manual)
        )

        #expect(wave.detailText == ". · debug")
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
            model: "claude",
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
            model: "claude",
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
            model: "claude",
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
        let wave = makeWave(
            id: "test",
            repo: "/tmp",
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
        let wave = makeWave(id: "test", repo: "/tmp", status: .idle)

        #expect(wave.waitingReason == nil)
    }
}

@Suite("CollapsePRsResult")
struct CollapsePRsResultTests {

    @Test("initializes with URL and closed PRs")
    func initializesWithUrlAndClosedPRs() {
        let result = CollapsePRsResult(
            newPRUrl: "https://github.com/owner/repo/pull/100",
            closedPRs: [1, 2, 3]
        )

        #expect(result.newPRUrl == "https://github.com/owner/repo/pull/100")
        #expect(result.closedPRs == [1, 2, 3])
    }

    @Test("initializes with nil URL")
    func initializesWithNilUrl() {
        let result = CollapsePRsResult(newPRUrl: nil, closedPRs: [])

        #expect(result.newPRUrl == nil)
        #expect(result.closedPRs.isEmpty)
    }
}

@Suite("Stimulus")
struct StimulusTests {

    @Test("description returns kind for non-cron")
    func descriptionNonCron() {
        #expect(Stimulus(kind: .manual).description == "manual")
        #expect(Stimulus(kind: .once).description == "once")
        #expect(Stimulus(kind: .loop).description == "loop")
        #expect(Stimulus(kind: .watch).description == "watch")
    }

    @Test("description includes cron expression")
    func descriptionCron() {
        let stimulus = Stimulus(kind: .cron, cron: "0 9 * * *")

        #expect(stimulus.description == "cron(0 9 * * *)")
    }

    @Test("icon returns correct SF Symbol for each kind")
    func iconForKind() {
        #expect(Stimulus(kind: .manual).icon == "circle")
        #expect(Stimulus(kind: .once).icon == "play.circle")
        #expect(Stimulus(kind: .loop).icon == "circle.fill")
        #expect(Stimulus(kind: .watch).icon == "eye.circle")
        #expect(Stimulus(kind: .cron).icon == "clock")
    }
}

@Suite("WaveStatus")
struct WaveStatusTests {

    @Test("color returns correct SwiftUI color")
    func colorForStatus() {
        #expect(WaveStatus.running.color == .green)
        #expect(WaveStatus.waiting.color == .yellow)
        #expect(WaveStatus.idle.color == .gray)
        #expect(WaveStatus.completed.color == .green)
        #expect(WaveStatus.failed.color == .red)
    }

    @Test("icon returns correct SF Symbol")
    func iconForStatus() {
        #expect(WaveStatus.running.icon == "circle.fill")
        #expect(WaveStatus.waiting.icon == "circle.lefthalf.filled")
        #expect(WaveStatus.idle.icon == "circle")
        #expect(WaveStatus.completed.icon == "checkmark.circle.fill")
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
        #expect(session.command == "lf design")
    }

    @Test("command includes shell-escaped prompt")
    func commandWithPrompt() {
        let session = InteractiveSession(
            waveId: "wave-1",
            step: "design",
            worktreePath: "/tmp/wt",
            prompt: "add rate limiting"
        )
        #expect(session.command == "lf design 'add rate limiting'")
    }

    @Test("command escapes single quotes in prompt")
    func commandEscapesSingleQuotes() {
        let session = InteractiveSession(
            waveId: "wave-1",
            step: "debug",
            worktreePath: "/tmp/wt",
            prompt: "fix the user's auth flow"
        )
        #expect(session.command == "lf debug 'fix the user'\\''s auth flow'")
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
        #expect(session.command == "lf implement 'add $HOME expansion & pipes | redirects > /dev/null'")
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
