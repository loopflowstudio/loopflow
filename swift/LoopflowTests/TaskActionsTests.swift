import Foundation
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
        row["selection_states"] = [lifecycle == "abandoned" ? "done" : lifecycle] + (auto ? ["auto"] : [])
        row["run_control"] = ["pausing": pausing, "unavailable": refusal.map { $0 as Any } ?? NSNull()]
        return try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row))
    }

    /// The Task's reading with its Graph at `position`, or with none taken up.
    private func work(_ position: [String: Any]?) throws -> TaskOverview {
        var wire = try #require(JSONSerialization.jsonObject(with: loadFixtureData("task_work.json")) as? [String: Any])
        if let position {
            var graph = try #require(wire["graph"] as? [String: Any])
            graph["position"] = position
            wire["graph"] = graph
        } else {
            wire["graph"] = NSNull()
        }
        return try JSONDecoder().decode(TaskOverview.self, from: JSONSerialization.data(withJSONObject: wire))
    }

    private func actions(_ position: [String: Any]?, _ row: RoadmapTask) throws -> TaskActions {
        TaskActions(work: try work(position), row: row, execution: nil)
    }

    @Test func pausedTaskOffersUnpauseAfterCleanupAndKeepsAutoOffAvailable() throws {
        let held = try actions(nil, row(lifecycle: "paused", refusal: "Task is paused"))
        #expect(held.run == nil)
        #expect(held.pause == .unpause && held.pauseRefusal == nil)
        #expect(held.autoRefusal == "Task is paused")
        let draining = try actions(nil, row(lifecycle: "paused", auto: true, refusal: "land is finishing", pausing: true))
        #expect(draining.pause == .unpause && draining.pauseRefusal == "land is finishing")
        #expect(draining.refusal == draining.pauseRefusal)
        #expect(draining.auto == .on && draining.autoRefusal == nil)
        #expect(try actions(nil, row()).pause == .pause)
        for lifecycle in ["unstarted", "done", "abandoned"] {
            #expect(try actions(nil, row(lifecycle: lifecycle)).pause == nil)
        }
    }

    @Test("Waiting at a step offers Run and Auto-Ship")
    func waiting() throws {
        let actions = try actions(["kind": "at", "step": "demo"], row())
        #expect(actions.run?.label == "Run")
        #expect(actions.auto == .off)
        #expect(actions.auto?.offer == "Auto-Ship")
        #expect(actions.autoRefusal == nil)
    }

    @Test("A run carrying the Task offers no Run, and auto on offers Stop Auto-Ship")
    func carried() throws {
        let running: [String: Any] = ["kind": "run", "step": "plan", "at": "implement", "lf_process_id": "p", "running": true]
        let actions = try actions(running, row(lifecycle: "active", auto: true))
        #expect(actions.run == nil)
        #expect(actions.auto?.offer == "Stop Auto-Ship")
    }

    @Test("A reading with no Graph offers Start; no reading offers no run")
    func noGraph() throws {
        let unstarted = try row(lifecycle: "unstarted")
        #expect(try actions(nil, unstarted).run == TaskActions.Run(label: "Start", help: "lf task run W2-144"))
        #expect(TaskActions(work: nil, row: unstarted, execution: nil).run == nil)
    }

    @Test("Rust's refusal disables Run and Auto-Ship, never Stop Auto-Ship")
    func refused() throws {
        let waiting: [String: Any] = ["kind": "at", "step": "demo"]
        let off = try actions(waiting, row(refusal: "Task checkout is on another Machine"))
        #expect(off.run != nil)
        #expect(off.runRefusal == "Task checkout is on another Machine")
        #expect(off.autoRefusal == "Task checkout is on another Machine")
        let on = try actions(waiting, row(auto: true, refusal: "Task checkout is on another Machine"))
        #expect(on.auto == .on)
        #expect(on.autoRefusal == nil)
    }

    @Test("The reason is given only while an offered action is disabled")
    func refusalShown() throws {
        let waiting: [String: Any] = ["kind": "at", "step": "demo"]
        let running: [String: Any] = ["kind": "run", "step": "plan", "at": "implement", "lf_process_id": "p", "running": true]
        let ended: [String: Any] = ["kind": "at", "step": "end"]
        #expect(try actions(waiting, row(refusal: "worktree is missing")).refusal == "worktree is missing")
        #expect(try actions(running, row(lifecycle: "active", refusal: "worktree is missing")).refusal == "worktree is missing")
        #expect(try actions(running, row(lifecycle: "active", auto: true, refusal: "worktree is missing")).refusal == nil)
        #expect(try actions(ended, row(lifecycle: "done", refusal: "Task is complete")).refusal == nil)
        #expect(try actions(waiting, row()).refusal == nil)
    }

    @Test("A completed Task offers Auto-Ship only to stop it")
    func completed() throws {
        let ended: [String: Any] = ["kind": "at", "step": "end"]
        #expect(try actions(ended, row(lifecycle: "done")).auto == nil)
        #expect(try actions(ended, row(lifecycle: "done", auto: true)).auto == .on)
    }

    @Test("The Task reading's auto mark outranks the state its row carries")
    func readingsMark() throws {
        // The fixture's execution has auto off, set by a person.
        let execution = try JSONDecoder().decode(TaskExecution.self, from: loadFixtureData("task_checkout.json"))
        #expect(TaskActions(work: try work(nil), row: try row(auto: true), execution: execution).auto == .off)
    }
}
