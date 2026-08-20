import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@MainActor
@Suite("Wave chat message source")
struct MessageRowTests {
    @Test("collapsed failures still identify their source")
    func collapsedFailureKeepsSource() throws {
        let turn = try ChatTurn(
            id: "turn-2",
            role: .assistant,
            text: "",
            status: .failed,
            items: [],
            createdAt: "2026-07-21T09:00:00Z",
            body: nil,
            activity: nil
        )
        let failure = AttemptFailurePresentation(
            state: .failed,
            reason: "Connection closed.",
            flow: "wave",
            step: "pursue",
            attempts: [
                AttemptFailureDetail(
                    turnId: "turn-1",
                    bodyId: "body-1",
                    reason: "Connection closed.",
                    createdAt: "2026-07-21T08:59:00Z"
                ),
                AttemptFailureDetail(
                    turnId: "turn-2",
                    bodyId: "body-2",
                    reason: "Connection closed.",
                    createdAt: "2026-07-21T09:00:00Z"
                ),
            ]
        )
        let row = MessageRow(
            turn: turn,
            timestampLabel: "now",
            attemptFailure: failure,
            source: .local(journalSeq: 2)
        )

        _ = try row.inspect().find(text: "Local")
    }
}
