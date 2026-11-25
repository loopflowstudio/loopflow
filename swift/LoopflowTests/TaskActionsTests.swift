import Foundation
import SwiftUI
import ViewInspector
import Testing
@testable import Loopflow

/// What a Task offers, for the Mac's Workflow bar and the phone's menu alike.
@Suite("Task actions")
struct TaskActionsTests {
    /// The shared fixture's first row: a moving Task with auto on that Rust would run.
    private func row(lifecycle: String = "idle", auto: Bool = false, refusal: String? = nil, pausing: Bool = false) throws -> RoadmapTask {
        let root = try #require(JSONSerialization.jsonObject(with: loadFixtureData("roadmap_snapshot.json")) as? [String: Any])
        var row = try #require(((root["tasks"] as? [String: Any])?["items"] as? [[String: Any]])?.first)
        row["lifecycle"] = lifecycle
        row["selection_states"] = (lifecycle == "paused" ? ["working", "paused"] : [lifecycle == "abandoned" ? "done" : lifecycle]) + (auto ? ["auto"] : [])
        row["run_control"] = ["pausing": pausing, "unavailable": refusal.map { $0 as Any } ?? NSNull()]
        return try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row))
    }

    private func work(_ control: WorkflowControl?) throws -> TaskOverview {
        var wire = try #require(JSONSerialization.jsonObject(with: loadFixtureData("task_work.json")) as? [String: Any])
        if let control {
            var graph = try #require(wire["graph"] as? [String: Any])
            graph["control"] = ["action": control.action.rawValue, "label": control.label]
            wire["graph"] = graph
        } else {
            wire["graph"] = NSNull()
        }
        return try JSONDecoder().decode(TaskOverview.self, from: JSONSerialization.data(withJSONObject: wire))
    }

    @MainActor @Test func primaryUnpauseLetsTheMachineChooseAndOnlyResumeOffersAnOverride() async throws {
        for (action, label, menu, runs) in [(WorkflowControl.Action.resume, "Paused: implement", false, true),
                                             (.resume, "Paused: implement", true, false),
                                             (.unpause, "Paused: ship-review", false, true),
                                             (.unpause, "Paused: land", false, true),
                                             (.unpause, "Paused: end", false, true)] {
            let control = WorkflowControl(action: action, label: label)
            let row = try row(lifecycle: "paused", refusal: "Task is paused")
            let actions = TaskActions(work: try work(control), row: row, execution: nil)
            let machine = HeldMachine()
            let changes = PendingChanges { write, id in
                #expect(write == .taskUnpause(directory: "/repo", issue: "W2-144", run: runs))
                return try await machine.send(write, id)
            }
            var requested: TaskActions.Pause?
            let bar = TaskWorkflowControls(task: row, actions: actions, pending: nil,
                run: { _ in Issue.record("Unpause must lift the hold in the same write") },
                pause: { requested = $0 },
                interrupt: {}, toggleAuto: {}, complete: {}) { EmptyView() }
            _ = try bar.inspect().find(text: label)
            if action == .unpause {
                #expect(throws: (any Error).self) { try bar.inspect().find(button: "Unpause without running") }
            }
            if menu {
                try bar.inspect().find(button: "Unpause without running").tap()
            } else {
                try bar.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-primary").button().tap()
            }
            changes.setPause(try #require(requested), issue: row.task.identifier, directory: "/repo", notices: NoticeCenter(), show: nil) {}
            await machine.answer(nil)
        }
    }

    @Test func aMergedPRKeepsTheStoppedVerifyActionBesideComplete() throws {
        let rows = try JSONDecoder().decode([String: RoadmapTask].self, from: loadFixtureData("task_delivery_rows.json"))
        let row = try #require(rows["pending"])
        let verify = WorkflowControl(action: .run, label: "Run: verify")
        let actions = TaskActions(work: try work(verify), row: row, execution: nil)
        #expect(actions.completeReady && actions.control == verify && actions.primaryRefusal == nil)
    }

    @MainActor @Test func waitingAtLandLeavesAutoAndStartFromAvailable() throws {
        let work = try JSONDecoder().decode(TaskOverview.self, from: loadFixtureData("task_overview.json"))
        let row = try row()
        let bar = TaskWorkflowControls(task: row, actions: TaskActions(work: work, row: row, execution: nil), pending: nil,
            run: { _ in }, pause: { _ in }, interrupt: {}, toggleAuto: {}, complete: {}) { EmptyView() }
        let view = try bar.inspect()
        _ = try view.find(text: "Waiting: land")
        #expect(try view.find(viewWithAccessibilityIdentifier: "task-workflow-primary").isDisabled())
        #expect(try !view.find(viewWithAccessibilityIdentifier: "task-workflow-move-start").isDisabled())
        #expect(try !view.find(viewWithAccessibilityIdentifier: "task-auto").isDisabled())
    }

    @Test func cleanupBlocksUnpauseAndAnArmedSwitchCanStillTurnOff() throws {
        let actions = TaskActions(work: try work(.init(action: .resume, label: "Paused: land")),
                                  row: try row(lifecycle: "paused", auto: true, refusal: "land is finishing", pausing: true), execution: nil)
        #expect(actions.primaryRefusal == "land is finishing")
        #expect(actions.auto == .on && actions.autoRefusal == nil)
    }

    @Test func defaultGraphSuppliesItsFirstStepBeforeTheTaskTakesItUp() throws {
        let catalog = try JSONDecoder().decode([GraphCatalogEntry].self, from: loadFixtureData("graph_catalog.json"))
        let unstarted = try row(lifecycle: "unstarted")
        let actions = TaskActions(work: try work(nil), row: unstarted, execution: nil, outline: catalog[0].workflow)
        #expect(actions.control?.label == "Run: design")
        #expect(actions.pause == nil)
        #expect(TaskActions(work: nil, row: unstarted, execution: nil).control == nil)
    }

    @Test func aRunRefusalStillAllowsTurningAutoOff() throws {
        let work = try work(.init(action: .run, label: "Run: gate"))
        let off = TaskActions(work: work, row: try row(refusal: "checkout missing"), execution: nil)
        #expect(off.primaryRefusal == "checkout missing" && off.autoRefusal == "checkout missing")
        let on = TaskActions(work: work, row: try row(auto: true, refusal: "checkout missing"), execution: nil)
        #expect(on.autoRefusal == nil)
    }

    @Test("The Task reading's auto mark outranks the state its row carries")
    func readingsMark() throws {
        // The fixture's execution has auto off, set by a person.
        let execution = try JSONDecoder().decode(TaskExecution.self, from: loadFixtureData("task_checkout.json"))
        #expect(TaskActions(work: try work(nil), row: try row(auto: true), execution: execution).auto == .off)
    }
}
