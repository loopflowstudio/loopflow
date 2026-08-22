import Testing
import ViewInspector

@testable import Loopflow
@testable import LoopflowMac

@MainActor
@Suite("Project failure history")
struct ProjectFailureHistoryViewTests {
    @Test("historical failure is labeled with its occurrence time")
    func rendersFailureAsHistory() throws {
        let view = ProjectFailureHistoryView(
            failure: HistoricalFailure(
                message: "project runner failed: credential is missing",
                occurredAt: "2026-07-22T09:30:00Z",
                runId: "run_failure"
            )
        )

        _ = try view.inspect().find(
            text: "Last failure at 2026-07-22T09:30:00Z: project runner failed: credential is missing"
        )
    }
}
