import Testing
@testable import Loopflow

@Suite("Attempt failure presentation")
struct AttemptFailurePresentationTests {
    private let capacityReason = "codex_error: Selected model is at capacity. Please try a different model."

    @Test("a running body for the same logical step is a retry")
    func retrying() {
        let failed = turn("turn-10", .failed, body: body("body-1", reason: capacityReason))
        let retry = turn("turn-11", .running, body: body("body-2"))

        let result = attemptFailurePresentations(
            turns: [failed, retry],
            playhead: nil,
            loopState: .turning
        )[failed.id]

        #expect(result?.state == .retrying)
        #expect(result?.title == "Attempt failed · retrying")
        #expect(result?.reason == capacityReason)
    }

    @Test("a completed body for the same logical step recovers the attempt")
    func recovered() {
        let failed = turn("turn-10", .failed, body: body("body-1", reason: capacityReason))
        let retry = turn("turn-11", .completed, body: body("body-2"))

        let result = attemptFailurePresentations(
            turns: [failed, retry],
            playhead: nil,
            loopState: .idle
        )[failed.id]

        #expect(result?.state == .recoveredOnRetry)
        #expect(result?.title == "Attempt failed · recovered on retry")
    }

    @Test("the same selected step with no active body has a retry pending")
    func retryPending() {
        let failedBody = body("body-1", reason: capacityReason)
        let failed = turn("turn-10", .failed, body: failedBody)
        let playhead = PlayheadView(
            stack: [],
            active: nil,
            now: stepRef(failedBody),
            next: nil,
            returnTo: nil
        )

        let result = attemptFailurePresentations(
            turns: [failed],
            playhead: playhead,
            loopState: .idle
        )[failed.id]

        #expect(result?.state == .retryPending)
    }

    @Test("a later iteration is not a retry")
    func iterationDoesNotMatch() {
        let failed = turn("turn-10", .failed, body: body("body-1", reason: capacityReason))
        let later = turn("turn-11", .completed, body: body("body-2", iteration: 1))

        let result = attemptFailurePresentations(
            turns: [failed, later],
            playhead: nil,
            loopState: .idle
        )[failed.id]

        #expect(result?.state == .failed)
    }

    @Test("the same named step in another invocation is not a retry")
    func invocationDoesNotMatch() {
        let failed = turn("turn-10", .failed, body: body("body-1", reason: capacityReason))
        let later = turn(
            "turn-11",
            .completed,
            body: body("body-2", invocationID: "invocation-2")
        )

        let result = attemptFailurePresentations(
            turns: [failed, later],
            playhead: nil,
            loopState: .idle
        )[failed.id]

        #expect(result?.state == .failed)
    }

    @Test("loop failure never invents terminal step failure")
    func loopFailureKeepsAttemptLanguage() {
        let failedBody = body("body-1", reason: capacityReason)
        let failed = turn("turn-10", .failed, body: failedBody)
        let playhead = PlayheadView(
            stack: [],
            active: nil,
            now: stepRef(failedBody),
            next: nil,
            returnTo: nil
        )

        let result = attemptFailurePresentations(
            turns: [failed],
            playhead: playhead,
            loopState: .failed
        )[failed.id]

        #expect(result?.state == .failed)
        #expect(result?.title == "Attempt failed")
        #expect(result?.reason == capacityReason)
    }

    @Test("a bodyless failed turn stays a neutral legacy failure")
    func bodylessFailureHasNoAttemptProjection() {
        let turn = try! ChatTurn(
            id: "turn-legacy",
            role: .assistant,
            text: "",
            status: .failed,
            items: [],
            createdAt: "2026-07-10T17:53:00Z",
            from: nil,
            body: nil,
            activity: nil
        )

        #expect(attemptFailurePresentations(
            turns: [turn],
            playhead: nil,
            loopState: .failed
        ).isEmpty)
    }

    @Test("equivalent operational failures roll up at the latest attempt")
    func equivalentFailuresRollUp() throws {
        let first = turn("turn-10", .failed, body: body("body-1", reason: capacityReason))
        let second = turn("turn-11", .failed, body: body("body-2", reason: capacityReason))
        let third = turn("turn-12", .failed, body: body("body-3", reason: capacityReason))
        let authored = try ChatTurn(
            id: "turn-13",
            role: .assistant,
            text: "The provider is still at capacity; the Wave remains available for messages.",
            status: .completed,
            items: [],
            createdAt: "2026-07-10T17:54:00Z",
            from: nil,
            body: body("body-4"),
            activity: nil
        )
        let turns = [first, second, third, authored]

        let failures = attemptFailurePresentations(
            turns: turns,
            playhead: nil,
            loopState: .failed
        )

        #expect(failures.keys.sorted() == ["turn-12"])
        #expect(failures["turn-12"]?.count == 3)
        #expect(failures["turn-12"]?.title == "3 attempts failed · recovered on retry")
        #expect(failures["turn-12"]?.attempts.map(\.bodyId) == ["body-1", "body-2", "body-3"])
        #expect(visibleConversationTurns(turns, failures: failures).map(\.id) == [
            "turn-12", "turn-13",
        ])
    }

    @Test("authored failure prose never collapses")
    func authoredFailureProseStaysVisible() throws {
        let first = try ChatTurn(
            id: "turn-10",
            role: .assistant,
            text: "First attempt reached the provider but failed.",
            status: .failed,
            items: [],
            createdAt: "2026-07-10T17:53:00Z",
            from: nil,
            body: body("body-1", reason: capacityReason),
            activity: nil
        )
        let second = try ChatTurn(
            id: "turn-11",
            role: .assistant,
            text: "Second attempt failed after preserving the queue.",
            status: .failed,
            items: [],
            createdAt: "2026-07-10T17:54:00Z",
            from: nil,
            body: body("body-2", reason: capacityReason),
            activity: nil
        )

        let failures = attemptFailurePresentations(
            turns: [first, second],
            playhead: nil,
            loopState: .failed
        )

        #expect(failures.count == 2)
        #expect(visibleConversationTurns([first, second], failures: failures).map(\.id) == [
            "turn-10", "turn-11",
        ])
    }

    @Test("whitespace-only thoughts are hidden without hiding real thoughts")
    func emptyThoughtVisibility() {
        #expect(!ConversationItem.thought(id: "empty", text: "  \n").isVisibleInConversation)
        #expect(ConversationItem.thought(id: "real", text: "checking").isVisibleInConversation)
        #expect(ConversationItem.message(id: "message", text: "", phase: nil).isVisibleInConversation)
    }

    private func body(
        _ id: String,
        invocationID: String = "invocation-1",
        iteration: Int = 0,
        reason: String? = nil
    ) -> BodyProvenance {
        BodyProvenance(
            bodyId: id,
            invocationId: invocationID,
            stepIndex: 0,
            flow: "wave",
            step: "wave_pursue",
            iteration: iteration,
            sessionId: nil,
            harness: "codex",
            model: nil,
            host: "host",
            worktree: "/tmp/worktree",
            startedAt: "2026-07-10T17:53:00Z",
            endedAt: nil,
            terminationReason: reason
        )
    }

    private func turn(_ id: String, _ status: Lifecycle, body: BodyProvenance) -> ChatTurn {
        try! ChatTurn(
            id: id,
            role: .assistant,
            text: "",
            status: status,
            items: [],
            createdAt: "2026-07-10T17:53:00Z",
            from: nil,
            body: body,
            activity: nil
        )
    }

    private func stepRef(_ body: BodyProvenance) -> PlayheadStepRef {
        PlayheadStepRef(
            invocationId: body.invocationId,
            flow: body.flow,
            step: body.step,
            kind: .skill,
            index: body.stepIndex,
            total: 3,
            iteration: body.iteration
        )
    }
}
