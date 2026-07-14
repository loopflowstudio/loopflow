// Tests for WaveViewModel and Trigger struct.

import Foundation
import SwiftUI
import Testing
@testable import LoopflowMac
@testable import Loopflow

@Suite("Wave View Model")
struct WaveModelTests {
    private func makeWave(
        id: String = "test-id",
        name: String = "",
        repo: String = "/tmp/repo",
        direction: [String] = [],
        area: [String] = [],
        triggers: [Trigger] = [],
        status: WaveStatus = .idle,
        iteration: Int = 0
    ) -> WaveViewModel {
        WaveViewModel(
            api: Wave(
                id: id,
                name: name,
                repo: repo,
                direction: direction,
                area: area,
                triggers: triggers,
                status: status,
                iteration: iteration
            )
        )
    }

    // MARK: - Display Name

    @Test("displayName uses name when set")
    func displayNameUsesName() {
        let wave = makeWave(name: "swift-falcon")

        #expect(wave.displayName == "swift-falcon")
    }

    @Test("displayName generates from area when name is empty")
    func displayNameGeneratesFromConfig() {
        let wave = makeWave(area: ["src/auth"])

        #expect(wave.displayName == "src/auth")
    }

    @Test("displayName shows 'root' for dot area")
    func displayNameRootForDotArea() {
        let wave = makeWave(area: ["."])

        #expect(wave.displayName == "root")
    }

    @Test("displayName shows root when area is empty")
    func displayNameDefaultFlow() {
        let wave = makeWave(area: [])

        #expect(wave.displayName == "root")
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

    @Test("shortId returns first 7 characters")
    func shortIdTruncates() {
        let wave = makeWave(id: "abcdefghijklmnop", repo: "/tmp")

        #expect(wave.shortId == "abcdefg")
    }

    // MARK: - Detail Text

    @Test("detailText combines area and trigger signal")
    func detailTextCombines() {
        let wave = makeWave(
            id: "test",
            repo: "/tmp",
            area: ["src/"],
            triggers: [Trigger(signal: .repo)]
        )

        #expect(wave.detailText == "src/ · repo")
    }

    @Test("detailText omits trigger when none active")
    func detailTextOmitsWhenNoTrigger() {
        let wave = makeWave(
            id: "test",
            repo: "/tmp",
            area: ["."]
        )

        #expect(wave.detailText == ".")
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

    @Test("command returns skill without prompt")
    func commandWithoutPrompt() {
        let session = InteractiveSession(
            waveId: "wave-1",
            skill: "design",
            worktreePath: "/tmp/wt"
        )
        #expect(session.command == "lf design && lf commit --push")
    }

    @Test("command includes shell-escaped prompt")
    func commandWithPrompt() {
        let session = InteractiveSession(
            waveId: "wave-1",
            skill: "design",
            worktreePath: "/tmp/wt",
            prompt: "add rate limiting"
        )
        #expect(session.command == "lf design 'add rate limiting' && lf commit --push")
    }

    @Test("command escapes single quotes in prompt")
    func commandEscapesSingleQuotes() {
        let session = InteractiveSession(
            waveId: "wave-1",
            skill: "debug",
            worktreePath: "/tmp/wt",
            prompt: "fix the user's auth flow"
        )
        #expect(session.command == "lf debug 'fix the user'\\''s auth flow' && lf commit --push")
    }

    @Test("command handles special shell characters")
    func commandHandlesSpecialChars() {
        let session = InteractiveSession(
            waveId: "wave-1",
            skill: "implement",
            worktreePath: "/tmp/wt",
            prompt: "add $HOME expansion & pipes | redirects > /dev/null"
        )
        // Single quotes protect all special characters except single quotes themselves
        #expect(session.command == "lf implement 'add $HOME expansion & pipes | redirects > /dev/null' && lf commit --push")
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
