import Foundation
import Testing
@testable import Loopflow

/// What a Task offers, for the Mac's Workflow bar and the phone's menu alike.
@Suite("Task actions")
struct TaskActionsTests {
    /// The shared fixture's first row: a moving Task with auto on that Rust would run.
    private func row(lifecycle: String = "idle", auto: Bool = false, refusal: String? = nil) throws -> RoadmapTask {
        let root = try #require(JSONSerialization.jsonObject(with: loadFixtureData("roadmap_snapshot.json")) as? [String: Any])
        var row = try #require(((root["tasks"] as? [String: Any])?["items"] as? [[String: Any]])?.first)
        row["lifecycle"] = lifecycle
        row["selection_states"] = [lifecycle] + (auto ? ["auto"] : [])
        row["run_control"] = ["unavailable": refusal.map { $0 as Any } ?? NSNull()]
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

    @Test("A blocked breakpoint omits the plain Continue button; an unblocked one offers Continue with its arrival")
    func breakpointBlocking() throws {
        let ready = try actions(["kind": "breakpoint", "step": "after-ship-review", "arrival": "arr_1", "blocked": false], row())
        #expect(ready.run == TaskActions.Run(label: "Continue", help: "lf task run W2-144 --at-arrival arr_1", arrival: "arr_1"))

        let blocked = try actions(["kind": "breakpoint", "step": "after-ship-review", "arrival": "arr_1", "blocked": true], row())
        #expect(blocked.run == nil)
    }

    @Test("Start from… lists start and top-level runnable stages, excluding standalone breakpoints")
    func startFromSteps() throws {
        let graph = TaskGraph(
            name: "feature",
            breakpoints: ["after-explore", "after-ship-review", "after-verify"],
            position: .at(step: "start"),
            steps: [
                .init(name: "explore", breakpoint: false),
                .init(name: "after-explore", breakpoint: true),
                .init(name: "plan", breakpoint: false),
                .init(name: "build", breakpoint: false),
                .init(name: "ship-review", breakpoint: false),
                .init(name: "after-ship-review", breakpoint: true),
                .init(name: "deliver", breakpoint: false),
                .init(name: "verify", breakpoint: false),
                .init(name: "after-verify", breakpoint: true),
            ],
            currentStep: nil,
            sentence: "Not started."
        )
        #expect(graph.startFromSteps == ["start", "explore", "plan", "build", "ship-review", "deliver", "verify"])
    }
}
