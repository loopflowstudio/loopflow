import Foundation
import Loopflow
import XCTest
@testable import LoopflowiOS

/// What "Task actions" offers on the phone, from the Task's Graph and its row.
@MainActor
final class TaskActionsTests: XCTestCase {
    private func graph(_ position: [String: Any]) throws -> TaskGraph {
        try JSONDecoder().decode(TaskGraph.self, from: JSONSerialization.data(withJSONObject: [
            "name": "code", "interactive_steps": [], "position": position, "steps": [],
            "current_step": NSNull(), "sentence": "Waiting for you at ship-review.",
        ] as [String: Any]))
    }

    private func actions(_ graph: Loaded<TaskGraph?>, _ states: [WorkSelectionState]) -> [TaskWorkspaceView.Action] {
        TaskWorkspaceView.actions(graph: graph, states: states, issue: "LOO-5")
    }

    func testATaskWaitingAtAStepCanBeRunAndAutoShipped() throws {
        let waiting = try graph(["kind": "at", "step": "ship-review"])
        XCTAssertEqual(actions(.value(waiting), [.working, .idle]), [.run("Run"), .autoShip])
    }

    func testATaskARunCarriesOffersNoRun() throws {
        let running = try graph(["kind": "run", "step": "plan", "at": "implement", "lf_process_id": "p", "running": true])
        XCTAssertEqual(actions(.value(running), [.working, .active, .auto]), [.stopAutoShip])
    }

    func testATaskWithNoGraphStartsOnlyWhileUnstarted() {
        XCTAssertEqual(actions(.value(nil), [.unstarted, .backlog]), [.run("Start")])
        XCTAssertEqual(actions(.value(nil), [.done]), [])
    }

    func testWithoutAGraphReadingOnlyTheAutoSettingIsOffered() {
        XCTAssertEqual(actions(.failed("The Machine is away"), [.working, .idle]), [.autoShip])
        XCTAssertEqual(actions(.loading, [.unstarted]), [])
    }

    func testAReadingTheMachineCouldNotMakeShowsNoGraph() throws {
        let frame = RawJSON(Data(#"{"answers":null,"unavailable":"store is busy","body":null,"read_at":null}"#.utf8))
        guard case .failed(let reason) = TaskWorkspaceView.graph(in: frame, task: "LOO-5") else {
            return XCTFail("an unavailable reading must not yield a Graph")
        }
        XCTAssertEqual(reason, "store is busy")
    }
}
