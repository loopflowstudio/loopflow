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

    @MainActor @Test func unpauseAlwaysLetsTheMachineChooseAndTheMenuOffersNoOverride() async throws {
        for (action, label) in [(WorkflowControl.Action.resume, "Paused: implement"),
                                (.unpause, "Paused: ship-review"),
                                (.unpause, "Paused: land"),
                                (.unpause, "Paused: end")] {
            let control = WorkflowControl(action: action, label: label)
            let row = try row(lifecycle: "paused", refusal: "Task is paused")
            let actions = TaskActions(work: try work(control), row: row, execution: nil)
            let machine = HeldMachine()
            let changes = PendingChanges { write, id in
                #expect(write == .taskUnpause(directory: "/repo", issue: "W2-144", run: true))
                return try await machine.send(write, id)
            }
            var requested: TaskActions.Pause?
            let bar = TaskWorkflowControls(task: row, actions: actions, pending: nil,
                run: { _ in Issue.record("Unpause must lift the hold in the same write") },
                pause: { requested = $0 },
                interrupt: {}, toggleAuto: {}, complete: {}) { EmptyView() }
            _ = try bar.inspect().find(text: label)
            #expect(throws: (any Error).self) { try bar.inspect().find(button: "Unpause without running") }
            try bar.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-primary").button().tap()
            #expect(requested == .unpause)
            changes.setPause(try #require(requested), issue: row.task.identifier, directory: "/repo", notices: NoticeCenter(), show: nil) {}
            await machine.answer(nil)
        }
    }

    @MainActor @Test func theTrackGrowsTheCurrentStepIntoTheChipAndJumpsFromTheOthers() throws {
        // The fixture runs implement inside build, the third of four steps.
        let work = try work(.init(action: .pause, label: "build · implement"))
        let row = try row(lifecycle: "moving")
        let actions = TaskActions(work: work, row: row, execution: nil)
        #expect(actions.track.map(\.name) == ["plan", "review-design", "build", "demo"] && actions.chip == 2)
        #expect(actions.steps == ["start", "plan", "build"])
        var jumped: [String?] = []
        let bar = try TaskWorkflowControls(task: row, actions: actions, pending: nil, run: { jumped.append($0) },
            pause: { _ in }, interrupt: {}, toggleAuto: {}, complete: {}) { EmptyView() }.inspect()
        _ = try bar.find(text: "build · implement")
        #expect(throws: (any Error).self) { try bar.find(viewWithAccessibilityIdentifier: "task-workflow-step-2") }
        // A running step holds the other steps until it stops.
        #expect(try bar.find(viewWithAccessibilityIdentifier: "task-workflow-step-0").isDisabled())

        let stopped = TaskActions(work: try self.work(.init(action: .run, label: "Run: implement")), row: try self.row(), execution: nil)
        let idle = try TaskWorkflowControls(task: row, actions: stopped, pending: nil, run: { jumped.append($0) },
            pause: { _ in }, interrupt: {}, toggleAuto: {}, complete: {}) { EmptyView() }.inspect()
        try idle.find(viewWithAccessibilityIdentifier: "task-workflow-step-3").button().tap()
        #expect(jumped == ["demo"])
    }

    @MainActor @Test func aPendingPauseShowsTheHeldGlyph() throws {
        let row = try row(lifecycle: "moving")
        let actions = TaskActions(work: try work(.init(action: .pause, label: "build · implement")), row: row, execution: nil)
        let bar = try TaskWorkflowControls(task: row, actions: actions, pending: PendingChanges.pausing, run: { _ in },
            pause: { _ in }, interrupt: {}, toggleAuto: {}, complete: {}) { EmptyView() }.inspect()
        let primary = try bar.find(viewWithAccessibilityIdentifier: "task-workflow-primary")
        #expect(try primary.find(ViewType.Image.self).actualImage().name() == "pause.fill")
        _ = try bar.find(text: PendingChanges.pausing)
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

    @MainActor @Test func pausingDrawsADisabledPauseGlyphAndHidesStopMenuItems() throws {
        // Even before roadmap.runControl.pausing catches up, WorkflowAction.pausing disables primary directly.
        let row = try row(lifecycle: "paused", refusal: nil, pausing: false)
        let actions = TaskActions(work: try work(.init(action: .pausing, label: "Pausing: build · implement")),
                                  row: row, execution: nil)
        #expect(actions.control?.action == .pausing)
        let bar = try TaskWorkflowControls(task: row, actions: actions, pending: nil, run: { _ in },
            pause: { _ in Issue.record("Pausing must not trigger pause or unpause") },
            interrupt: {}, toggleAuto: {}, complete: {}) { EmptyView() }.inspect()
        let primary = try bar.find(viewWithAccessibilityIdentifier: "task-workflow-primary")
        #expect(try primary.find(ViewType.Image.self).actualImage().name() == "pause.fill")
        #expect(try primary.isDisabled())
        _ = try bar.find(text: "Pausing: build · implement")
        #expect(try bar.find(viewWithAccessibilityIdentifier: "task-workflow-step-0").isDisabled())
        let menu = try bar.find(viewWithAccessibilityIdentifier: "task-workflow-move")
        #expect(throws: (any Error).self) { try menu.find(button: "Interrupt step") }
        #expect(throws: (any Error).self) { try menu.find(button: "Pause") }
    }

    @Test func cleanupBlocksUnpauseAndAnArmedSwitchCanStillTurnOff() throws {
        let actions = TaskActions(work: try work(.init(action: .pausing, label: "Pausing: deliver · land")),
                                  row: try row(lifecycle: "paused", auto: true, refusal: "land is finishing", pausing: true), execution: nil)
        #expect(actions.primaryRefusal == "land is finishing")
        #expect(actions.auto == .on && actions.autoRefusal == nil)
    }

    @Test func defaultGraphSuppliesItsFirstStepBeforeTheTaskTakesItUp() throws {
        let catalog = try JSONDecoder().decode([GraphCatalogEntry].self, from: loadFixtureData("graph_catalog.json"))
        let unstarted = try row(lifecycle: "unstarted")
        let actions = TaskActions(work: try work(nil), row: unstarted, execution: nil, outline: catalog[0].workflow)
        #expect(actions.steps == ["start", "design", "pursue", "xor-route", "queue"])
        #expect(actions.control?.label == "Run: design")
        #expect(actions.pause == nil)
        let held = TaskActions(work: try work(nil), row: try row(lifecycle: "paused"), execution: nil, outline: catalog[0].workflow)
        #expect(held.control == WorkflowControl(action: .resume, label: "Paused: design"))
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

    @MainActor @Test func blockedBreakpointsKeepTheirReasonWithoutOfferingContinue() throws {
        let row = try row(lifecycle: "active")
        for blocked in [false, true] {
            var json = try #require(JSONSerialization.jsonObject(with: loadFixtureData("task_work.json")) as? [String: Any])
            var graph = try #require(json["graph"] as? [String: Any])
            graph["control"] = ["action": blocked ? "wait" : "continue", "label": blocked ? "Choose a layout" : "Continue"]
            graph["position"] = ["kind": "breakpoint", "step": "demo", "arrival": "arr_1", "blocked": blocked]
            graph["current_step"] = 3
            graph["sentence"] = blocked ? "At breakpoint demo. Blocked: choose a layout" : "At breakpoint demo."
            json["graph"] = graph
            let work = try JSONDecoder().decode(TaskOverview.self, from: JSONSerialization.data(withJSONObject: json))
            let actions = TaskActions(work: work, row: row, execution: nil)
            let bar = try TaskWorkflowControls(task: row, actions: actions, pending: nil,
                run: { _ in }, pause: { _ in }, interrupt: {}, toggleAuto: {}, complete: {}) { EmptyView() }.inspect()
            if blocked {
                #expect(actions.breakpointBlocker?.contains("Choose a layout") == true)
                #expect(throws: (any Error).self) { try bar.find(viewWithAccessibilityIdentifier: "task-workflow-primary") }
            } else {
                #expect(actions.breakpointArrival == "arr_1")
                #expect(try !bar.find(viewWithAccessibilityIdentifier: "task-workflow-primary").isDisabled())
            }
        }
    }
}
