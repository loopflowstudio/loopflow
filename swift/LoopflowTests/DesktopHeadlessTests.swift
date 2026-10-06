#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

// Inspect production view bodies and invoke controls directly. No NSApplication,
// NSWindow, window server connection, screenshot capture, or UI automation.
private actor Recorder {
    private(set) var calls: [[String]] = []
    func add(_ args: [String]) { calls.append(args) }
}

@Suite("Desktop without a display")
@MainActor
struct DesktopHeadlessTests {
    @Test("Task work renders checkout conversations, Flows and mechanical Execs")
    func taskWorkInventory() async throws {
        let fixtures = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
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

        // Any Flow run opens to its launched graph, where it stands and every step it started.
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

    @Test("A Task's workflow shows its running edge, then offers the edges leaving the stage it waits at")
    func taskWorkflow() async throws {
        let fixtures = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")))
        let wave = try #require(roadmap.waves.first)
        let task = try #require(wave.tasks.items.first { $0.flow.control(.start)?.unavailable == nil })
        var wire = try #require(JSONSerialization.jsonObject(
            with: Data(contentsOf: fixtures.appendingPathComponent("task_work.json"))) as? [String: Any])
        let started = Recorder()
        let model = PodiumModel(query: RegistryQuery(start: { args, _ in await started.add(args) }, run: { _, _ in "{}" }))
        func view() throws -> TaskWorkflowView {
            let work = try JSONDecoder().decode(TaskWork.self, from: JSONSerialization.data(withJSONObject: wire))
            return TaskWorkflowView(model: model, task: task, wave: wave.wave, workflow: try #require(work.workflow))
        }

        // The fixture's `pursue` edge is running: nothing else can be started beside it.
        let running = try view()
        #expect(try running.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-position").text().string()
            == "Running pursue · design to demo")
        #expect(try running.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-run-1").button().isDisabled())

        // Once it finishes the Task waits at `demo`, whose only edge runs nothing and enters `end`.
        var workflow = try #require(wire["workflow"] as? [String: Any])
        workflow["position"] = ["kind": "stage", "stage": "demo"]
        wire["workflow"] = workflow
        let waiting = try view()
        #expect(try waiting.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-position").text().string()
            == "Waiting on you at demo · demo")
        #expect(throws: (any Error).self) {
            try waiting.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-run-1")
        }
        try waiting.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-run-2").button().tap()
        for _ in 0..<200 where await started.calls.isEmpty { try await Task.sleep(for: .milliseconds(5)) }
        #expect(await started.calls == [["-b", "task", "run", task.task.identifier, "end"]])
    }

    @Test("A Task opens on its primary and lists waiting conversations before working ones")
    func taskSessions() throws {
        let fixtures = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
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

        let fixture = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        let data = try Data(contentsOf: fixture)
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
