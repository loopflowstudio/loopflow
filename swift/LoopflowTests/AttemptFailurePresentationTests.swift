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
