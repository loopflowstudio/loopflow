import Foundation
import Loopflow
import XCTest
@testable import LoopflowiOS

/// What is the phone's own on the Task screen: reading the Machine's frames and
/// opening a conversation that begins while the screen shows. What a Task
/// offers is `TaskActions`, tested with the shared library.
@MainActor
final class TaskActionsTests: XCTestCase {
    private func fixture(_ name: String) throws -> [String: Any] {
        let directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto")
        return try XCTUnwrap(JSONSerialization.jsonObject(
            with: Data(contentsOf: directory.appendingPathComponent(name))) as? [String: Any])
    }

    /// The Task's reading, with the fixture's Sessions or with none yet.
    private func reading(sessions: Bool) throws -> Loaded<TaskWorkspaceView.Reading> {
        var work = try fixture("task_work.json")
        if !sessions { work["sessions"] = [Any]() }
        let roadmap = try fixture("roadmap_snapshot.json")
        let row = try XCTUnwrap(((roadmap["tasks"] as? [String: Any])?["items"] as? [[String: Any]])?.first)
        return .value(TaskWorkspaceView.Reading(
            work: try JSONDecoder().decode(TaskOverview.self, from: JSONSerialization.data(withJSONObject: work)),
            row: try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row)),
            execution: nil))
    }

    private func task(session: String?) -> MobileTask {
        MobileTask(WorkViewPlan.Content(work: .init(repository: "repo", kind: .task, id: "task"), title: "Task",
            states: [.working], identifier: "W2-144", primarySession: session, location: nil, argv: nil, unavailable: []))
    }

    func testAReadingTheMachineCouldNotMakeShowsNoGraph() throws {
        let frame = RawJSON(Data(#"{"answers":null,"unavailable":"store is busy","body":null,"read_at":null}"#.utf8))
        guard case .failed(let reason) = TaskWorkspaceView.reading(in: frame, task: "LOO-5") else {
            return XCTFail("an unavailable reading must not yield a Graph")
        }
        XCTAssertEqual(reason, "store is busy")
    }

    func testATaskStartedFromItsScreenOpensTheConversationItsRowNames() async throws {
        var asked: [String] = []
        let begun = try await TaskWorkspaceView.conversation(after: try reading(sessions: true)) { repository in
            asked.append(repository)
            return self.task(session: "session-1")
        }
        XCTAssertEqual(begun, "session-1")
        XCTAssertEqual(asked, ["/src/loopflow"])
    }

    func testARowThatNamesNoConversationYetLeavesTheScreenAsItIs() async throws {
        let begun = try await TaskWorkspaceView.conversation(after: try reading(sessions: true)) { _ in self.task(session: nil) }
        XCTAssertNil(begun)
    }

    func testBeforeTheReadingListsASessionNothingIsAsked() async throws {
        for reading in [try reading(sessions: false), .loading, .failed("The Machine is away")] {
            let begun = try await TaskWorkspaceView.conversation(after: reading) { _ in
                XCTFail("the row is read only once a Session exists")
                return nil
            }
            XCTAssertNil(begun)
        }
    }
}
