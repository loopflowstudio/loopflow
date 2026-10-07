#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

// Inspect production view bodies and invoke controls directly. No NSApplication,
// NSWindow, window server connection, screenshot capture, or UI automation.
private let fixtures = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    .appendingPathComponent("tests/fixtures/dto")

private actor Recorder {
    private(set) var calls: [[String]] = []
    func add(_ args: [String]) { calls.append(args) }
}

/// A scripted workspace reader: the test decides which frames arrive.
@MainActor
private final class Feed {
    private var continuation: AsyncThrowingStream<WorkFrame, any Error>.Continuation?
    private(set) var requests: [WorkRequest] = []
    var isOpen: Bool { continuation != nil }

    func open() -> WorkObservation {
        let (stream, continuation) = AsyncThrowingStream<WorkFrame, any Error>.makeStream()
        self.continuation = continuation
        return WorkObservation(frames: stream, request: { request in
            Task { @MainActor in self.requests.append(request) }
        }, cancel: { continuation.finish() })
    }

    func send(_ frame: WorkFrame) { continuation?.yield(frame) }
}

@Suite("Desktop without a display")
@MainActor
struct DesktopHeadlessTests {
    @Test("A Task's work, a Workflow move and a new Flow run arrive from the stream, with no lf read")
    func taskWorkFollowsTheStream() async throws {
        func object(_ name: String) throws -> [String: Any] {
            try #require(JSONSerialization.jsonObject(
                with: Data(contentsOf: fixtures.appendingPathComponent(name))) as? [String: Any])
        }
        func frame(_ part: String, sequence: Int, answers: Int, body: [String: Any]) throws -> WorkFrame {
            let line: [String: Any] = [
                "part": part, "sequence": sequence, "answers": answers, "home": "/home",
                "revisions": NSNull(), "unavailable": NSNull(), "body": body,
            ]
            return try WorkFrame.decode(line: JSONSerialization.data(withJSONObject: line))
        }
        let roadmap = try object("roadmap_snapshot.json")
        let waves = try #require(roadmap["waves"] as? [[String: Any]]).map { try #require($0["wave"]) }
        var work = try object("task_work.json")
        let run = try object("flow_detail.json")
        let planning: [String: Any] = ["roadmap": roadmap, "waves": waves]

        let calls = Recorder()
        let feed = Feed()
        let model = WorkModel(query: RegistryQuery(watchWork: { await feed.open() }) { args, _ in
            await calls.add(args)
            return ""
        })
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        func eventually(_ condition: @MainActor () -> Bool) async throws {
            for _ in 0..<600 where !condition() { try await Task.sleep(for: .milliseconds(5)) }
            try #require(condition())
        }

        try await eventually { feed.isOpen }
        feed.send(try frame("planning", sequence: 1, answers: 0, body: planning))
        try await eventually { model.task(id: "issue-now") != nil }
        let found = try #require(model.task(id: "issue-now"))
        let (task, wave) = (found.task, found.wave.wave)
        model.select(.task(id: task.id))
        model.syncWorkScope()
        try await eventually { feed.requests.last?.id == 2 }
        guard case .scope(_, let scope) = try #require(feed.requests.last) else {
            Issue.record("selecting a Task scopes the reader to it")
            return
        }
        #expect(scope.task == task.task.identifier)

        feed.send(try frame("task", sequence: 2, answers: 2,
                            body: ["task": task.task.identifier, "work": work, "flow_runs": [run]]))
        try await eventually { model.taskWork[task.id].value != nil }
        let shown = try #require(model.taskWork[task.id].value)
        let view = TaskWorkView(model: model, task: task)
        for id in shown.sessions.map(\.id) + shown.processes.map(\.id) {
            _ = try view.inspect().find(viewWithAccessibilityIdentifier: "task-work-\(id)")
        }

        // Any Flow process is one line that opens to its id, its launched graph,
        // where it stands and every step it started.
        let log = FlowProcessLog(model: model, taskId: task.id)
        let flow = try #require(shown.flows.first)
        #expect(throws: (any Error).self) {
            try log.inspect().find(viewWithAccessibilityIdentifier: "flow-run-status-\(flow.id)")
        }
        #expect(throws: (any Error).self) { try log.inspect().find(text: flow.id) }
        model.navigation.expandedFlowRuns.insert(flow.id)
        _ = try log.inspect().find(viewWithAccessibilityIdentifier: "flow-run-id-\(flow.id)")
        let detail = try #require(model.flowRuns[flow.id])
        let status = try log.inspect().find(viewWithAccessibilityIdentifier: "flow-run-status-\(flow.id)").text().string()
        #expect(status == "Running implement · pass 2")
        for step in detail.steps {
            _ = try log.inspect().find(viewWithAccessibilityIdentifier: "flow-run-step-\(step.processLfid)")
        }
        #expect(detail.progress.execution == .running)
        #expect(detail.progress.current == 0)

        // Desktop's own write asks the reader again and shows what it answers.
        let moving = Task { await model.moveTask(to: "demo", task: task, wave: wave) }
        try await eventually { feed.requests.last == .refresh(id: 3) }
        var workflow = try #require(work["workflow"] as? [String: Any])
        workflow["position"] = ["kind": "node", "node": "demo"]
        work["workflow"] = workflow
        var second = try #require((work["flows"] as? [[String: Any]])?.first)
        second["id"] = "44444444-4444-4444-8444-444444444444"
        second["name"] = "pursue"
        var secondRun = run
        secondRun["entry"] = second
        let moved: [String: Any] = [
            "task": task.task.identifier,
            "work": work.merging(["flows": [try #require((work["flows"] as? [[String: Any]])?.first), second]]) { $1 },
            "flow_runs": [run, secondRun],
        ]
        // A frame read before the write cannot stand for it.
        feed.send(try frame("task", sequence: 3, answers: 2, body: moved))
        try await Task.sleep(for: .milliseconds(50))
        #expect(model.taskWork[task.id].value?.flows.count == 1)
        feed.send(try frame("task", sequence: 4, answers: 3, body: moved))
        feed.send(try frame("planning", sequence: 5, answers: 3, body: planning))
        await moving.value
        try await eventually { model.taskWork[task.id].value?.flows.count == 2 }
        #expect(model.taskWork[task.id].value?.workflow?.position == .node("demo"))
        _ = try log.inspect().find(viewWithAccessibilityIdentifier: "task-work-44444444-4444-4444-8444-444444444444")
        #expect(model.flowRuns["44444444-4444-4444-8444-444444444444"]?.entry.name == "pursue")
        #expect(await calls.calls == [["task", "move", task.task.identifier, "demo"]])
    }

    @Test("A Project's workflow is drawn, is set through lf, and keeps an invalid file visible")
    func waveDefault() async throws {
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")))
        let wave = try #require(roadmap.waves.first).wave
        let catalog = String(decoding: try Data(contentsOf: fixtures.appendingPathComponent("flow_catalog.json")), as: UTF8.self)
        let calls = Recorder()
        let model = WorkModel(query: RegistryQuery { args, _ in
            await calls.add(args)
            if args.prefix(2) == ["flow", "list"] { return catalog }
            if args.prefix(2) == ["flow", "customize"] { return "/repo/.lf/workflows/\(args[2]).yaml\n" }
            if args.prefix(2) == ["wave", "update-plan"], args.last == "broken" {
                throw RegistryQueryError("broken does not load")
            }
            return "{}"
        })
        await model.loadFlowCatalog()
        func view(_ name: String) -> WaveWorkflowView { WaveWorkflowView(model: model, wave: wave, name: name) }

        // A builtin workflow is drawn before any Task has run it, and editing it is an explicit Customize.
        let builtin = view("code")
        _ = try builtin.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-node-demo")
        #expect(try builtin.inspect().find(viewWithAccessibilityIdentifier: "wave-workflow-source").text().string() == "builtin workflow")
        #expect(try builtin.inspect().find(viewWithAccessibilityIdentifier: "wave-workflow-edit").button().labelView().text().string() == "Customize")
        let entry = try #require(model.flowCatalog.value?.named("code"))
        #expect(await model.definitionSource(entry, wave: wave)?.path == "/repo/.lf/workflows/code.yaml")

        // An invalid repository file stays listed with its reason and can still be opened.
        let invalid = view("proof")
        #expect(try invalid.inspect().find(viewWithAccessibilityIdentifier: "wave-workflow-invalid").text().string().contains("unreachable"))
        #expect(try invalid.inspect().find(viewWithAccessibilityIdentifier: "wave-workflow-source").text().string() == ".lf/workflows/proof.yaml")
        #expect(try invalid.inspect().find(viewWithAccessibilityIdentifier: "wave-workflow-edit").button().labelView().text().string() == "Edit")

        // Setting the workflow changes only that line; a refusal is shown on the Wave.
        await model.setWorkflow("code", wave: wave)
        #expect(await calls.calls.contains(["wave", "update-plan", "--wave", wave.name, "--workflow", "code"]))
        #expect(model.workflowErrors[wave.id] == nil)
        await model.setWorkflow("broken", wave: wave)
        #expect(try view("code").inspect().find(viewWithAccessibilityIdentifier: "wave-workflow-error").text().string() == "broken does not load")
    }

    @Test("A Task's Workflow shows its running edge, offers a stopped edge again, and moves by choice or by node")
    func taskWorkflow() async throws {
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")))
        let wave = try #require(roadmap.waves.first)
        let task = try #require(wave.tasks.items.first { $0.flow.control(.start)?.unavailable == nil })
        var wire = try #require(JSONSerialization.jsonObject(
            with: Data(contentsOf: fixtures.appendingPathComponent("task_work.json"))) as? [String: Any])
        let started = Recorder()
        let model = WorkModel(query: RegistryQuery(
            start: { args, _ in await started.add(args) },
            run: { args, _ in
                if args.prefix(2) == ["task", "move"] { await started.add(args) }
                return "{}"
            }))
        func view(position: [String: Any]? = nil, outgoing: [Int] = []) throws -> TaskWorkflowHeader {
            var workflow = try #require(wire["workflow"] as? [String: Any])
            if let position { workflow["position"] = position }
            workflow["outgoing"] = outgoing
            wire["workflow"] = workflow
            let work = try JSONDecoder().decode(TaskWork.self, from: JSONSerialization.data(withJSONObject: wire))
            return TaskWorkflowHeader(model: model, task: task, wave: wave.wave, work: .available(work))
        }

        // The fixture's `pursue` edge is running: Rust lists no edge to choose beside it.
        let running = try view()
        #expect(try running.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-edge-2")
            .accessibilityValue().string() == "Running")
        #expect(throws: (any Error).self) {
            try running.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-run-2")
        }

        // Stopped, it holds the Task and is offered again with the other edge leaving its node.
        let process = "11111111-1111-4111-8111-111111111111"
        let stopped = try view(
            position: ["kind": "edge", "edge": 2, "process_lfid": process, "running": false], outgoing: [1, 2])
        let again = try stopped.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-run-2")
        #expect(try again.accessibilityValue().string() == "Stopped")
        #expect(try !again.button().isDisabled())
        _ = try stopped.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-run-1")

        // At `demo` the edges leaving it are the buttons, named by what they run.
        let waiting = try view(position: ["kind": "node", "node": "demo"], outgoing: [3, 4])
        #expect(throws: (any Error).self) {
            try waiting.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-position")
        }
        #expect(try waiting.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-node-demo")
            .accessibilityValue().string() == "Current")
        let ship = try waiting.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-run-4").button()
        #expect(try ship.labelView().text().string() == "ship")
        try ship.tap()
        for _ in 0..<200 where await started.calls.isEmpty { try await Task.sleep(for: .milliseconds(5)) }
        #expect(await started.calls == [["-b", "task", "run", task.task.identifier, "ship"]])

        // A Task that has taken up no workflow starts on its Project's.
        wire["workflow"] = NSNull()
        let unstarted = TaskWorkflowHeader(model: model, task: task, wave: wave.wave, work: .available(
            try JSONDecoder().decode(TaskWork.self, from: JSONSerialization.data(withJSONObject: wire))))
        try unstarted.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-start").button().tap()
        for _ in 0..<200 where await started.calls.count < 2 { try await Task.sleep(for: .milliseconds(5)) }
        #expect(await started.calls.last == ["-b", "task", "run", task.task.identifier])

        // Going back is the same command a person would type.
        await model.moveTask(to: "design", task: task, wave: wave.wave)
        #expect(await started.calls.last == ["task", "move", task.task.identifier, "design"])

        // Completing over Linear's own completion is the forced move.
        await model.moveTask(to: "end", force: true, task: task, wave: wave.wave)
        #expect(await started.calls.last == ["task", "move", task.task.identifier, "end", "--force"])
    }

    @Test("A workflow is drawn as a graph: circles, described nodes, a loop arc and an arrow per edge")
    func workflowGraph() async throws {
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")))
        let wave = try #require(roadmap.waves.first)
        let task = try #require(wave.tasks.items.first { $0.flow.control(.start)?.unavailable == nil })
        let started = Recorder()
        let model = WorkModel(query: RegistryQuery(start: { args, _ in await started.add(args) }, run: { _, _ in "{}" }))
        var wire = try #require(JSONSerialization.jsonObject(
            with: Data(contentsOf: fixtures.appendingPathComponent("task_work.json"))) as? [String: Any])
        var drawnWorkflow: Workflow?
        func view(_ change: (inout [String: Any]) -> Void) throws -> TaskWorkflowHeader {
            var workflow = try #require(wire["workflow"] as? [String: Any])
            change(&workflow)
            wire["workflow"] = workflow
            let work = try JSONDecoder().decode(TaskWork.self, from: JSONSerialization.data(withJSONObject: wire))
            drawnWorkflow = work.workflow
            return TaskWorkflowHeader(model: model, task: task, wave: wave.wave, work: .available(work))
        }

        // `feature`, with the Task waiting at `design`.
        let feature = try view {
            $0["position"] = ["kind": "node", "node": "design"]
            $0["outgoing"] = [1, 2]
        }
        for node in ["start", "design", "demo", "end"] {
            let drawn = try feature.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-node-\(node)")
            #expect(try drawn.accessibilityValue().string() == (node == "design" ? "Current" : ""))
        }
        _ = try feature.inspect().find(text: "you review the plan")
        _ = try feature.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-edge-0")
        _ = try feature.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-edge-3")
        let workflow = try #require(drawnWorkflow)
        let drawn = WorkflowGraphLayout(nodes: workflow.nodes, edges: workflow.edges)
        #expect(drawn.arrows.map(\.route) == [.straight, .loop(level: 0), .straight, .loop(level: 0), .straight])
        #expect(drawn.boxes.map(\.terminal) == [true, false, false, true])
        // Each revise loop arcs over its own node, above the row.
        let design = try #require(drawn.boxes.first { $0.name == "design" })
        #expect(drawn.arrows[1].label.y < design.frame.minY)
        #expect(abs(drawn.arrows[1].label.x - design.frame.midX) < 1)

        // A definition with no PR: the edge into `end` runs nothing and is chosen as `end`.
        let catalog = try #require(JSONSerialization.jsonObject(
            with: Data(contentsOf: fixtures.appendingPathComponent("flow_catalog.json"))) as? [[String: Any]])
        let research = try #require(catalog.first { $0["name"] as? String == "research" }?["workflow"] as? [String: Any])
        let findings = try view {
            $0.merge(research) { _, definition in definition }
            $0["position"] = ["kind": "node", "node": "findings"]
            $0["outgoing"] = [1, 2]
        }
        #expect(try findings.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-node-findings")
            .accessibilityValue().string() == "Current")
        _ = try findings.inspect().find(text: "you read the findings")
        let finish = try findings.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-run-2").button()
        #expect(try finish.labelView().text().string() == "finish")
        try finish.tap()
        for _ in 0..<200 where await started.calls.isEmpty { try await Task.sleep(for: .milliseconds(5)) }
        #expect(await started.calls == [["-b", "task", "run", task.task.identifier, "end"]])

        // An edge that goes back, or passes over a node, takes its own lane below the row.
        let node = { Workflow.Node(name: $0, skill: "demo", description: nil) }
        let edge = { Workflow.Edge(from: $0, to: $1, flow: $2) }
        let lanes = WorkflowGraphLayout(nodes: [node("a"), node("b")], edges: [
            edge("start", "a", "one"), edge("a", "b", "two"), edge("b", "a", "back"), edge("start", "b", "skip"),
        ])
        #expect(lanes.arrows.map(\.route) == [.straight, .straight, .lane(level: 0), .lane(level: 1)])
        #expect(lanes.arrows[2].label.y > lanes.boxes[1].frame.maxY)
        #expect(lanes.arrows[3].label.y > lanes.arrows[2].label.y)
        #expect(lanes.size.height > lanes.arrows[3].label.y)
    }

    @Test("A Task lists waiting conversations before working ones, its primary leading each")
    func taskSessions() throws {
        let base = try #require(JSONSerialization.jsonObject(
            with: Data(contentsOf: fixtures.appendingPathComponent("session.json"))) as? [String: Any])
        func session(_ id: String, waiting: Bool = false, primary: Bool = false, state: String = "unknown") throws -> SessionRecord {
            var value = base
            value["id"] = id
            value["state"] = state
            value["attention"] = waiting ? "waiting" : NSNull()
            value["task_primary"] = primary
            return try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: value))
        }
        let groups = TaskSessionGroups([
            try session("working"), try session("closed", waiting: true, state: "closed"),
            try session("asks", waiting: true), try session("primary", primary: true),
        ])
        #expect(groups.waiting.map(\.id) == ["asks"])
        #expect(groups.working.map(\.id) == ["primary", "working"])
        #expect(try session("run").statusLabel == "Session")
    }

    @Test("Loading, unavailable, empty and selected Work have distinct content")
    func workStates() throws {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Unexpected external read") }
        let model = WorkModel(query: query)
        let view = WorkSurfaceView(model: model)
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "loopflow-work-loading")

        model.applyFixture(roadmap: .unavailable(lastGood: nil, reason: "offline"),
                           waves: .available([]),
                           workActivity: .loading, repos: [])
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "loopflow-work-unavailable")
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "loopflow-work-empty")
        }

        let data = try Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json"))
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        wire["waves"] = []
        let empty = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: wire))
        model.applyFixture(roadmap: .available(empty), waves: .available([]),
                           workActivity: .loading, repos: [])
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "loopflow-work-empty")

        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: data)
        let wave = try #require(roadmap.waves.first).wave
        model.applyFixture(roadmap: .available(roadmap), waves: .available([wave.toWave()]),
                           workActivity: .loading, repos: [])
        model.navigation.presentation = .full
        let navigator = WorkNavigator(model: model, onOpenSession: { _ in })
        try navigator.inspect()
            .find(viewWithAccessibilityIdentifier: "work-wave-\(wave.id)").button().tap()

        #expect(model.selection == .wave(id: wave.id))
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "loopflow-detail-wave")
        let title = try view.inspect().find(viewWithAccessibilityIdentifier: "wave-title").text().string()
        #expect(title == "Product")
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "loopflow-work-loading")
        }
    }
}
#endif
