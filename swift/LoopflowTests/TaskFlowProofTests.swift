#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
import ViewInspector
#if canImport(GhosttyKit)
import GhosttyKit
#endif
@testable import Loopflow
@testable import LoopflowMac

private let fixtureRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    .deletingLastPathComponent().deletingLastPathComponent()
    .appendingPathComponent("tests/fixtures/dto")

private func fixture(_ name: String) throws -> Data {
    try Data(contentsOf: fixtureRoot.appendingPathComponent(name))
}

@MainActor
private func ax(_ element: NSObject, _ property: String) -> Any? {
    let key = property == "Focused" ? "isAccessibilityFocused" : "accessibility" + property
    guard element.responds(to: NSSelectorFromString(key)) else { return nil }
    return element.value(forKey: key)
}

/// Every element AppKit exposes under `root`, panes included.
@MainActor
private func accessible(_ root: Any) -> [NSObject] {
    guard let element = root as? NSObject else { return [] }
    return [element] + ((ax(element, "Children") as? [Any]) ?? []).flatMap { accessible($0) }
}

/// Set LOOPFLOW_FLOW_CAPTURE_DIR to write what a window drew as `name`.png.
@MainActor
private func captureIfRequested(_ window: NSWindow, name: String) throws {
    guard let directory = ProcessInfo.processInfo.environment["LOOPFLOW_FLOW_CAPTURE_DIR"],
          let content = window.contentView,
          let bitmap = content.bitmapImageRepForCachingDisplay(in: content.bounds) else { return }
    content.cacheDisplay(in: content.bounds, to: bitmap)
    let png = try #require(bitmap.representation(using: .png, properties: [:]))
    try png.write(to: URL(fileURLWithPath: directory).appendingPathComponent("\(name).png"))
}

@Suite("Task Flow")
struct TaskFlowTests {

    @Test("Task delivery shows pending work and confirmed follow-up links beside the conversation")
    @MainActor
    func deliveryEvidence() throws {
        let rows = try JSONDecoder().decode([String: RoadmapTask].self, from: fixture("task_delivery_rows.json"))
        let pending = TaskDeliveryView(task: try #require(rows["pending"]))
        #expect(try pending.inspect().find(text: "Merged · Follow-through pending").string()
                == "Merged · Follow-through pending")
        let conversion = TaskDeliveryView(task: try #require(rows["conversion"]))
        #expect(try conversion.inspect().find(text: "Verify the installed release. Evidence: the command succeeds on the released version.").string()
                == "Verify the installed release. Evidence: the command succeeds on the released version.")
        let done = TaskDeliveryView(task: try #require(rows["done"]))
        #expect(try done.inspect().find(text: "Done · Follow-up W2-FOLLOW").string()
                == "Done · Follow-up W2-FOLLOW")
        #expect(try done.inspect().find(ViewType.Link.self).url().absoluteString
                == "https://linear.app/loopflow/issue/W2-FOLLOW")
        let none = TaskDeliveryView(task: try #require(rows["none"]))
        #expect(try none.inspect().find(text: "No accepted remaining obligations.").string()
                == "No accepted remaining obligations.")
        let due = TaskDeliveryView(task: try #require(rows["due"]))
        #expect(try due.inspect().find(text: "Follow-up to W2-SOURCE · Due 2026-10-08").string()
                == "Follow-up to W2-SOURCE · Due 2026-10-08")
    }
    @Test("Separate catalogs retain invalid entries and graph composition")
    func flowFixtures() throws {
        let catalog = try JSONDecoder().decode([FlowCatalogEntry].self, from: fixture("flow_catalog.json"))
        let workflows = try JSONDecoder().decode([WorkflowCatalogEntry].self, from: fixture("workflow_catalog.json"))
        #expect(catalog.map(\.id) == ["feature", "broken"])
        #expect(workflows[0].workflow?.edges.map(\.launchName) == ["pursue", "pursue", "ship"])
        #expect(workflows[1].source == ".lf/workflows/proof.yaml" && workflows[1].unavailable != nil)
        #expect(catalog[0].graph?.steps.count == 8 && catalog[1].unavailable != nil)
        let graph = try #require(catalog[0].graph)
        let composition = try #require(catalog[0].composition)
        #expect(FlowCompositionProjection(graph: graph, items: composition.items, expanded: ["group-0"]).graph == graph)
    }

    @Test("Template disclosure keeps repeated and empty groups, XOR paths and both returns")
    @MainActor
    func templateDisclosure() throws {
        let entry = try JSONDecoder().decode(FlowCatalogEntry.self, from: fixture("flow_composition.json"))
        let template = try #require(entry.composition)
        let graph = try #require(entry.graph)
        let folded = FlowCompositionProjection(graph: graph, items: template.items, expanded: [])
        #expect(folded.graph.steps.map(\.key) == [8, 9, 10, 2, 12])
        #expect(folded.graph.steps[0].label == folded.graph.steps[1].label)
        #expect(folded.graph.steps[2].label.contains("0"))
        let returns = FlowCompositionView.spans(graph, projection: folded)
        #expect(returns.map(\.decider) == [5, 7])
        #expect(returns.allSatisfy { $0.from == 4 && $0.to == 4 })
        let partial = FlowCompositionProjection(graph: graph, items: template.items, expanded: ["group-0", "group-4"])
        #expect(partial.graph.steps.map(\.key) == [0, 8, 9, 2, 4, 5, 6, 7])
        let all = FlowCompositionProjection(graph: graph, items: template.items, expanded: Set((0...4).map { "group-\($0)" }))
        #expect(all.graph == graph)
        #expect(all.graph.node(3)?.label == "implement")

        var missing = try #require(JSONSerialization.jsonObject(with: fixture("flow_composition.json")) as? [String: Any])
        var body = try #require(missing["composition"] as? [String: Any])
        body.removeValue(forKey: "items")
        missing["composition"] = body
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(FlowCatalogEntry.self, from: JSONSerialization.data(withJSONObject: missing))
        }
    }

    @Test("Occurrence state keeps pass completions while iteration keeps each edge count")
    func occurrenceStates() throws {
        let snapshots = try JSONDecoder().decode([FlowProcessDetail].self, from: fixture("flow_process_progress.json"))
        let review = snapshots[1]
        let states = flowNodeStates(review.graph, latest: review)
        #expect(states[1] == .completed && states[3] == .completed)
        #expect(states[4] == .stopped)
        // The second decision is pending again in this pass; the router pends too.
        #expect(states[5] == .pending && states[6] == .pending)
        #expect(states[7] == .pending)
        #expect(review.iterations == [[1, 1]])
        #expect(review.returns[1].traversals == 1)
        let running = snapshots[0]
        #expect(running.iterations == [[2, 0]])
        #expect(flowIterationLabel([[2, 1], [3]]) == "loop 1 pass 3, loop 2 pass 2, loop 3 pass 4")
        #expect(flowIterationLabel([[], [2]]) == "pass 3")
        #expect(flowIterationLabel([[0, 0]]) == nil)
        #expect(running.returns.map(\.traversals) == [2, 0])

        let blocked = snapshots[2]
        #expect(flowNodeStates(blocked.graph, latest: blocked)[3] == .blocked)
        let evidence = try JSONDecoder().decode(TaskExecutionSnapshot.self, from: fixture("task_execution_stalled.json"))
        #expect(evidence.state == .stalled)
        // A preview only marks human boundaries.
        let preview = flowNodeStates(review.graph, latest: nil)
        #expect(preview[4] == .pendingHuman && preview[1] == .pending)
    }

    @Test("A stopped Flow preserves the last Process outcome", arguments: ["succeeded", "failed", "interrupted"])
    func stoppedProcessOutcome(_ outcome: String) throws {
        var wire = try #require(JSONSerialization.jsonObject(with: fixture("flow_detail.json")) as? [String: Any])
        var entry = try #require(wire["entry"] as? [String: Any])
        entry["state"] = "stopped"
        wire["entry"] = entry
        var steps = try #require(wire["steps"] as? [[String: Any]])
        steps[steps.count - 1]["outcome"] = outcome
        wire["steps"] = steps
        let flow = try JSONDecoder().decode(FlowProcessDetail.self, from: JSONSerialization.data(withJSONObject: wire))
        #expect(flowNodeStates(flow.graph, latest: flow)[try #require(flow.current)] ==
                (outcome == "succeeded" ? .stopped : .blocked))
    }

    @Test("Captured numeric IDs preserve nested containment and independent return counts")
    func nestedNumericIdentity() throws {
        let latest = try JSONDecoder().decode(FlowProcessDetail.self, from: fixture("flow_numeric_nested.json"))
        let graph = latest.graph
        #expect(graph.steps.map(\.key) == [0, 1, 9])
        #expect(graph.node(5)?.label == "check")
        #expect(graph.node(5)?.returnsTo == 4)
        #expect(graph.node(6)?.returnsTo == 2)
        #expect(graph.node(8)?.returnsTo == 7)
        #expect(graph.node(9)?.returnsTo == 0)
        #expect(graph.node(10) == nil)
        let outer = try #require(graph.node(1))
        #expect(outer.contains(5) && outer.contains(8) && !outer.contains(9))
        #expect(outer.paths[0].steps.contains { $0.contains(5) })
        #expect(!outer.paths[1].steps.contains { $0.contains(5) })
        let states = flowNodeStates(graph, latest: latest)
        #expect(states[1] == .running && states[3] == .running && states[5] == .running)
        #expect(states[4] == .completed && states[7] == .pending && states[9] == .pending)
        #expect(latest.returns.map(\.decider) == [5, 6, 8, 9])
        #expect(latest.returns.map(\.traversals) == [2, 0, 0, 3])
        #expect(latest.iterations == [[3], [0], [2]])
    }

    @Test("A Flow process's length and the two return ports")
    @MainActor
    func durationAndPorts() {
        #expect(FlowProcessView.duration(0) == "0s")
        #expect(FlowProcessView.duration(42) == "42s")
        #expect(FlowProcessView.duration(725) == "12m 5s")
        #expect(FlowProcessView.duration(11_100) == "3h 5m")
        #expect(FlowProcessView.duration(-3) == "0s")
        // Loop 1 lands from above, Loop 2 from below, both at the target's left edge.
        #expect(FlowLoopGeometry.returnsAbove(0) && !FlowLoopGeometry.returnsAbove(1))
        #expect(FlowLoopGeometry.landing(0) == FlowLoopGeometry.landing(1))
        #expect(FlowLoopGeometry.above(1) > FlowLoopGeometry.above(0))
        #expect(FlowLoopGeometry.below(1) > FlowLoopGeometry.below(0))
    }

    @Test("The Task workspace opens on the ensured Session, with the Workflow in its header and the Flow process log as a pane")
    @MainActor
    func taskWorkspace() async throws {
        _ = NSApplication.shared
        NSApp.accessibilitySetValue(true, forAttribute: NSAccessibility.Attribute(rawValue: "AXEnhancedUserInterface"))
        func object(_ name: String) throws -> [String: Any] {
            try #require(JSONSerialization.jsonObject(with: fixture(name)) as? [String: Any])
        }
        let repo = "/src/loopflow"
        let roadmap = try object("roadmap_snapshot.json")
        let waves = try #require(roadmap["waves"] as? [[String: Any]]).map { try #require($0["wave"]) }
        // The Task waits at `demo` after three Flow processes: one finished, one stopped, one running.
        var work = try object("task_work.json")
        var workflow = try #require(work["workflow"] as? [String: Any])
        workflow["position"] = ["kind": "node", "node": "demo"]
        workflow["outgoing"] = [3, 4]
        work["workflow"] = workflow
        let run = try object("flow_detail.json")
        let running = try #require((work["flow_processes"] as? [[String: Any]])?.first)
        let runId = try #require(running["id"] as? String)
        let started = try #require((run["steps"] as? [[String: Any]])?.first?["started_at"] as? Int)
        func process(_ id: String, _ name: String, _ state: String, seconds: Int) -> [String: Any] {
            running.merging(["id": id, "name": name, "state": state, "updated_at": started - 7_200,
                             "ended_at": started - 7_200 + seconds]) { $1 }
        }
        work["flow_processes"] = [
            process("22222222-2222-4222-8222-222222222222", "task-design", "completed", seconds: 725),
            process("44444444-4444-4444-8444-444444444444", "pursue", "stopped", seconds: 42),
            running,
        ]

        let reader = ScriptedReader(repo: repo, planning: ["roadmap": roadmap, "waves": waves],
                                    work: work, runs: [run])
        let calls = CallLog()
        let query = RegistryQuery(watchWork: { await reader.open() }) { args, _ in
            await calls.add(args)
            switch (args.first, args.dropFirst().first) {
            case ("session", "ensure"):
                return try await reader.ensure(issue: args[3])
            case ("machine", "id"): return #"{"id":"\#(fixtureMachineId)"}"#
            default: throw RegistryQueryError("The fixture has no \(args.prefix(2).joined(separator: " "))")
            }
        }
        let model = WorkModel(query: query, repoPath: repo)
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let view = SessionsView(model: model, repoPath: repo, workspaces: registry, query: query)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1280, height: 720),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: view)
        defer { window.contentView = nil }
        func settle() async throws {
            window.contentView?.layoutSubtreeIfNeeded()
            try await Task.sleep(for: .milliseconds(50))
        }
        func eventually(_ what: String, _ condition: @MainActor () async -> Bool) async throws {
            for _ in 0..<200 where !(await condition()) {
                model.syncWorkScope()
                try await settle()
            }
            #expect(await condition(), "\(what)")
        }
        // ViewInspector reads the toolbar and header; panes take the model from
        // the environment, so what they draw is read from AppKit's tree.
        func find(_ id: String) throws -> InspectableView<ViewType.ClassifiedView> {
            try view.inspect().find(viewWithAccessibilityIdentifier: id)
        }
        func drawn() -> Set<String> {
            Set(accessible(window.contentView!).compactMap { ax($0, "Identifier") as? String })
        }

        // Opening a Task asks `lf` for its one Session and opens what it answers.
        try await eventually("planning arrives") { model.task(id: "issue-now") != nil }
        let task = try #require(model.task(id: "issue-now")?.task)
        let identity = try #require(task.reference.workspace?.identity)
        let panes = registry.workspace(for: identity).multiplexer
        model.select(.task(id: task.id))
        model.navigation.content = .terminals
        try await eventually("the Task's Session is ensured and opened") {
            panes.focusedPane.content == .session(id: "ensured-\(task.task.identifier)")
        }
        #expect(await calls.calls.filter { $0.first == "session" && $0[1] == "ensure" }
            == [["session", "ensure", "--task", task.task.identifier, "--json"]])

        // The header is the Workflow: the current node, its edges as buttons, Move to.
        try await eventually("the Task part arrives") { model.taskWork[task.id].value != nil }
        try await settle()
        #expect(try find("task-workflow-node-demo").accessibilityValue().string() == "Current")
        #expect(try find("task-workflow-process-4").button().labelView().text().string() == "ship")
        _ = try find("task-workflow-move")
        let header = drawn()
        #expect(header.contains("task-workflow-node-demo"), "the accessibility tree is readable")
        #expect(header.isDisjoint(with: [
            "work-materials", "work-toggle-materials", "work-toggle-files",
            "work-current-stage", "task-show-monitor-\(task.id)", "task-workflow-position",
            "task-flow-processes", "task-work", "task-flow-start",
        ]))

        // One menu adds the Flow process log as a pane beside the Session.
        try find("work-add-flow-log").button().tap()
        try await settle()
        #expect(panes.focusedPane.content == .flowLog(taskId: task.id))
        #expect(panes.layout.allPanes.count == 2)
        // A line per process; its id, graph and steps only when opened.
        let log = FlowProcessLog(model: model, taskId: task.id)
        let stopped = "44444444-4444-4444-8444-444444444444"
        #expect(try log.inspect().find(viewWithAccessibilityIdentifier: "task-work-\(stopped)")
            .find(text: "42s").string() == "42s")
        #expect(throws: (any Error).self) { try log.inspect().find(text: stopped) }
        model.navigation.expandedFlowProcesses.insert(runId)
        try await settle()
        #expect(try log.inspect().find(viewWithAccessibilityIdentifier: "flow-process-id-\(runId)").text().string() == runId)
        #expect(drawn().isSuperset(of: ["task-flow-processes", "flow-process-status-\(runId)"]))

        // The same menu adds the Task's files as a pane.
        try find("work-add-files").button().tap()
        try await settle()
        #expect(panes.focusedPane.content == .files(taskId: task.id))
        panes.close(panes.focusedPaneId)
        try await settle()

        try captureIfRequested(window, name: "task-workspace")
    }
}

private actor CallLog {
    private(set) var calls: [[String]] = []
    func add(_ args: [String]) { calls.append(args) }
}

/// A scripted workspace reader: every request is answered with the planning,
/// Sessions and Task parts as they stand, the way `lf monitor work` does.
@MainActor
private final class ScriptedReader {
    private let repo: String
    private let planning: [String: Any]
    private let work: [String: Any]
    private let runs: [[String: Any]]
    private var sessions: [[String: Any]] = []
    private var continuation: AsyncThrowingStream<WorkFrame, any Error>.Continuation?
    private var sequence = 0
    private var task: String?

    init(repo: String, planning: [String: Any], work: [String: Any], runs: [[String: Any]]) {
        (self.repo, self.planning, self.work, self.runs) = (repo, planning, work, runs)
    }

    func open() -> WorkObservation {
        let (stream, continuation) = AsyncThrowingStream<WorkFrame, any Error>.makeStream()
        self.continuation = continuation
        return WorkObservation(frames: stream, request: { request in
            Task { @MainActor in self.answer(request) }
        }, cancel: { continuation.finish() })
    }

    /// `lf session ensure --task`: the Task gets one conversation, its primary.
    func ensure(issue: String) throws -> String {
        let roadmap = try JSONDecoder().decode(
            RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: try #require(planning["roadmap"])))
        let task = try #require(roadmap.waves.flatMap(\.tasks.items).first { $0.task.identifier == issue })
        let workspace = try #require(task.reference.workspace)
        let work = WorkReference.task(id: try #require(task.runtime?.workId))
        var session = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(
            renameFixtureRecord("ensured-\(issue)", title: "task-conversation", work: work))) as? [String: Any])
        session["task_primary"] = true
        session["workspace"] = ["machine_id": workspace.machineId, "worktree": workspace.worktree,
                                "task_id": task.id, "unavailable": NSNull()]
        sessions = [session]
        return String(decoding: try JSONSerialization.data(withJSONObject: session), as: UTF8.self)
    }

    private func answer(_ request: WorkRequest) {
        let id: Int
        switch request {
        case .scope(let request, let scope): (id, task) = (request, scope.task)
        case .refresh(let request): id = request
        }
        send("planning", answers: id, body: planning)
        send("sessions", answers: id, body: ["repo": repo, "includes_headless": false, "entries": sessions])
        if let task { send("task", answers: id, body: ["task": task, "work": work, "flow_processes": runs]) }
    }

    private func send(_ part: String, answers: Int, body: [String: Any]) {
        sequence += 1
        let line: [String: Any] = ["part": part, "sequence": sequence, "answers": answers, "home": "/home",
                                   "revisions": NSNull(), "unavailable": NSNull(), "body": body]
        guard let data = try? JSONSerialization.data(withJSONObject: line),
              let frame = try? WorkFrame.decode(line: data) else { return }
        continuation?.yield(frame)
    }
}

#if canImport(GhosttyKit)
@Suite("Task Flow native proof", .requiresDisplay, .serialized)
@MainActor
struct TaskFlowProofTests {
    @Test("Template disclosure and the Task workspace keep the Session's terminal, draft and companion")
    func flowControlsRetainTerminals() async throws {
        _ = NSApplication.shared
        NSApp.accessibilitySetValue(true, forAttribute: NSAccessibility.Attribute(rawValue: "AXEnhancedUserInterface"))
        GhosttyManager.shared.initialize()
        let repo = "/src/loopflow"
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let workspace = registry.workspace(for: fixtureWorkspace(repo))
        var terminals: [GhosttyMetalView] = []
        for _ in 0..<2 {
            workspace.multiplexer.newShell()
            let pane = workspace.multiplexer.focusedPaneId
            let terminal = registry.surfaces.view(for: .shell(pane))
            terminal.frame = CGRect(x: 0, y: 0, width: 400, height: 350)
            terminal.workingDirectory = NSTemporaryDirectory()
            terminal.command = buildWorkspaceShellCommand(id: pane, argv: ["/bin/cat"], env: [:])
            terminal.createSurface(manager: GhosttyManager.shared)
            terminals.append(terminal)
        }
        defer { for terminal in terminals { registry.surfaces.release(terminal.terminal) } }
        let surfaces = try terminals.map { try #require($0.surface) }
        let shells = workspace.multiplexer.layout.allPanes.map(\.id)
        let layout = workspace.multiplexer.layout

        let reviewed = WorkReference.task(id: "ts_review00000000000000000000000000")
        var session = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(
            renameFixtureRecord("design", title: "review-design", work: reviewed)
        )) as? [String: Any])
        session["state"] = "active"
        session["actions"] = sessionActionFixture(state: "active")
        session["terminal_ids"] = [shells[0]]
        session["open_argv"] = ["must-not-launch"]
        let source = try FlowSource(session: JSONSerialization.data(withJSONObject: [session]))
        let query = RegistryQuery { args, _ in try await source.respond(args) }
        let model = WorkModel(query: query, repoPath: repo)
        await model.refresh()
        model.navigation.content = .terminals
        let view = SessionsView(model: model, repoPath: repo, workspaces: registry, query: query)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1500, height: 820),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: view)
        window.orderFront(nil)
        defer { window.orderOut(nil); window.contentView = nil }
        try await settle(window)
        let draft = "flow-proof-draft"
        draft.withCString { ghostty_surface_text(surfaces[0], $0, UInt(draft.utf8.count)) }

        func find(_ id: String) throws -> InspectableView<ViewType.ClassifiedView> {
            do {
                return try view.inspect().find(viewWithAccessibilityIdentifier: id)
            } catch {
                throw RegistryQueryError("Missing \(id): \(error)")
            }
        }
        func text(_ id: String) throws -> String { try find(id).text().string() }

        // Flow inspection keeps native Task panes retained; composition starts folded.
        await model.loadFlowCatalog()
        model.navigation.palette = .flow("feature")
        for _ in 0..<20 where model.flowCatalog.value == nil { try await settle(window) }
        try await settle(window)
        _ = try find("flow-composition-feature")
        #expect((try? find("flow-node-4")) == nil, "composition starts folded")
        try find("composition-group-group-0").disclosureGroup().expand()
        try await settle(window)
        #expect(try find("flow-node-4").accessibilityLabel().string() == "demo, human review, pending")

        let oldRevision = try #require(model.flowCatalog.value?.first?.composition?.revision)
        try await source.reviseTemplate()
        await model.loadFlowCatalog(force: true)
        try await settle(window)
        #expect(model.flowCatalog.value?.first?.composition?.revision != oldRevision)
        #expect((try? find("flow-node-4")) == nil, "a new source revision starts folded")
        #expect(await source.controls.isEmpty)

        // A return outside a folded composition still names its semantic target.
        try await source.crossingReturns()
        await model.loadFlowCatalog(force: true)
        try await settle(window)
        let inputBeforeInspection = surfaces.map(terminalText)
        #expect(inputBeforeInspection[0].contains(draft))
        #expect(terminals.allSatisfy { !$0.acceptsFirstResponder })
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(window.contentView)
        for _ in 0..<30 where focusedLabel(in: window) != "build · 1 steps" {
            try press("\t", keyCode: 48, in: window)
            try await settle(window)
        }
        try #require(focusedLabel(in: window) == "build · 1 steps")
        for prefix in ["", "3/fix/"] {
            if !prefix.isEmpty {
                try press("\t", keyCode: 48, in: window)
                try await settle(window)
                #expect(focusedLabel(in: window) == "xor-route · fix")
                try press("\u{f703}", keyCode: 124, in: window)
                try await settle(window)
                try press("\t", keyCode: 48, in: window)
                try await settle(window)
            }
            #expect(focusedLabel(in: window) == "build · 1 steps")
            func expanded(_ suffix: String) -> Bool {
                model.navigation.expandedCompositionGroups["crossing-returns"]?.contains(prefix + suffix) == true
            }
            try press("\u{f703}", keyCode: 124, in: window)
            try await settle(window)
            #expect(expanded("outer"))
            try press("\t", keyCode: 48, in: window)
            try await settle(window)
            #expect(focusedLabel(in: window) == "edit · 1 steps")
            try press(" ", keyCode: 49, in: window)
            try await settle(window)
            #expect(expanded("inner"))
            try press("\t", keyCode: 48, in: window)
            try await settle(window)
            #expect(focusedLabel(in: window) == "empty · 0 steps")
            try press("\r", keyCode: 36, in: window)
            try await settle(window)
            #expect(expanded("empty"))
            #expect(try find("composition-group-\(prefix)empty").find(text: "No steps").string() == "No steps")
            try press("\u{f702}", keyCode: 123, in: window)
            try await settle(window)
            #expect(!expanded("empty"))
            try press("\u{19}", keyCode: 48, modifiers: [.shift], in: window)
            try await settle(window)
            #expect(focusedLabel(in: window) == "edit · 1 steps")
            try press("\u{f702}", keyCode: 123, in: window)
            try await settle(window)
            #expect(!expanded("inner"))
            try press("\u{19}", keyCode: 48, modifiers: [.shift], in: window)
            try await settle(window)
            #expect(focusedLabel(in: window) == "build · 1 steps")
            try press("\u{f702}", keyCode: 123, in: window)
            try await settle(window)
            #expect(!expanded("outer"))
        }
        for prefix in ["", "3/fix/"] {
            for expanded in [false, true, false] {
                let group = try find("composition-group-\(prefix)outer").disclosureGroup()
                if expanded {
                    try group.expand()
                    try await settle(window)
                    try find("composition-group-\(prefix)inner").disclosureGroup().expand()
                } else { try group.collapse() }
                try await settle(window)
                for (number, offset) in [1, 2].enumerated() {
                    let key = (prefix.isEmpty ? 0 : 4) + offset
                    #expect(try text("task-flow-loop-\(key)") == "Loop \(number + 1)")
                    try pressElement("flow-node-\(key)", in: window)
                    try await settle(window)
                    let expected = prefix.isEmpty ? "implement" : "fix-implement"
                    let details = accessible(window.contentView!).compactMap { ax($0, "Value") as? String }
                        .filter { $0.hasPrefix("Iterate returns to") }
                    #expect(details.contains("Iterate returns to \(expected)"))
                    #expect(details.allSatisfy { $0 == "Iterate returns to implement" || $0 == "Iterate returns to fix-implement" })
                }
            }
        }
        #expect(await source.controls.isEmpty, "template inspection never starts work")
        #expect(surfaces.map(terminalText) == inputBeforeInspection, "disclosure keys never reach a PTY")

        // The Session's Task: its Workflow is the header, with no Flow card or Sessions list.
        model.select(.task(id: "issue-review"))
        try await settle(window)
        _ = try find("task-workflow")
        #expect((try? find("task-flow-start")) == nil)
        #expect((try? find("work-materials")) == nil)
        try captureIfRequested(window, name: "task-workspace-native")

        // The Session and its companion survived every Flow interaction.
        let record = try #require(model.sessions.value?.first { $0.id == "design" })
        model.navigation.content = .terminals
        model.navigation.selectedSessionId = record.id
        workspace.multiplexer.setFocusedPane(shells[0])
        try await settle(window)
        #expect(workspace.multiplexer.layout == layout)
        for (index, surface) in surfaces.enumerated() {
            #expect(terminals[index].surface == surface)
            let reply = index == 0 ? draft : "companion-responds"
            let input = index == 0 ? "\n" : reply + "\n"
            input.withCString { ghostty_surface_text(surface, $0, UInt(input.utf8.count)) }
            let deadline = ContinuousClock.now + .seconds(3)
            while terminalText(surface).components(separatedBy: reply).count < 3, ContinuousClock.now < deadline {
                try await Task.sleep(for: .milliseconds(20))
            }
            #expect(terminalText(surface).components(separatedBy: reply).count == 3)
        }
    }

    private func press(_ character: String, keyCode: UInt16, modifiers: NSEvent.ModifierFlags = [], in window: NSWindow) throws {
        for type in [NSEvent.EventType.keyDown, .keyUp] {
            let event = try #require(NSEvent.keyEvent(with: type, location: .zero, modifierFlags: modifiers,
                timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil,
                characters: character, charactersIgnoringModifiers: character, isARepeat: false, keyCode: keyCode))
            NSApp.sendEvent(event)
        }
    }

    private func focusedLabel(in window: NSWindow) -> String? {
        accessible(window.contentView!).first { ax($0, "Focused") as? Bool == true }
            .flatMap { ax($0, "Label") as? String }
    }

    private func pressElement(_ id: String, in window: NSWindow) throws {
        let element = try #require(accessible(window.contentView!).first { ax($0, "Identifier") as? String == id })
        let action = NSSelectorFromString("accessibilityPerformPress")
        try #require(element.responds(to: action))
        element.perform(action)
    }

    private func settle(_ window: NSWindow) async throws {
        window.contentView?.layoutSubtreeIfNeeded()
        window.layoutIfNeeded()
        // Drive AppKit's dynamic key loop in this unhosted window, alongside layout.
        window.recalculateKeyViewLoop()
        if let content = window.contentView,
           let bitmap = content.bitmapImageRepForCachingDisplay(in: content.bounds) {
            content.cacheDisplay(in: content.bounds, to: bitmap)
        }
        try await Task.sleep(for: .milliseconds(100))
    }

    private func terminalText(_ surface: ghostty_surface_t) -> String {
        var text = ghostty_text_s()
        let selection = ghostty_selection_s(
            top_left: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_TOP_LEFT, x: 0, y: 0),
            bottom_right: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_BOTTOM_RIGHT, x: 0, y: 0),
            rectangle: false
        )
        guard ghostty_surface_read_text(surface, selection, &text) else { return "" }
        defer { ghostty_surface_free_text(surface, &text) }
        guard let bytes = text.text else { return "" }
        return String(decoding: Data(bytes: bytes, count: Int(text.text_len)), as: UTF8.self)
    }
}

/// Shared reads plus recorded controls. Only `task run` counts as a control;
/// anything else unexpected fails loudly.
private actor FlowSource {
    private var roadmap: [String: Any]
    private var catalog: String
    private let session: String
    private(set) var controls: [[String]] = []

    init(session: Data) throws {
        guard let roadmap = try JSONSerialization.jsonObject(with: fixture("roadmap_snapshot.json")) as? [String: Any],
              var entries = try JSONSerialization.jsonObject(with: fixture("flow_catalog.json")) as? [[String: Any]],
              var graph = entries[0]["graph"] as? [String: Any]
        else { throw RegistryQueryError("Flow proof fixtures are malformed") }
        self.roadmap = roadmap
        var build = entries[0]
        graph["name"] = "build"
        build["name"] = "build"
        build["graph"] = graph
        entries.insert(build, at: 1)
        catalog = String(decoding: try JSONSerialization.data(withJSONObject: entries), as: UTF8.self)
        self.session = String(decoding: session, as: UTF8.self)
    }

    func crossingReturns() throws {
        var entries = try #require(JSONSerialization.jsonObject(with: Data(catalog.utf8)) as? [[String: Any]])
        func steps(_ prefix: String) -> [[String: Any]] {
            (0...2).map { index -> [String: Any] in
                let target = prefix.isEmpty ? "implement" : "fix-implement"
                return ["key": (prefix.isEmpty ? 0 : 4) + index, "id": NSNull(),
                 "label": index == 0 ? target : "loop-or-next", "kind": "skill",
                 "human": false, "returns_to": index == 0 ? NSNull() : (prefix.isEmpty ? 0 : 4) as Any,
                 "sources": ["feature"], "paths": []]
            }
        }
        func items(_ prefix: String) -> [[String: Any]] {
            [["kind": "group", "id": "\(prefix)outer", "name": "build", "items": [
                ["kind": "group", "id": "\(prefix)inner", "name": "edit", "items": [
                    ["kind": "node", "key": (prefix.isEmpty ? 0 : 4) + 0, "paths": [:]]
                ]],
                ["kind": "group", "id": "\(prefix)empty", "name": "empty", "items": []]
            ]],
             ["kind": "node", "key": (prefix.isEmpty ? 0 : 4) + 1, "paths": [:]],
             ["kind": "node", "key": (prefix.isEmpty ? 0 : 4) + 2, "paths": [:]]]
        }
        var nodes = steps("")
        nodes.append(["key": 3, "id": NSNull(), "label": "xor-route", "kind": "xor",
                      "human": false, "returns_to": NSNull(), "sources": ["feature"],
                      "paths": [["name": "fix", "description": "Repair", "steps": steps("3/fix/")]]])
        var tree = items("")
        tree.append(["kind": "node", "key": 3, "paths": ["fix": items("3/fix/")]])
        entries[0]["graph"] = ["name": "feature", "steps": nodes]
        entries[0]["composition"] = ["revision": "crossing-returns", "items": tree]
        catalog = String(decoding: try JSONSerialization.data(withJSONObject: entries), as: UTF8.self)
    }

    func reviseTemplate() throws {
        var entries = try #require(JSONSerialization.jsonObject(with: Data(catalog.utf8)) as? [[String: Any]])
        var template = try #require(entries[0]["composition"] as? [String: Any])
        template["revision"] = "changed-feature-definition"
        entries[0]["composition"] = template
        catalog = String(decoding: try JSONSerialization.data(withJSONObject: entries), as: UTF8.self)
    }

    func respond(_ args: [String]) throws -> String {
        switch (args.first, args.dropFirst().first) {
        case ("roadmap", _):
            return String(decoding: try JSONSerialization.data(withJSONObject: roadmap), as: UTF8.self)
        case ("wave", "list"): return "[]"
        case ("activity", _): return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
        case ("session", "list"): return #"{"entries":\#(session),"next":null}"#
        case ("flow", "list"): return catalog
        case ("-b", "task") where args.dropFirst(2).first == "run":
            controls.append(args)
            // The fixture refuses W2-131's launch.
            if args[3] == "W2-131" { throw RegistryQueryError("Task flow \"build\" was refused by the fixture") }
            return ""
        default:
            throw RegistryQueryError("Unexpected Flow proof operation: \(args)")
        }
    }
}
#endif
#endif
