import Foundation
import Loopflow
import SwiftUI
import ViewInspector
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
    private func reading(sessions: Bool, pausedAt: (action: WorkflowControl.Action, label: String)? = nil) throws -> Loaded<TaskWorkspaceView.Reading> {
        var work = try fixture("task_work.json")
        if !sessions { work["sessions"] = [Any]() }
        let roadmap = try fixture("roadmap_snapshot.json")
        var row = try XCTUnwrap(((roadmap["tasks"] as? [String: Any])?["items"] as? [[String: Any]])?.first)
        if let pausedAt {
            var graph = try XCTUnwrap(work["graph"] as? [String: Any])
            graph["control"] = ["action": pausedAt.action.rawValue, "label": pausedAt.label]
            work["graph"] = graph
            row["lifecycle"] = "paused"
            row["section"] = "working"
            row["working_group"] = "paused"
            row["selection_states"] = ["open", "working", "paused"]
            row["run_control"] = ["pausing": false, "unavailable": "Task is paused"]
        }
        return .value(TaskWorkspaceView.Reading(
            work: try JSONDecoder().decode(TaskOverview.self, from: JSONSerialization.data(withJSONObject: work)),
            row: try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row)),
            execution: nil, readAt: nil))
    }

    func testPausedTaskOffersUnpauseInWorking() throws {
        for control in [(action: WorkflowControl.Action.resume, label: "Paused: implement"),
                        (action: .unpause, label: "Paused: ship-review")] {
            guard case .value(let held) = try reading(sessions: true, pausedAt: control) else { return XCTFail("missing Task") }
            let row = try XCTUnwrap(held.row)
            var requested: TaskActions.Pause?
            let bar = TaskWorkflowControls(task: row, actions: held.actions(for: row), pending: nil,
                run: { _ in XCTFail("Play should lift the hold") }, pause: { requested = $0 },
                interrupt: {}, toggleAuto: {}, complete: {}) { EmptyView() }
            let view = try bar.inspect()
            XCTAssertEqual(try view.find(text: control.label).string(), control.label)
            try view.find(viewWithAccessibilityIdentifier: "task-workflow-primary").button().tap()
            XCTAssertEqual(requested, .unpause)
        }
    }

    /// The track grows and folds between Auto and the transport button; the row keeps its width.
    func testPausingKeepsTheWorkflowControlsTheSameWidth() throws {
        func width(paused: Bool) throws -> CGFloat {
            guard case .value(let reading) = try reading(sessions: true, pausedAt: paused ? (.resume, "Paused: implement") : nil) else {
                XCTFail("Task fixture is unavailable")
                return 0
            }
            let row = try XCTUnwrap(reading.row)
            let controls = TaskWorkflowControls(task: row, actions: reading.actions(for: row), pending: nil,
                run: { _ in }, pause: { _ in }, interrupt: {}, toggleAuto: {}, complete: {}) { EmptyView() }
            return UIHostingController(rootView: controls).sizeThatFits(in: CGSize(width: 360, height: 80)).width
        }
        let running = try width(paused: false)
        XCTAssertLessThanOrEqual(running, 361)
        XCTAssertEqual(try width(paused: true), running)
    }

    func testAReadingTheMachineCouldNotMakeShowsNoGraph() throws {
        let frame = RawJSON(Data(#"{"answers":null,"unavailable":"store is busy","body":null,"read_at":null}"#.utf8))
        guard case .failed(let reason) = TaskWorkspaceView.reading(in: frame, task: "LOO-5") else {
            return XCTFail("an unavailable reading must not yield a Graph")
        }
        XCTAssertEqual(reason, "store is busy")
    }

    func testPausingDrawsDisabledPauseGlyphAndPendingCompleteDisablesCheckmarkImmediately() async throws {
        guard case .value(let pausing) = try reading(sessions: true, pausedAt: (.pausing, "Pausing: deliver · land")) else {
            return XCTFail("missing Task")
        }
        let pausingRow = try XCTUnwrap(pausing.row)
        let pausingBar = try TaskWorkflowControls(task: pausingRow, actions: pausing.actions(for: pausingRow), pending: nil,
            run: { _ in }, pause: { _ in }, interrupt: {}, toggleAuto: {}, complete: {}) { EmptyView() }.inspect()
        let primary = try pausingBar.find(viewWithAccessibilityIdentifier: "task-workflow-primary")
        XCTAssertEqual(try primary.find(ViewType.Image.self).actualImage().name(), "pause.fill")
        XCTAssertTrue(try primary.isDisabled())

        // Pending Complete patches the TaskWorkspaceView row immediately, disabling ✓ before the Machine answers.
        guard case .value(let open) = try reading(sessions: true) else { return XCTFail("missing Task") }
        let rawRow = try XCTUnwrap(open.row)
        let changes = PendingChanges { _, _ in
            try await Task.sleep(for: .seconds(60))
            return nil
        }
        changes.make(.complete(task: rawRow.task.identifier), subject: rawRow.task.identifier, verb: "completed",
                     write: .taskComplete(directory: rawRow.task.repo, issue: rawRow.task.identifier),
                     notices: NoticeCenter(), show: nil)
        let patched = changes.apply(to: [rawRow], readAt: open.readAt).first ?? rawRow
        let bar = try TaskWorkflowControls(task: patched, actions: open.actions(for: patched), pending: nil,
            run: { _ in }, pause: { _ in }, interrupt: {}, toggleAuto: {}, complete: {}) { EmptyView() }.inspect()
        XCTAssertTrue(try bar.find(viewWithAccessibilityIdentifier: "task-complete").isDisabled())
        _ = try bar.find(text: "Done")
    }

    func testATaskStartedFromItsScreenOpensTheConversationTheMachineNames() async throws {
        var asked = 0
        let begun = try await TaskWorkspaceView.conversation(after: try reading(sessions: true)) {
            asked += 1
            return "session-1"
        }
        XCTAssertEqual(begun, "session-1")
        XCTAssertEqual(asked, 1)
    }

    func testAMachineThatNamesNoConversationYetLeavesTheScreenAsItIs() async throws {
        let begun = try await TaskWorkspaceView.conversation(after: try reading(sessions: true)) { nil }
        XCTAssertNil(begun)
    }

    func testBeforeTheReadingListsASessionNothingIsAsked() async throws {
        for reading in [try reading(sessions: false), .loading, .failed("The Machine is away")] {
            let begun = try await TaskWorkspaceView.conversation(after: reading) {
                XCTFail("the primary is read only once a Session exists")
                return nil
            }
            XCTAssertNil(begun)
        }
    }
}
