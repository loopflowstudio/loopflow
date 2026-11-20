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
    @Test("Run history preserves nested positions, repeated nodes and failed exits")
    func nestedRunHistory() throws {
        func step(_ id: String, _ iterations: [[UInt32]], outcome: String = "succeeded") -> GraphStepProcess {
            GraphStepProcess(lfProcessId: id, label: "implement", key: 0, iterations: iterations,
                             startedAt: 10, completedAt: 15, outcome: outcome, exitCode: outcome == "failed" ? 1 : 0,
                             error: nil)
        }
        let steps = [step("a", [[0], [1]]), step("b", [[0], [1]]),
                     step("c", [[0, 1]]), step("d", [[0], [1]], outcome: "failed")]
        let groups = GraphStepGroup.groups(steps)
        #expect(groups.map(\.id) == ["a", "c", "d"])
        #expect(groups.flatMap(\.steps) == steps)
        #expect(groups[0].label == "level 1: loop 1 pass 1 · level 2: loop 1 pass 2")
        #expect(groups[1].label == "level 1: loop 1 pass 1, loop 2 pass 2")
        let view = GraphRunHistory(steps: steps)
        for group in groups {
            _ = try view.inspect().find(viewWithAccessibilityIdentifier: "graph-process-loop-\(group.id)")
        }
        let failed = GraphRunStep(step: steps[3])
        #expect(try failed.inspect().find(text: "failed").string() == "failed")
        #expect(try failed.inspect().find(text: "5s").string() == "5s")
        #expect(throws: (any Error).self) { try failed.inspect().find(text: "Summary") }
    }

    @Test("The run page names interactive review independently of completed runs")
    func runPositionReadiness() throws {
        let workflow = TaskGraph(name: "feature", interactiveSteps: [], position: .at(step: "demo"))
        let view = TaskRunPosition(workflow: workflow, execution: nil)
        #expect(try view.inspect().find(text: "demo · available for review").string() == view.label)
    }

    @Test("The repository roadmap shows where its plan lives through the ordinary query")
    func roadmapRepositoryPlan() async throws {
        let roadmap = String(decoding: try Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")), as: UTF8.self)
        let model = WorkModel(query: RegistryQuery { args, cwd in
            if args == ["repo", "tracker", "--json"], cwd == "/src/loopflow" {
                return #"{"tracker":null,"plan":{"git":null,"hub":{"machine":"mini","unsent":1,"records":["3e1c9a07"],"error":null,"failing_since":null,"not_taken":null}},"actions":[]}"#
            }
            if args.first == "roadmap" { return roadmap }
            if args.first == "wave" { return "[]" }
            if args.prefix(2) == ["session", "list"] { return #"{"entries":[],"next":null}"# }
            throw RegistryQueryError("Unexpected command: \(args)")
        }, repoPath: "/src/loopflow")
        await model.refresh()
        #expect(model.repositoryPlan.value?.plan?.hub?.machine == "mini")
        let view = RoadmapView(model: model, onOpenWave: { _ in }, onOpenTask: { _, _ in })
        let planView = try view.inspect().find(RepositoryPlanView.self)
        #expect(try planView.find(text: "Plan: this Machine.").string() == "Plan: this Machine.")
        #expect(try planView.find(text: "Hub mini, 1 unsent.").string() == "Hub mini, 1 unsent.")
        #expect(try planView.find(text: "3e1c9a07").string() == "3e1c9a07")
    }

    @Test("Tracker sync shows queued, sent and conflicted actions, then clears confirmed work")
    func trackerSync() throws {
        let data = try Data(contentsOf: fixtures.appendingPathComponent("tracker_sync.json"))
        let sync = try JSONDecoder().decode(TrackerSync.self, from: data)
        let view = TrackerSyncView(sync: sync, repo: "/r")
        for action in sync.actions {
            #expect(try view.inspect().find(text: action.text).string() == action.text)
        }
        #expect(try view.inspect().findAll(ViewType.Button.self).isEmpty)
        let confirmed = try JSONDecoder().decode(TrackerSync.self, from: Data("{\"actions\":[]}".utf8))
        #expect(try TrackerSyncView(sync: confirmed, repo: "/r").inspect().findAll(ViewType.Text.self).isEmpty)
    }

    @Test("A lost line, a Task's or a Project's, offers Keep theirs and Use mine")
    func trackerSyncAnswers() throws {
        let data = try Data(contentsOf: fixtures.appendingPathComponent("tracker_sync.json"))
        let sync = try JSONDecoder().decode(TrackerSync.self, from: data)
        let lost = try #require(sync.actions.first { $0.phase == .conflicted })
        var answers: [(String, String, LostAnswer)] = []
        let view = TrackerSyncView(sync: sync, repo: "/r")
            .environment(\.answerLost) { repo, action, answer in answers.append((repo, action.id, answer)) }
        let buttons = try view.inspect().findAll(ViewType.Button.self)
        #expect(buttons.count == 2 * sync.actions.filter { $0.phase == .conflicted }.count)
        try view.inspect().find(viewWithAccessibilityIdentifier: "tracker-keep-theirs-\(lost.id)").button().tap()
        try view.inspect().find(viewWithAccessibilityIdentifier: "tracker-use-mine-\(lost.id)").button().tap()
        #expect(answers.map(\.0) == ["/r", "/r"])
        #expect(answers.map(\.1) == [lost.id, lost.id])
        #expect(answers.map(\.2) == [.keepTheirs, .useMine])
    }

    @Test("A builtin Workflow offers Customize beside its interactive steps")
    func customizedWorkflow() async throws {
        var entries = try #require(JSONSerialization.jsonObject(with: Data(contentsOf:
            fixtures.appendingPathComponent("graph_catalog.json"))) as? [[String: Any]])
        entries[0]["name"] = "code"
        entries[0]["source"] = NSNull()
        let catalog = String(decoding: try JSONSerialization.data(withJSONObject: entries), as: UTF8.self)
        let model = WorkModel(query: RegistryQuery { args, _ in
            if args.prefix(2) == ["graph", "list"] { return catalog }
            throw RegistryQueryError("Unexpected command: \(args)")
        }, repoPath: "/tmp/repo")
        await model.loadGraphCatalog()
        let entry = try #require(model.workflowCatalog.value?.first { $0.name == "code" })
        let view = WorkflowCatalogInspector(entry: entry, model: model)
        #expect(try view.inspect().find(text: "builtin Workflow").string() == "builtin Workflow")
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-node-demo")
        _ = try view.inspect().find(button: "Customize")
    }

    @Test("An unplaced Task and its CLI comment thread decode without a display")
    func storedTaskComments() throws {
        struct Proof: Decodable {
            let task: RoadmapTask
            let comments: TaskComments
        }
        let data = try Data(contentsOf: fixtures.appendingPathComponent("local_task.json"))
        let proof = try JSONDecoder().decode(Proof.self, from: data)
        #expect(proof.task.reference.workspace == nil)
        #expect(proof.task.task.id == "task_0123456789ab40008000000000000001")
        #expect(proof.task.runControl.unavailable == nil)
        #expect(TaskCommentsView.author(proof.comments.comments[0].author) == "Fixture Person")
        #expect(TaskCommentsView.readableBody(proof.comments.comments[0].body) == "Preserve the escaped quote")
    }

    @Test("A Task's work, comments and pending delivery arrive from the stream, with no lf read")
    func taskWorkFollowsTheStream() async throws {
        func object(_ name: String) throws -> [String: Any] {
            try #require(JSONSerialization.jsonObject(
                with: Data(contentsOf: fixtures.appendingPathComponent(name))) as? [String: Any])
        }
        func frame(_ part: String, sequence: Int, answers: Int, task: String? = nil,
                   body: Any) throws -> WorkFrame {
            if part == "checkpoint" {
                return WorkFrame(sequence: sequence, answers: answers, unavailable: nil, content: .checkpoint, wire: nil)
            }
            let line: [String: Any] = [
                "answers": answers, "unavailable": NSNull(), "body": body, "read_at": NSNull(),
            ]
            return try WorkFrame.decode(part: try #require(WorkPart(rawValue: part)),
                                        scope: WorkScope(repo: nil, headless: false, task: task, activity: nil, taskListWave: nil),
                                        sequence: sequence, line: JSONSerialization.data(withJSONObject: line))
        }
        let roadmap = try object("roadmap_snapshot.json")
        var work = try object("task_work.json")
        let run = try object("graph_detail.json")
        let planning = roadmap

        let calls = Recorder()
        let feed = Feed()
        let model = WorkModel(query: RegistryQuery(subscribeWork: { await feed.open() }) { args, _ in
            await calls.add(args)
            return ""
        })
        model.confirmMachine(fixtureMachineId)
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
        let task = found.task
        model.select(.task(id: task.id))
        model.syncWorkScope()
        try await eventually { feed.requests.last?.id == 2 }
        guard case .scope(_, let scope) = try #require(feed.requests.last) else {
            Issue.record("selecting a Task scopes the reader to it")
            return
        }
        #expect(scope.task == task.task.identifier)

        // `lf task status`: its work, Graph processes, comments and recorded execution.
        feed.send(try frame("task", sequence: 2, answers: 2, task: task.task.identifier,
                            body: ["overview": work, "graph_processes": [run], "execution": ["work": work,
                                                 "started_by": ["kind": "session", "id": "session_1", "name": "LOO-1 task", "requester": "Maya"],
                                                 "auto": ["state": "on", "at": 1793266000, "by": ["kind": "person", "name": "Maya"]],
                                                 "stack_parent": ["identifier": "LOO-0", "branch": "maya/loo-0-parent", "pr_number": 7]],
                                   "comments": ["identifier": task.task.identifier, "comments": [], "pending_sync": [], "conflicts": [String: String](), "refresh_error": NSNull()]]))
        try await eventually { model.taskWork[task.id].value != nil }
        let shown = try #require(model.taskWork[task.id].value)
        try await eventually { model.taskExecution[task.id].value != nil }
        let diagnostics = try #require(model.taskExecution[task.id].value?.work)
        let view = TaskWorkView(model: model, task: task)
        for id in diagnostics.sessions.map(\.id) {
            _ = try view.inspect().find(viewWithAccessibilityIdentifier: "task-work-\(id)")
        }
        // The Workflow line names who turned auto on, and the Auto toggle reads on.
        let bar = TaskWorkflowBar(model: model, task: task, work: model.taskWork[task.id])
        #expect(bar.live.hasSuffix(" · auto on, by Maya"))
        let control = TaskAutoControl(model: model, task: task)
        #expect(try control.inspect().find(viewWithAccessibilityIdentifier: "task-auto").toggle().isOn())
        // Turning it off says who did and when.
        let line = TaskWorkflowBar.autoLine(Auto(state: .off, at: 1793266380, by: .person(name: "Maya")))
        #expect(line.hasPrefix("auto off by Maya "))
        // A stacked Task names its parent by Task beside its PR line.
        let parent = try #require(model.taskExecution[task.id].value?.stackParent)
        let stacked = TaskStackParentLabel(parent: parent)
        #expect(try stacked.inspect().find(text: "Stacked on LOO-0").string() == "Stacked on LOO-0")
        #expect(stacked.help == "Builds on LOO-0's branch maya/loo-0-parent (PR #7). lf sync main detaches it.")

        // Any Graph process is one line that opens to its id, its launched graph,
        // where it stands and every step it started.
        let log = GraphProcessLog(model: model, taskId: task.id, location: task.reference.workspace?.identity)
        let graph = try #require(shown.graphProcesses.first)
        #expect(throws: (any Error).self) {
            try log.inspect().find(viewWithAccessibilityIdentifier: "graph-process-status-\(graph.id)")
        }
        #expect(throws: (any Error).self) { try log.inspect().find(text: graph.id) }
        let latest = try #require(shown.graphProcesses.last)
        let starter = try log.inspect().find(viewWithAccessibilityIdentifier: "graph-process-started-by-\(latest.id)")
        #expect(try starter.text().string() == "started by Session \"LOO-1 task\" for Maya")
        model.navigation.expandedGraphProcesses.insert(graph.id)
        let detail = try #require(model.graphProcesses[graph.id])
        let expandedRun = GraphProcessView(model: model, graph: graph, run: detail)
        _ = try expandedRun.inspect().find(viewWithAccessibilityIdentifier: "graph-process-id-\(graph.id)")
        let status = try expandedRun.inspect().find(viewWithAccessibilityIdentifier: "graph-process-status-\(graph.id)").text().string()
        #expect(status == "Running implement · pass 2")
        for group in GraphStepGroup.groups(detail.steps) {
            _ = try expandedRun.inspect().find(viewWithAccessibilityIdentifier: "graph-process-loop-\(group.id)")
        }
        #expect(detail.presentation.execution == .running)
        #expect(detail.current == 0)

        // The subscription updates normally; command confirmation is separate.
        let moving = Task { await model.startTask(nil, from: "demo", task: task) }
        try await eventually { feed.requests.last == .checkpoint(id: 3, replay: []) }
        var workflow = try #require(work["graph"] as? [String: Any])
        workflow["position"] = ["kind": "at", "step": "demo"]
        work["graph"] = workflow
        var second = try #require((work["graph_processes"] as? [[String: Any]])?.first)
        second["id"] = "44444444-4444-4444-8444-444444444444"
        second["name"] = "pursue"
        var secondRun = run
        secondRun["entry"] = second
        var incomingComments = try object("task_comments.json")
        incomingComments["identifier"] = task.task.identifier
        incomingComments["pending_sync"] = ["c-1"]
        let moved: [String: Any] = [
            "execution": NSNull(),
            "overview": work.merging(["graph_processes": [try #require((work["graph_processes"] as? [[String: Any]])?.first), second]]) { $1 },
            "graph_processes": [run, secondRun],
            "comments": incomingComments,
        ]
        // A frame read before the write cannot stand for it.
        feed.send(try frame("task", sequence: 4, answers: 2, task: task.task.identifier, body: moved))
        try await eventually { model.taskWork[task.id].value?.graphProcesses.count == 2 }
        feed.send(try frame("task", sequence: 5, answers: 3, task: task.task.identifier, body: moved))
        feed.send(try frame("planning", sequence: 6, answers: 3, body: planning))
        feed.send(try frame("checkpoint", sequence: 7, answers: 3, body: [:]))
        await moving.value
        try await eventually { model.taskWork[task.id].value?.graphProcesses.count == 2 }
        #expect(model.taskWork[task.id].value?.graph?.position == .at(step: "demo"))
        #expect(model.comments[task.id].value?.comments.count == 4)
        #expect(model.comments[task.id].value?.pendingSync == ["c-1"])
        model.navigation.expandedComments.insert(task.id)
        let comments = TaskCommentsView(model: model, task: task)
        _ = try comments.inspect().find(text: "Pending sync")
        _ = try log.inspect().find(viewWithAccessibilityIdentifier: "task-work-44444444-4444-4444-8444-444444444444")
        #expect(model.graphProcesses["44444444-4444-4444-8444-444444444444"]?.entry.name == "pursue")
        #expect(await calls.calls == [
            ["task", "run", task.task.identifier, "--from", "demo", "--json"],
        ])
        feed.send(WorkFrame(sequence: 8, answers: 3, unavailable: "offline", content: .task(nil), wire: nil))
        try await eventually { model.taskWork[task.id].failure == "offline" }
        #expect(model.navigation.notices.current?.text == "The Task couldn't be read: offline")
        _ = try log.inspect().find(viewWithAccessibilityIdentifier: "task-work-44444444-4444-4444-8444-444444444444")
    }

    @Test("Same-name definitions keep separate destinations and a failed refresh retains visible stale data")
    func definitionKindsStayDistinct() async throws {
        let graphs = String(decoding: try Data(contentsOf: fixtures.appendingPathComponent("graph_catalog.json")), as: UTF8.self)
        actor Source {
            var failed = false
            let graphs: String
            init(graphs: String) { self.graphs = graphs }
            func fail() { failed = true }
            func read(_ args: [String]) throws -> String {
                if failed { throw RegistryQueryError("catalog unreadable") }
                return graphs
            }
        }
        let source = Source(graphs: graphs)
        let model = WorkModel(query: RegistryQuery { args, _ in try await source.read(args) })
        await model.loadGraphCatalog()
        let rows = model.paletteRows.filter { $0.title == "feature" }
        #expect(Set(rows.map(\.id)) == [.graph("feature")])
        model.navigation.palette = .workflow("feature")
        let retained = model.workflowCatalog.value
        await source.fail()
        await model.loadGraphCatalog(force: true)
        #expect(model.workflowCatalog.value == retained)
        #expect(model.workflowCatalog.failure == "catalog unreadable")
        #expect(model.graphCatalog.failure == "catalog unreadable")
        #expect(model.paletteIsStale)
        #expect(model.navigation.palette == .workflow("feature"))
        let view = WorkflowCatalogInspector(entry: retained?.first, model: model)
        #expect(model.navigation.notices.current?.text == "Graphs couldn't be read: catalog unreadable")
        #expect(throws: (any Error).self) { try view.inspect().find(text: "catalog unreadable") }
        _ = try view.inspect().find(button: "Refresh")
    }

    @Test("The workflow inspector draws builtins and keeps invalid definitions editable")
    func workflowDefinitions() async throws {
        let catalog = try String(contentsOf: fixtures.appendingPathComponent("graph_catalog.json"), encoding: .utf8)
        let model = WorkModel(query: RegistryQuery { _, _ in catalog })
        await model.loadGraphCatalog()
        func view(_ name: String) -> WorkflowCatalogInspector {
            WorkflowCatalogInspector(entry: model.workflowCatalog.value?.first { $0.name == name }, model: model)
        }
        let builtin = view("feature")
        _ = try builtin.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-node-demo")
        _ = try builtin.inspect().find(text: "builtin Workflow")
        _ = try builtin.inspect().find(button: "Customize")
        let invalid = view("broken")
        _ = try invalid.inspect().find(ViewType.Text.self) { try $0.string().contains("failed to load skill") }
        _ = try invalid.inspect().find(text: ".lf/graphs/broken.yaml")
        _ = try invalid.inspect().find(button: "Edit")
    }

    @Test("A Task's Workflow shows its running run, offers a stopped one again, continues after an interactive step, and starts from any step")
    func taskWorkflow() async throws {
        let planning = try Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json"))
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: planning)
        let wave = try #require(roadmap.waves.first)
        let task = try #require(wave.tasks.items.first { $0.runControl.unavailable == nil && !$0.selectionStates.contains(.auto) })
        var wire = try #require(JSONSerialization.jsonObject(
            with: Data(contentsOf: fixtures.appendingPathComponent("task_work.json"))) as? [String: Any])
        let started = Recorder()
        let model = WorkModel(query: RegistryQuery(
            run: { args, _ in
                if args.prefix(2) == ["plan", "list"] { return String(decoding: planning, as: UTF8.self) }
                if args.prefix(2) == ["task", "run"] || args.prefix(2) == ["task", "complete"]
                    || args.contains("auto") {
                    await started.add(args)
                }
                return "{}"
            }))
        func view(position: [String: Any]? = nil) throws -> TaskWorkflowHeader {
            var workflow = try #require(wire["graph"] as? [String: Any])
            if let position { workflow["position"] = position }
            wire["graph"] = workflow
            let work = try JSONDecoder().decode(TaskOverview.self, from: JSONSerialization.data(withJSONObject: wire))
            return TaskWorkflowHeader(model: model, task: task, work: .available(work))
        }
        func tapped(_ count: Int) async throws -> [String]? {
            for _ in 0..<200 where await started.calls.count < count { try await Task.sleep(for: .milliseconds(5)) }
            return await started.calls.last
        }

        // The fixture's run from review-design is running: nothing to start beside it.
        let running = try view()
        #expect(try running.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-position").text().string()
            == "review-design → demo · running")
        #expect(try running.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-edge-1")
            .accessibilityValue().string() == "Running")
        #expect(throws: (any Error).self) { try running.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-run") }
        #expect(throws: (any Error).self) {
            try running.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-process-1")
        }

        // Stopped, it holds the Task and is offered again.
        let process = "11111111-1111-4111-8111-111111111111"
        let stopped = try view(position: ["kind": "run", "step": "review-design", "at": "implement", "lf_process_id": process, "running": false])
        let again = try stopped.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-process-1")
        #expect(try again.accessibilityValue().string() == "Stopped")
        #expect(try !again.button().isDisabled())
        #expect(try stopped.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-run").button()
            .labelView().text().string() == "Run again")

        // At `demo` the one action continues after it; going into `end` is auto's too.
        let waiting = try view(position: ["kind": "at", "step": "demo"])
        #expect(try waiting.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-position").text().string() == "feature · demo")
        #expect(try waiting.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-node-demo")
            .accessibilityValue().string() == "Current")
        let next = try waiting.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-run").button()
        #expect(try next.labelView().text().string() == "Continue")
        try next.tap()
        #expect(try await tapped(1) == ["task", "run", task.task.identifier, "--json"])
        // The Auto toggle sits in the Workflow bar, not in the detail page's Workflow header.
        #expect(throws: (any Error).self) {
            try waiting.inspect().find(viewWithAccessibilityIdentifier: "task-auto")
        }
        // Off until someone turns it on with `lf task auto on`.
        let control = TaskAutoControl(model: model, task: task)
        let toggle = try control.inspect().find(viewWithAccessibilityIdentifier: "task-auto").toggle()
        #expect(try !toggle.isOn())
        try toggle.tap()
        #expect(try await tapped(2) == ["--task", task.task.identifier, "task", "auto", "on", "--json"])

        // At its end nothing remains to run.
        let ended = try view(position: ["kind": "at", "step": "end"])
        #expect(throws: (any Error).self) { try ended.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-run") }

        // A Task that has taken up no workflow uses its Wave or repository default.
        wire["graph"] = NSNull()
        let unstarted = TaskWorkflowHeader(model: model, task: task, work: .available(
            try JSONDecoder().decode(TaskOverview.self, from: JSONSerialization.data(withJSONObject: wire))))
        try unstarted.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-start").button().tap()
        #expect(try await tapped(3) == ["task", "run", task.task.identifier, "--json"])

        // Going back is the same command a person would type; Start from… lists `start` and each interactive step.
        for step in ["start", "review-design", "demo"] {
            _ = try waiting.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-move-\(step)")
        }
        try waiting.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-move-review-design").button().tap()
        #expect(try await tapped(4) == ["task", "run", task.task.identifier, "--from", "review-design", "--json"])

        // Completion has its own operation and does not move the Workflow.
        model.completeTask(task)
        for _ in 0..<200 where await started.calls.last?.contains("complete") != true { try await Task.sleep(for: .milliseconds(5)) }
        #expect(await started.calls.last == ["task", "complete", task.task.identifier, "--json"])

        // Auto on says it was requested, never that it shipped; its Undo is auto off.
        model.turnAutoOn(task)
        let notice = try #require(model.navigation.notices.current)
        #expect(notice.text == "\(task.task.identifier) Auto on requested")
        let view = NoticeView(center: model.navigation.notices)
        let sent = await started.calls.count
        try view.inspect().find(viewWithAccessibilityIdentifier: "notice-action").button().tap()
        for _ in 0..<200 where await started.calls.count == sent { try await Task.sleep(for: .milliseconds(5)) }
        #expect(await started.calls.last == ["task", "auto", "off", task.task.identifier, "--json"])
    }

    @Test("Task lifecycle controls remain available without a Workflow", arguments: ["unstarted", "completed", "canceled"])
    func taskLifecycleControls(state: String) throws {
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")))
        let wave = try #require(roadmap.waves.first)
        let original = try #require(wave.tasks.items.first)
        let root = try #require(JSONSerialization.jsonObject(
            with: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json"))) as? [String: Any])
        let tasks = try #require(root["tasks"] as? [String: Any])
        var row = try #require((tasks["items"] as? [[String: Any]])?.first)
        var planning = try #require(row["task"] as? [String: Any])
        planning["state"] = state
        planning["completed"] = state == "completed"
        row["task"] = planning
        row["runtime"] = NSNull()
        let lifecycle = state == "completed" ? "done" : state == "canceled" ? "abandoned" : "unstarted"
        row["lifecycle"] = lifecycle
        let refusal: Any = state == "unstarted" ? NSNull() : "Reopen the Task"
        row["run_control"] = ["unavailable": refusal]
        let task = try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row))
        #expect(task.id == original.id)
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Rendering must not mutate") })
        let view = TaskWorkflowHeader(model: model, task: task,
            work: .available(TaskOverview(sessions: [], graphProcesses: [], graph: nil)))
        if state == "unstarted" {
            _ = try view.inspect().find(button: "Complete")
            // The fixture Task has no owner, so the assignment control offers Claim.
            let assignment = try view.inspect().find(viewWithAccessibilityIdentifier: "task-claim-unassign")
            #expect(try assignment.button().labelView().text().string() == "Claim")
            _ = try view.inspect().find(button: "Abandon…")
        } else {
            _ = try view.inspect().find(button: "Reopen")
            #expect(throws: (any Error).self) { try view.inspect().find(button: "Complete") }
        }
    }

    @Test("A Task with auto on reads on and switches off with lf task auto off")
    func autoOnRow() async throws {
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")))
        let task = try #require(snapshot.waves.first?.tasks.items.first { $0.selectionStates.contains(.auto) })
        let sent = Recorder()
        let model = WorkModel(query: RegistryQuery(run: { args, _ in
            if args.prefix(2) == ["task", "auto"] { await sent.add(args) }
            return "{}"
        }))
        // Rust's refusal to turn it on again never disables turning it off.
        let toggle = try TaskAutoControl(model: model, task: task).inspect()
            .find(viewWithAccessibilityIdentifier: "task-auto").toggle()
        #expect(try toggle.isOn())
        try toggle.tap()
        for _ in 0..<200 where await sent.calls.isEmpty { try await Task.sleep(for: .milliseconds(5)) }
        #expect(await sent.calls.last == ["task", "auto", "off", task.task.identifier, "--json"])
    }

    @Test("A workflow is drawn as a graph: circles, described interactive steps, a loop arc and an arrow per run")
    func workflowGraph() async throws {
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")))
        let wave = try #require(roadmap.waves.first)
        let task = try #require(wave.tasks.items.first { $0.runControl.unavailable == nil && !$0.selectionStates.contains(.auto) })
        let started = Recorder()
        let model = WorkModel(query: RegistryQuery(run: { args, _ in
            if args.prefix(2) == ["task", "run"] { await started.add(args) }
            return "{}"
        }))
        var wire = try #require(JSONSerialization.jsonObject(
            with: Data(contentsOf: fixtures.appendingPathComponent("task_work.json"))) as? [String: Any])
        func view(_ position: [String: Any]) throws -> TaskWorkflowHeader {
            var workflow = try #require(wire["graph"] as? [String: Any])
            workflow["position"] = position
            wire["graph"] = workflow
            let work = try JSONDecoder().decode(TaskOverview.self, from: JSONSerialization.data(withJSONObject: wire))
            return TaskWorkflowHeader(model: model, task: task, work: .available(work))
        }

        // `feature`, with the Task waiting at `review-design`.
        let feature = try view(["kind": "at", "step": "review-design"])
        for node in ["start", "review-design", "demo", "end"] {
            let drawn = try feature.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-node-\(node)")
            #expect(try drawn.accessibilityValue().string() == (node == "review-design" ? "Current" : ""))
        }
        _ = try feature.inspect().find(text: "you review the plan")
        _ = try feature.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-edge-0")
        _ = try feature.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-edge-2")
        // The run `lf task run` would start next is the diagram's button.
        let next = try feature.inspect().find(viewWithAccessibilityIdentifier: "task-workflow-process-1").button()
        try next.tap()
        for _ in 0..<200 where await started.calls.isEmpty { try await Task.sleep(for: .milliseconds(5)) }
        #expect(await started.calls == [["task", "run", task.task.identifier, "--json"]])

        // A definition: a step that returns to an earlier one draws a loop over
        // the interactive step it comes back to.
        let catalog = try JSONDecoder().decode([GraphCatalogEntry].self,
            from: Data(contentsOf: fixtures.appendingPathComponent("graph_catalog.json")))
        let definition = try #require(catalog.first?.compiled.map(WorkflowDefinition.init))
        #expect(definition.steps.map(\.name) == ["demo"])
        #expect(definition.runs.map { [$0.from, $0.to, $0.name] } == [
            ["start", "demo", "task-design"], ["demo", "end", "loop-or-next"], ["demo", "demo", "loop-or-next"],
        ])
        let drawn = WorkflowDiagramLayout(nodes: definition.steps, edges: definition.runs)
        #expect(drawn.arrows.map(\.route) == [.straight, .straight, .loop(level: 0)])
        #expect(drawn.boxes.map(\.terminal) == [true, false, true])
        let demo = try #require(drawn.boxes.first { $0.name == "demo" })
        #expect(drawn.arrows[2].label.y < demo.frame.minY)
        #expect(abs(drawn.arrows[2].label.x - demo.frame.midX) < 1)

        // A run that goes back, or passes over a step, takes its own lane below the row.
        let step = { TaskGraph.InteractiveStep(name: $0, skill: "demo", description: nil) }
        let run = { WorkflowDefinition.Run(from: $0, to: $1, name: $2) }
        let lanes = WorkflowDiagramLayout(nodes: [step("a"), step("b")], edges: [
            run("start", "a", "one"), run("a", "b", "two"), run("b", "a", "back"), run("start", "b", "skip"),
        ])
        #expect(lanes.arrows.map(\.route) == [.straight, .straight, .lane(level: 0), .lane(level: 1)])
        #expect(lanes.arrows[2].label.y > lanes.boxes[1].frame.maxY)
        #expect(lanes.arrows[3].label.y > lanes.arrows[2].label.y)
        #expect(lanes.size.height > lanes.arrows[3].label.y)
    }

    @Test("A Task lists available conversations before working ones, its primary leading each")
    func taskSessions() throws {
        let base = try #require(JSONSerialization.jsonObject(
            with: Data(contentsOf: fixtures.appendingPathComponent("session.json"))) as? [String: Any])
        func session(_ id: String, available: Bool = false, primary: Bool = false, state: String = "unknown") throws -> SessionRecord {
            var value = base
            value["id"] = id
            value["state"] = state
            value["availability"] = available ? "available" : NSNull()
            value["task_primary"] = primary
            return try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: value))
        }
        let groups = TaskSessionGroups([
            try session("working"), try session("closed", available: true, state: "closed"),
            try session("asks", available: true), try session("primary", primary: true),
        ])
        #expect(groups.available.map(\.id) == ["asks"])
        #expect(groups.working.map(\.id) == ["primary", "working"])
        #expect(try session("run").statusLabel == "Session")
    }

    @Test("Loading, unread, empty and selected Work have distinct content")
    func workStates() async throws {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Unexpected external read") }
        let model = WorkModel(query: query)
        let view = WorkSurfaceView(model: model)
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "loopflow-work-loading")

        model.applyFixture(roadmap: .unavailable(lastGood: nil, reason: "offline"),
                           waves: .available([]),
                           workActivity: .loading, repos: [])
        // A failed first read stays empty: the Notice says why.
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "loopflow-work-loading")
        }
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "loopflow-work-empty")
        }

        let data = try Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json"))
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        wire["waves"] = []
        wire["tasks"] = ["state": "ok", "items": [], "truncated": false]
        let empty = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: wire))
        model.applyFixture(roadmap: .available(empty), waves: .available([]),
                           workActivity: .loading, repos: [])
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "loopflow-work-empty")

        var saved = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var tasks = try #require(saved["tasks"] as? [String: Any])
        var items = try #require(tasks["items"] as? [[String: Any]])
        var planning = try #require(items[0]["task"] as? [String: Any])
        planning["sync"] = ["actions": [[
            "id": "pending-title", "seq": 1, "owner_kind": "task", "owner_id": "task", "kind": "field",
            "change": "title", "subject": "task_title", "phase": "queued", "tracker": "linear",
            "error": NSNull(), "local_value": "Saved title", "tracker_value": NSNull(),
        ]]]
        items[0]["task"] = planning
        tasks["items"] = items
        saved["tasks"] = tasks
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: saved))
        let wave = try #require(roadmap.waves.first).wave
        model.applyFixture(roadmap: .available(roadmap), waves: .available([wave.toWave()]),
                           workActivity: .loading, repos: [])
        model.applySelectionPlans([fixtureViewPlan(owner: .init(repository: "repo", kind: .repository, id: "repo"), members: [
            fixtureViewContent(work: .init(repository: "repo", kind: .wave, id: wave.id), title: wave.displayName)
        ])], reason: "")
        let navigator = WorkNavigator(model: model, onOpenSession: { _ in })
        try navigator.inspect()
            .find(viewWithAccessibilityIdentifier: "work-wave-\(wave.id)").button().tap()

        // The clicked row darkens before the selection moves it.
        for _ in 0..<50 where model.selection != .wave(id: wave.id) { try await Task.sleep(for: .milliseconds(20)) }
        #expect(model.selection == .wave(id: wave.id))
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "wave-backlog")
        let title = try view.inspect().find(viewWithAccessibilityIdentifier: "wave-title").text().string()
        #expect(title == "Product")
        let task = try #require(roadmap.waves.first?.tasks.items.first)
        model.select(.task(id: task.id))
        let pending = try view.inspect().find(viewWithAccessibilityIdentifier: "tracker-sync-pending-title").text().string()
        #expect(pending == "title · queued")
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "loopflow-work-loading")
        }
    }
}
#endif
