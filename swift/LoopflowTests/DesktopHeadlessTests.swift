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

@Suite("Desktop without a display")
@MainActor
struct DesktopHeadlessTests {
    @Test("Task work renders checkout conversations, Flows and mechanical Execs")
    func taskWorkInventory() async throws {
        let data = try Data(contentsOf: fixtures.appendingPathComponent("task_work.json"))
        let work = try JSONDecoder().decode(TaskWork.self, from: data)
        let payload = "{\"execution\":{\"work\":\(String(decoding: data, as: UTF8.self))}}"
        let run = String(decoding: try Data(contentsOf: fixtures.appendingPathComponent("flow_detail.json")), as: UTF8.self)
        let query = RegistryQuery { args, _ in args.first == "flow" ? run : payload }
        let model = PodiumModel(query: query)
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")))
        let wave = try #require(roadmap.waves.first)
        let task = try #require(wave.tasks.items.first)
        await model.loadTaskWork(task: task, wave: wave.wave)
        let view = TaskWorkView(model: model, task: task, wave: wave.wave)
        for id in work.sessions.map(\.id) + work.flows.map(\.id) + work.execs.map(\.id) {
            _ = try view.inspect().find(viewWithAccessibilityIdentifier: "task-work-\(id)")
        }

        // Any Flow exec opens to its launched graph, where it stands and every step it started.
        let flow = try #require(work.flows.first)
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "flow-run-status-\(flow.id)")
        }
        model.navigation.expandedFlowRuns.insert(flow.id)
        await model.loadFlowRun(flow.id, wave: wave.wave)
        let detail = try #require(model.flowRuns[flow.id].value)
        let status = try view.inspect().find(viewWithAccessibilityIdentifier: "flow-run-status-\(flow.id)").text().string()
        #expect(status == "Running implement · iteration (1, 0)")
        for step in detail.steps {
            _ = try view.inspect().find(viewWithAccessibilityIdentifier: "flow-run-step-\(step.execId)")
        }
        #expect(detail.progress.execution == .running)
        #expect(detail.progress.current == 1)
    }

    @Test("A Project's workflow is drawn, is set through lf, and keeps an invalid file visible")
    func waveDefault() async throws {
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")))
        let wave = try #require(roadmap.waves.first).wave
        let catalog = String(decoding: try Data(contentsOf: fixtures.appendingPathComponent("flow_catalog.json")), as: UTF8.self)
        let calls = Recorder()
        let model = PodiumModel(query: RegistryQuery { args, _ in
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
        let model = PodiumModel(query: RegistryQuery(
            start: { args, _ in await started.add(args) },
            run: { args, _ in
                if args.prefix(2) == ["task", "move"] { await started.add(args) }
                return "{}"
            }))
        func view(position: [String: Any]? = nil, outgoing: [Int] = []) throws -> WorkflowView {
            var workflow = try #require(wire["workflow"] as? [String: Any])
            if let position { workflow["position"] = position }
            workflow["outgoing"] = outgoing
            wire["workflow"] = workflow
            let work = try JSONDecoder().decode(TaskWork.self, from: JSONSerialization.data(withJSONObject: wire))
            return WorkflowView(model: model, task: task, wave: wave.wave, workflow: try #require(work.workflow))
        }
        func position(_ view: WorkflowView) throws -> String {
            try view.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-position").text().string()
        }

        // The fixture's `pursue` edge is running: Rust lists no edge to choose beside it.
        let running = try view()
        #expect(try position(running) == "Running pursue · design to demo")
        #expect(throws: (any Error).self) {
            try running.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-run-1")
        }

        // Stopped, it holds the Task and is offered again.
        let exec = "11111111-1111-4111-8111-111111111111"
        let stopped = try view(
            position: ["kind": "edge", "edge": 1, "exec_id": exec, "running": false], outgoing: [1])
        #expect(try position(stopped) == "Stopped on pursue · design to demo")
        #expect(try !stopped.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-run-1").button().isDisabled())

        // At `demo` the only edge runs nothing and is chosen as `end`.
        let waiting = try view(position: ["kind": "node", "node": "demo"], outgoing: [2])
        #expect(try position(waiting) == "Waiting on you at demo · demo")
        let accept = try waiting.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-run-2").button()
        #expect(try accept.labelView().text().string() == "Finish")
        try accept.tap()
        for _ in 0..<200 where await started.calls.isEmpty { try await Task.sleep(for: .milliseconds(5)) }
        #expect(await started.calls == [["-b", "task", "run", task.task.identifier, "end"]])

        // Going back is the same command a person would type.
        await model.moveTask(to: "design", task: task, wave: wave.wave)
        #expect(await started.calls.last == ["task", "move", task.task.identifier, "design"])
    }

    @Test("A Task opens on its primary and lists waiting conversations before working ones")
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
        #expect(groups.entry?.id == "primary")
        #expect(TaskSessionGroups([try session("working"), try session("asks", waiting: true)]).entry?.id == "asks")
        #expect(try session("run").statusLabel == "Session")
    }

    @Test("Loading, unavailable, empty and selected Work have distinct content")
    func workStates() throws {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Unexpected external read") }
        let model = PodiumModel(query: query)
        let view = WorkSurfaceView(model: model)
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "podium-work-loading")

        model.applyFixture(roadmap: .unavailable(lastGood: nil, reason: "offline"),
                           waves: .available([]), processActivity: .loading,
                           workActivity: .loading, repos: [])
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "podium-work-unavailable")
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "podium-work-empty")
        }

        let data = try Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json"))
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        wire["waves"] = []
        let empty = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: wire))
        model.applyFixture(roadmap: .available(empty), waves: .available([]),
                           processActivity: .loading, workActivity: .loading, repos: [])
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "podium-work-empty")

        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: data)
        let wave = try #require(roadmap.waves.first).wave
        model.applyFixture(roadmap: .available(roadmap), waves: .available([wave.toWave()]),
                           processActivity: .loading, workActivity: .loading, repos: [])
        model.navigation.presentation = .full
        let navigator = WorkspaceNavigator(model: model, onOpenSession: { _ in })
        try navigator.inspect()
            .find(viewWithAccessibilityIdentifier: "workspace-wave-\(wave.id)").button().tap()

        #expect(model.selection == .wave(id: wave.id))
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "podium-detail-wave")
        let title = try view.inspect().find(viewWithAccessibilityIdentifier: "wave-title").text().string()
        #expect(title == "Product")
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "podium-work-loading")
        }
    }
}
#endif
