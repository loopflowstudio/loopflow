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
    @Test("Git planning shows selection, unknown changes and held records without claiming convergence")
    func peerPlanningStatus() throws {
        let data = try Data(contentsOf: fixtures.appendingPathComponent("peer_planning_status.json"))
        let statuses = try JSONDecoder().decode([PeerPlanningStatus].self, from: data)
        let view = PeerPlanningView(reading: .unavailable(lastGood: statuses, reason: "reader exited"))
        for text in ["Showing the last sync status", "Local changes unknown", "Local changes pending",
                     "Selected for future root Waves", "Held task retained-task: retained projection conflict",
                     "Publication: unconfirmed (attempted-publication)", "Fetched: newer-fetch",
                     "Retained import: retained-import", "Sync status unavailable: reader exited",
                     "Local only; not selected", "Retained reference: project private-parent",
                     "issue_title: retained alternative", "\"Retained private title\"",
                     "planning_assignee: candidate, not confirmed", "\"Maya\"",
                     "Mutation: comment-loser; author: Maya"] {
            #expect(try view.inspect().find(text: text).string() == text)
        }
        let local = PeerPlanningView(reading: .available([]))
        #expect(try local.inspect().find(text: "Future root Waves: local").string() == "Future root Waves: local")
        let unknown = PeerPlanningView(reading: .unavailable(lastGood: nil, reason: "store unavailable"))
        #expect(try unknown.inspect().findAll(ViewType.Text.self).allSatisfy {
            try $0.string() != "Future root Waves: local"
        })
    }

    @Test("The repository roadmap reads Git planning through the ordinary query")
    func roadmapPeerPlanning() async throws {
        let data = try Data(contentsOf: fixtures.appendingPathComponent("peer_planning_status.json"))
        let status = String(decoding: data, as: UTF8.self)
        let roadmap = String(decoding: try Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")), as: UTF8.self)
        let model = WorkModel(query: RegistryQuery { args, cwd in
            if args == ["planning", "status", "--json"], cwd == "/src/loopflow" {
                return "{\"destinations\":\(status)}"
            }
            if args.first == "roadmap" { return roadmap }
            if args.first == "wave" { return "[]" }
            if args.first == "session" { return #"{"entries":[],"next":null}"# }
            throw RegistryQueryError("Unexpected command: \(args)")
        }, repoPath: "/src/loopflow")
        await model.refresh()
        #expect(model.peerPlanning.value?.count == 2)
        let view = RoadmapView(model: model, onOpenWave: { _ in })
        let statusView = try view.inspect().find(PeerPlanningView.self)
        #expect(try statusView.find(text: "Local changes unknown").string() == "Local changes unknown")
    }

    @Test("Planning sync shows saves, uncertainty and retained losing edits, then clears settled work")
    func planningSync() throws {
        let data = try Data(contentsOf: fixtures.appendingPathComponent("planning_sync.json"))
        let sync = try JSONDecoder().decode(PlanningSyncStatus.self, from: data)
        let view = PlanningSyncView(sync: sync)
        for change in sync.changes {
            #expect(try view.inspect().find(text: change.text).string() == change.text)
        }
        for connected in [true, false] {
            let settled = try JSONDecoder().decode(PlanningSyncStatus.self, from: Data("{\"connected\":\(connected),\"changes\":[]}".utf8))
            #expect(try PlanningSyncView(sync: settled).inspect().findAll(ViewType.Text.self).isEmpty)
        }
    }

    @Test("Workflows render and save through the store without creating a repository file")
    func storedWorkflow() async throws {
        var roadmap = try #require(JSONSerialization.jsonObject(with: Data(contentsOf:
            fixtures.appendingPathComponent("roadmap_snapshot.json"))) as? [String: Any])
        var waves = try #require(roadmap["waves"] as? [[String: Any]])
        var waveObject = try #require(waves[0]["wave"] as? [String: Any])
        waveObject["name"] = "inbox"
        waves[0]["wave"] = waveObject
        roadmap["waves"] = waves
        let snapshot = String(decoding: try JSONSerialization.data(withJSONObject: roadmap), as: UTF8.self)
        let wave = try JSONDecoder().decode(WaveSnapshot.self, from: JSONSerialization.data(withJSONObject: waveObject))
        var entries = try #require(JSONSerialization.jsonObject(with: Data(contentsOf:
            fixtures.appendingPathComponent("workflow_catalog.json"))) as? [[String: Any]])
        entries[0]["name"] = "code"
        entries[0]["source"] = "stored"
        let catalog = String(decoding: try JSONSerialization.data(withJSONObject: entries), as: UTF8.self)
        actor Source {
            var content = "nodes: {}\nedges: []\n"
            func save(_ content: String) throws {
                if content == "invalid" { throw RegistryQueryError("Invalid workflow") }
                self.content = content
            }
        }
        let source = Source()
        let model = WorkModel(query: RegistryQuery(runWithInput: { args, _, content in
            guard args.prefix(3) == ["project", "workflow", "set"], args.contains("code") else {
                throw RegistryQueryError("Unexpected write: \(args)")
            }
            try await source.save(content)
            return ""
        }, run: { args, _ in
            if args.first == "roadmap" { return snapshot }
            if args.prefix(3) == ["project", "workflow", "list"] { return catalog }
            if args.prefix(3) == ["project", "workflow", "source"] { return await source.content }
            throw RegistryQueryError("Unexpected command: \(args)")
        }))
        await model.refresh()
        model.select(.wave(id: wave.id))
        await model.loadWorkflowCatalog()
        let view = WaveWorkflowView(model: model, wave: wave, name: "code")
        #expect(try view.inspect().find(viewWithAccessibilityIdentifier: "wave-workflow-source").text().string() == "Stored in this Wave")
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-node-demo")
        _ = try view.inspect().find(button: "Edit")
        let updated = "nodes: {review: demo}\nedges: []\n"
        try await model.saveWorkflow("code", content: updated, wave: wave)
        #expect(try await model.workflowSource("code", wave: wave) == updated)
        do {
            try await model.saveWorkflow("code", content: "invalid", wave: wave)
            Issue.record("Invalid content must not replace the stored workflow")
        } catch { #expect(error.localizedDescription == "Invalid workflow") }
        #expect(try await model.workflowSource("code", wave: wave) == updated)
    }

    @Test("An unplaced Task and its CLI comment thread render without a display")
    func storedTaskComments() async throws {
        struct Proof: Decodable {
            let task: RoadmapTask
            let comments: TaskComments
        }
        let data = try Data(contentsOf: fixtures.appendingPathComponent("local_task.json"))
        let proof = try JSONDecoder().decode(Proof.self, from: data)
        let object = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let thread = String(decoding: try JSONSerialization.data(withJSONObject: #require(object["comments"])), as: UTF8.self)
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")))
        let wave = try #require(roadmap.waves.first).wave
        let model = WorkModel(query: RegistryQuery { args, _ in
            guard args.prefix(2) == ["task", "comment"] else {
                throw RegistryQueryError("Unexpected command: \(args)")
            }
            return thread
        })
        #expect(proof.task.reference.workspace == nil)
        #expect(proof.task.task.id == "task_0123456789ab40008000000000000001")
        #expect(proof.task.runControl.unavailable == nil)
        await model.loadComments(task: proof.task, wave: wave)
        #expect(model.comments[proof.task.id].value == proof.comments)
        model.navigation.expandedComments.insert(proof.task.id)
        let view = TaskCommentsView(model: model, task: proof.task, wave: wave)
        _ = try view.inspect().find(text: "Fixture Person")
        _ = try view.inspect().find(viewWithAccessibilityIdentifier:
            "task-comment-00000000-0000-4000-8000-000000000001")
        #expect(TaskCommentsView.readableBody(proof.comments.comments[0].body) == "Preserve the escaped quote")
    }

    @Test("A Task's work, comments and pending delivery arrive from the stream, with no lf read")
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
        let (oldReads, releaseOldReads) = AsyncStream<Void>.makeStream()
        let feed = Feed()
        let model = WorkModel(query: RegistryQuery(watchWork: { await feed.open() }) { args, _ in
            await calls.add(args)
            if args.prefix(2) == ["task", "comment"] {
                for await _ in oldReads { }
                return #"{"identifier":"old","comments":[],"pending_sync":[],"conflicts":{},"refresh_error":null}"#
            }
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
                            body: ["task": task.task.identifier, "work": work, "flow_processes": [run], "comments": ["identifier": task.task.identifier, "comments": [], "pending_sync": [], "conflicts": [String: String](), "refresh_error": NSNull()]]))
        try await eventually { model.taskWork[task.id].value != nil }
        let shown = try #require(model.taskWork[task.id].value)
        let view = TaskWorkView(model: model, task: task)
        for id in shown.sessions.map(\.id) + shown.processes.map(\.id) {
            _ = try view.inspect().find(viewWithAccessibilityIdentifier: "task-work-\(id)")
        }

        // Any Flow process is one line that opens to its id, its launched graph,
        // where it stands and every step it started.
        let log = FlowProcessLog(model: model, taskId: task.id)
        let flow = try #require(shown.flowProcesses.first)
        #expect(throws: (any Error).self) {
            try log.inspect().find(viewWithAccessibilityIdentifier: "flow-process-status-\(flow.id)")
        }
        #expect(throws: (any Error).self) { try log.inspect().find(text: flow.id) }
        model.navigation.expandedFlowProcesses.insert(flow.id)
        _ = try log.inspect().find(viewWithAccessibilityIdentifier: "flow-process-id-\(flow.id)")
        let detail = try #require(model.flowProcesses[flow.id])
        let status = try log.inspect().find(viewWithAccessibilityIdentifier: "flow-process-status-\(flow.id)").text().string()
        #expect(status == "Running implement · pass 2")
        for step in detail.steps {
            _ = try log.inspect().find(viewWithAccessibilityIdentifier: "flow-process-step-\(step.processLfid)")
        }
        #expect(detail.presentation.execution == .running)
        #expect(detail.current == 0)

        let oldRead = Task { await model.loadComments(task: task, wave: wave) }
        try await eventually { model.comments.inFlight.contains(task.id) }

        // Desktop's own write asks the reader again and shows what it answers.
        let moving = Task { await model.moveTask(to: "demo", task: task, wave: wave) }
        try await eventually { feed.requests.last == .refresh(id: 3) }
        var workflow = try #require(work["workflow"] as? [String: Any])
        workflow["position"] = ["kind": "node", "node": "demo"]
        work["workflow"] = workflow
        var second = try #require((work["flow_processes"] as? [[String: Any]])?.first)
        second["id"] = "44444444-4444-4444-8444-444444444444"
        second["name"] = "pursue"
        var secondRun = run
        secondRun["entry"] = second
        var incomingComments = try object("task_comments.json")
        incomingComments["identifier"] = task.task.identifier
        incomingComments["pending_sync"] = ["c-1"]
        let moved: [String: Any] = [
            "task": task.task.identifier,
            "work": work.merging(["flow_processes": [try #require((work["flow_processes"] as? [[String: Any]])?.first), second]]) { $1 },
            "flow_processes": [run, secondRun],
            "comments": incomingComments,
        ]
        // A frame read before the write cannot stand for it.
        feed.send(try frame("task", sequence: 3, answers: 2, body: moved))
        try await Task.sleep(for: .milliseconds(50))
        #expect(model.taskWork[task.id].value?.flowProcesses.count == 1)
        feed.send(try frame("task", sequence: 4, answers: 3, body: moved))
        feed.send(try frame("planning", sequence: 5, answers: 3, body: planning))
        await moving.value
        try await eventually { model.taskWork[task.id].value?.flowProcesses.count == 2 }
        #expect(model.taskWork[task.id].value?.workflow?.position == .node("demo"))
        releaseOldReads.finish()
        await oldRead.value
        #expect(model.comments[task.id].value?.comments.count == 3)
        #expect(model.comments[task.id].value?.pendingSync == ["c-1"])
        model.navigation.expandedComments.insert(task.id)
        let comments = TaskCommentsView(model: model, task: task, wave: wave)
        _ = try comments.inspect().find(text: "Pending sync")
        _ = try log.inspect().find(viewWithAccessibilityIdentifier: "task-work-44444444-4444-4444-8444-444444444444")
        #expect(model.flowProcesses["44444444-4444-4444-8444-444444444444"]?.entry.name == "pursue")
        #expect(await calls.calls == [
            ["task", "comment", task.id, "--wave", wave.name, "--json"],
            ["task", "move", task.task.identifier, "demo"],
        ])
    }

    @Test("Same-name definitions keep separate destinations and a failed refresh retains visible stale data")
    func definitionKindsStayDistinct() async throws {
        let flows = String(decoding: try Data(contentsOf: fixtures.appendingPathComponent("flow_catalog.json")), as: UTF8.self)
        var workflows = try #require(JSONSerialization.jsonObject(with: Data(contentsOf:
            fixtures.appendingPathComponent("workflow_catalog.json"))) as? [[String: Any]])
        workflows[0]["name"] = "feature"
        let text = String(decoding: try JSONSerialization.data(withJSONObject: workflows), as: UTF8.self)
        actor Source {
            var failed = false
            let flows: String
            let workflows: String
            init(flows: String, workflows: String) { self.flows = flows; self.workflows = workflows }
            func fail() { failed = true }
            func read(_ args: [String]) throws -> String {
                if failed { throw RegistryQueryError("catalog unreadable") }
                return args.first == "project" ? workflows : flows
            }
        }
        let source = Source(flows: flows, workflows: text)
        let model = WorkModel(query: RegistryQuery { args, _ in try await source.read(args) })
        await model.loadFlowCatalog()
        await model.loadWorkflowCatalog()
        let rows = model.paletteRows.filter { $0.title == "feature" }
        #expect(Set(rows.map(\.id)) == [.flow("feature"), .workflow("feature")])
        model.navigation.palette = .workflow("feature")
        let retained = model.workflowCatalog.value
        await source.fail()
        await model.loadWorkflowCatalog(force: true)
        #expect(model.workflowCatalog.value == retained)
        #expect(model.workflowCatalog.errorMessage == "catalog unreadable")
        #expect(model.paletteIsStale)
        #expect(model.navigation.palette == .workflow("feature"))
        let view = WorkflowCatalogInspector(entry: retained?.first, model: model)
        _ = try view.inspect().find(text: "catalog unreadable")
        _ = try view.inspect().find(button: "Refresh")
    }

    @Test("A Project's workflow is drawn, is set through lf, and keeps an invalid file visible")
    func waveDefault() async throws {
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")))
        let wave = try #require(roadmap.waves.first).wave
        let catalog = String(decoding: try Data(contentsOf: fixtures.appendingPathComponent("workflow_catalog.json")), as: UTF8.self)
        let calls = Recorder()
        let model = WorkModel(query: RegistryQuery { args, _ in
            await calls.add(args)
            if args.prefix(3) == ["project", "workflow", "list"] { return catalog }
            if args.prefix(3) == ["project", "workflow", "set"], args.last == "broken" {
                throw RegistryQueryError("broken does not load")
            }
            if args.first == "roadmap" { return String(decoding: try Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")), as: UTF8.self) }
            return "{}"
        })
        await model.refresh()
        await model.loadWorkflowCatalog()
        func view(_ name: String) -> WaveWorkflowView { WaveWorkflowView(model: model, wave: wave, name: name) }

        // A builtin workflow is drawn before any Task has run it, and editing it is an explicit Customize.
        let builtin = view("code")
        _ = try builtin.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-node-demo")
        #expect(try builtin.inspect().find(viewWithAccessibilityIdentifier: "wave-workflow-source").text().string() == "builtin workflow")
        #expect(try builtin.inspect().find(viewWithAccessibilityIdentifier: "wave-workflow-edit").button().labelView().text().string() == "Customize")
        try builtin.inspect().find(viewWithAccessibilityIdentifier: "wave-workflow-edit").button().tap()

        // An invalid repository file stays listed with its reason and can still be opened.
        let invalid = view("proof")
        #expect(try invalid.inspect().find(viewWithAccessibilityIdentifier: "wave-workflow-invalid").text().string().contains("unreachable"))
        #expect(try invalid.inspect().find(viewWithAccessibilityIdentifier: "wave-workflow-source").text().string() == ".lf/workflows/proof.yaml")
        #expect(try invalid.inspect().find(viewWithAccessibilityIdentifier: "wave-workflow-edit").button().labelView().text().string() == "Edit")

        // Setting the workflow changes only that line; a refusal is shown on the Wave.
        await model.setWorkflow("code", wave: wave)
        #expect(await calls.calls.contains(["project", "workflow", "set", roadmap.waves[0].projects.currentProject!.id, "code"]))
        #expect(model.workflowErrors[wave.id] == nil)
        await model.setWorkflow("broken", wave: wave)
        #expect(try view("code").inspect().find(viewWithAccessibilityIdentifier: "wave-workflow-error").text().string() == "broken does not load")
    }

    @Test("A Task's Workflow shows its running edge, offers a stopped edge again, and moves by choice or by node")
    func taskWorkflow() async throws {
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")))
        let wave = try #require(roadmap.waves.first)
        let task = try #require(wave.tasks.items.first { $0.runControl.unavailable == nil })
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
            try running.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-process-2")
        }

        // Stopped, it holds the Task and is offered again with the other edge leaving its node.
        let process = "11111111-1111-4111-8111-111111111111"
        let stopped = try view(
            position: ["kind": "edge", "edge": 2, "process_lfid": process, "running": false], outgoing: [1, 2])
        let again = try stopped.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-process-2")
        #expect(try again.accessibilityValue().string() == "Stopped")
        #expect(try !again.button().isDisabled())
        _ = try stopped.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-process-1")

        // At `demo` the edges leaving it are the buttons, named by what they run.
        let waiting = try view(position: ["kind": "node", "node": "demo"], outgoing: [3, 4])
        #expect(throws: (any Error).self) {
            try waiting.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-position")
        }
        #expect(try waiting.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-node-demo")
            .accessibilityValue().string() == "Current")
        let ship = try waiting.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-process-4").button()
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
        let task = try #require(wave.tasks.items.first { $0.runControl.unavailable == nil })
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
            with: Data(contentsOf: fixtures.appendingPathComponent("workflow_catalog.json"))) as? [[String: Any]])
        let research = try #require(catalog.first { $0["name"] as? String == "research" }?["workflow"] as? [String: Any])
        let findings = try view {
            $0.merge(research) { _, definition in definition }
            $0["position"] = ["kind": "node", "node": "findings"]
            $0["outgoing"] = [1, 2]
        }
        #expect(try findings.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-node-findings")
            .accessibilityValue().string() == "Current")
        _ = try findings.inspect().find(text: "you read the findings")
        let finish = try findings.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-process-2").button()
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

        var saved = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var waves = try #require(saved["waves"] as? [[String: Any]])
        var tasks = try #require(waves[0]["tasks"] as? [String: Any])
        var items = try #require(tasks["items"] as? [[String: Any]])
        var planning = try #require(items[0]["task"] as? [String: Any])
        planning["sync"] = ["connected": true, "changes": [[
            "id": "pending-title", "field": "title", "state": "pending",
            "local_value": "Saved title", "linear_value": NSNull(), "error": NSNull(),
        ]]]
        items[0]["task"] = planning
        tasks["items"] = items
        waves[0]["tasks"] = tasks
        saved["waves"] = waves
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: saved))
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
        let task = try #require(roadmap.waves.first?.tasks.items.first)
        model.select(.task(id: task.id))
        let pending = try view.inspect().find(viewWithAccessibilityIdentifier: "planning-sync-pending-title").text().string()
        #expect(pending == "Saved locally; pending Linear sync · title")
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "loopflow-work-loading")
        }
    }
}
#endif
