#if os(macOS)
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Work observation retry")
struct WorkObservationRetryTests {
    private let scope = WorkScope(repo: "/src/loopflow", headless: false, task: nil, activity: nil)

    @Test("Repeated failures back off to thirty seconds with or without successful frames", arguments: [false, true])
    func repeatedFailure(withReadings: Bool) {
        var retry = WorkObservationRetry()
        let start = ContinuousClock.now
        for (attempt, seconds) in [1, 2, 4, 8, 16, 30, 30].enumerated() {
            let now = start + .seconds(attempt * 60)
            if withReadings {
                retry.receive(reading(.selections([])), scope: scope, at: now)
                retry.receive(reading(.selections([])), scope: scope, at: now + .seconds(1))
            }
            #expect(retry.failed(at: now + .seconds(2)) == .seconds(seconds))
        }
    }

    @Test("A quiet healthy observation resets only at thirty seconds", arguments: [29, 30])
    func healthyReset(seconds: Int) {
        var retry = WorkObservationRetry()
        let now = ContinuousClock.now
        _ = retry.failed(at: now)
        _ = retry.failed(at: now)
        retry.receive(reading(.selections([])), scope: scope, at: now)
        #expect(retry.failed(at: now + .seconds(seconds)) == .seconds(seconds == 30 ? 1 : 4))
    }

    @Test("An unavailable part cannot be healed by another part or checkpoint")
    func unavailablePart() {
        var retry = WorkObservationRetry()
        let now = ContinuousClock.now
        _ = retry.failed(at: now)
        retry.receive(reading(.selections(nil)), scope: scope, at: now)
        retry.receive(reading(.sessions(.init(repo: "/src/loopflow", includesHeadless: false, entries: []))),
                      scope: scope, at: now + .seconds(10))
        retry.receive(reading(.checkpoint), scope: scope, at: now + .seconds(20))
        #expect(retry.failed(at: now + .seconds(60)) == .seconds(2))
    }

    @Test("A recovered part begins a fresh healthy interval", arguments: [29, 30])
    func recoveredPart(seconds: Int) {
        var retry = WorkObservationRetry()
        let now = ContinuousClock.now
        _ = retry.failed(at: now)
        retry.receive(reading(.selections([])), scope: scope, at: now)
        retry.receive(reading(.selections(nil), unavailable: "read failed"), scope: scope, at: now + .seconds(20))
        retry.receive(reading(.selections([])), scope: scope, at: now + .seconds(40))
        #expect(retry.failed(at: now + .seconds(40 + seconds)) == .seconds(seconds == 30 ? 1 : 2))
    }

    @Test("A failure after a healthy interval keeps the earned reset")
    func healthyThenUnavailable() {
        var retry = WorkObservationRetry()
        let now = ContinuousClock.now
        _ = retry.failed(at: now)
        retry.receive(reading(.selections([])), scope: scope, at: now)
        retry.receive(reading(.selections(nil), unavailable: "read failed"), scope: scope, at: now + .seconds(30))
        #expect(retry.failed(at: now + .seconds(31)) == .seconds(1))
    }

    @Test("Leaving an unavailable part's scope permits recovery")
    func changedScope() {
        var retry = WorkObservationRetry()
        let now = ContinuousClock.now
        _ = retry.failed(at: now)
        retry.receive(reading(.task(nil), unavailable: "read failed"),
                      scope: WorkScope(repo: scope.repo, headless: false, task: "LOO-1", activity: nil), at: now)
        retry.receive(reading(.selections([])), scope: scope, at: now + .seconds(10))
        #expect(retry.failed(at: now + .seconds(40)) == .seconds(1))
    }

    private func reading(_ content: WorkFrame.Content, unavailable: String? = nil) -> WorkFrame {
        WorkFrame(sequence: 1, answers: nil, unavailable: unavailable, content: content, wire: nil)
    }
}
#endif
