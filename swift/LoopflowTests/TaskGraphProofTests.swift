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

/// Set LOOPFLOW_GRAPH_CAPTURE_DIR to write what a window drew as `name`.png.
@MainActor
private func captureIfRequested(_ window: NSWindow, name: String) throws {
    guard let directory = ProcessInfo.processInfo.environment["LOOPFLOW_GRAPH_CAPTURE_DIR"],
          let content = window.contentView,
          let bitmap = content.bitmapImageRepForCachingDisplay(in: content.bounds) else { return }
    content.cacheDisplay(in: content.bounds, to: bitmap)
    let png = try #require(bitmap.representation(using: .png, properties: [:]))
    try png.write(to: URL(fileURLWithPath: directory).appendingPathComponent("\(name).png"))
}

@Suite("Task Graph")
struct TaskGraphTests {

    @Test("Task due date and regression mark appear beside the conversation")
    @MainActor
    func planningFacts() throws {
        let rows = try JSONDecoder().decode([String: RoadmapTask].self, from: fixture("task_delivery_rows.json"))
        let view = TaskPlanningFactsView(task: try #require(rows["due"]))
        #expect(try view.inspect().find(text: "Due 2026-10-08").string() == "Due 2026-10-08")
        #expect(try view.inspect().find(text: "Regressed in v0.15.3").string() == "Regressed in v0.15.3")
    }

    @Test("Separate catalogs retain invalid entries and compiled composition")
    func graphFixtures() throws {
        let catalog = try JSONDecoder().decode([GraphCatalogEntry].self, from: fixture("graph_catalog.json"))
        let workflows = catalog.map(WorkflowCatalogEntry.init)
        #expect(catalog.map(\.id) == ["feature", "broken"])
        #expect(workflows[0].workflow?.steps.isEmpty == false)
        #expect(workflows[1].source == ".lf/graphs/broken.yaml" && workflows[1].unavailable != nil && workflows[1].workflow == nil)
        #expect(catalog[0].compiled?.steps.count == 8 && catalog[1].unavailable != nil)
        let compiled = try #require(catalog[0].compiled)
        let composition = try #require(catalog[0].composition)
        #expect(GraphCompositionProjection(compiled: compiled, items: composition.items, expanded: ["group-0"]).compiled == compiled)
    }

    @Test("Template disclosure keeps repeated and empty groups, XOR paths and both returns")
    @MainActor
    func templateDisclosure() throws {
        let entry = try JSONDecoder().decode(GraphCatalogEntry.self, from: fixture("graph_composition.json"))
        let template = try #require(entry.composition)
        let compiled = try #require(entry.compiled)
        let folded = GraphCompositionProjection(compiled: compiled, items: template.items, expanded: [])
        #expect(folded.compiled.steps.map(\.key) == [8, 9, 10, 2, 12])
        #expect(folded.compiled.steps[0].label == folded.compiled.steps[1].label)
        #expect(folded.compiled.steps[2].label.contains("0"))
        let returns = GraphCompositionView.spans(compiled, projection: folded)
        #expect(returns.map(\.decider) == [5, 7])
        #expect(returns.allSatisfy { $0.from == 4 && $0.to == 4 })
        let partial = GraphCompositionProjection(compiled: compiled, items: template.items, expanded: ["group-0", "group-4"])
        #expect(partial.compiled.steps.map(\.key) == [0, 8, 9, 2, 4, 5, 6, 7])
        let all = GraphCompositionProjection(compiled: compiled, items: template.items, expanded: Set((0...4).map { "group-\($0)" }))
        #expect(all.compiled == compiled)
        #expect(all.compiled.node(3)?.label == "implement")

        var missing = try #require(JSONSerialization.jsonObject(with: fixture("graph_composition.json")) as? [String: Any])
        var body = try #require(missing["composition"] as? [String: Any])
        body.removeValue(forKey: "items")
        missing["composition"] = body
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(GraphCatalogEntry.self, from: JSONSerialization.data(withJSONObject: missing))
        }
    }

    @Test("Occurrence state keeps pass completions while iteration keeps each edge count")
    func occurrenceStates() throws {
        let snapshots = try JSONDecoder().decode([GraphProcessDetail].self, from: fixture("graph_process_progress.json"))
        let review = snapshots[1]
        let states = graphNodeStates(review.compiled, latest: review)
        #expect(states[1] == .completed && states[3] == .completed)
        #expect(states[4] == .stopped)
        // The second decision is pending again in this pass; the router pends too.
        #expect(states[5] == .pending && states[6] == .pending)
        #expect(states[7] == .pending)
        #expect(review.iterations == [[1, 1]])
        #expect(review.returns[1].traversals == 1)
        let running = snapshots[0]
        #expect(running.iterations == [[2, 0]])
        #expect(graphIterationLabel([[2, 1], [3]]) == "loop 1 pass 3, loop 2 pass 2, loop 3 pass 4")
        #expect(graphIterationLabel([[], [2]]) == "pass 3")
        #expect(graphIterationLabel([[0, 0]]) == nil)
        #expect(running.returns.map(\.traversals) == [2, 0])

        let blocked = snapshots[2]
        #expect(graphNodeStates(blocked.compiled, latest: blocked)[3] == .blocked)
        let evidence = try JSONDecoder().decode(TaskExecutionSnapshot.self, from: fixture("task_execution_stalled.json"))
        #expect(evidence.state == .stalled)
        // A preview only marks stages.
        let preview = graphNodeStates(review.compiled, latest: nil)
        #expect(preview[4] == .pendingHuman && preview[1] == .pending)
    }

    @Test("A stopped Graph preserves the last Process outcome", arguments: ["succeeded", "failed", "interrupted"])
    func stoppedProcessOutcome(_ outcome: String) throws {
        var wire = try #require(JSONSerialization.jsonObject(with: fixture("graph_detail.json")) as? [String: Any])
        var entry = try #require(wire["entry"] as? [String: Any])
        entry["state"] = "stopped"
        wire["entry"] = entry
        var steps = try #require(wire["steps"] as? [[String: Any]])
        steps[steps.count - 1]["outcome"] = outcome
        wire["steps"] = steps
        let graph = try JSONDecoder().decode(GraphProcessDetail.self, from: JSONSerialization.data(withJSONObject: wire))
        #expect(graphNodeStates(graph.compiled, latest: graph)[try #require(graph.current)] ==
                (outcome == "succeeded" ? .stopped : .blocked))
    }

    @Test("Captured numeric IDs preserve nested containment and independent return counts")
    func nestedNumericIdentity() throws {
        let latest = try JSONDecoder().decode(GraphProcessDetail.self, from: fixture("graph_numeric_nested.json"))
        let compiled = latest.compiled
        #expect(compiled.steps.map(\.key) == [0, 1, 9])
        #expect(compiled.node(5)?.label == "check")
        #expect(compiled.node(5)?.returnsTo == 4)
        #expect(compiled.node(6)?.returnsTo == 2)
        #expect(compiled.node(8)?.returnsTo == 7)
        #expect(compiled.node(9)?.returnsTo == 0)
        #expect(compiled.node(10) == nil)
        let outer = try #require(compiled.node(1))
        #expect(outer.contains(5) && outer.contains(8) && !outer.contains(9))
        #expect(outer.paths[0].steps.contains { $0.contains(5) })
        #expect(!outer.paths[1].steps.contains { $0.contains(5) })
        let states = graphNodeStates(compiled, latest: latest)
        #expect(states[1] == .running && states[3] == .running && states[5] == .running)
        #expect(states[4] == .completed && states[7] == .pending && states[9] == .pending)
        #expect(latest.returns.map(\.decider) == [5, 6, 8, 9])
        #expect(latest.returns.map(\.traversals) == [2, 0, 0, 3])
        #expect(latest.iterations == [[3], [0], [2]])
    }

    @Test("A Graph process's length and the two return ports")
    @MainActor
    func durationAndPorts() {
        #expect(GraphProcessView.duration(0) == "0s")
        #expect(GraphProcessView.duration(42) == "42s")
        #expect(GraphProcessView.duration(725) == "12m 5s")
        #expect(GraphProcessView.duration(11_100) == "3h 5m")
        #expect(GraphProcessView.duration(-3) == "0s")
        // Loop 1 lands from above, Loop 2 from below, both at the target's left edge.
        #expect(GraphLoopGeometry.returnsAbove(0) && !GraphLoopGeometry.returnsAbove(1))
        #expect(GraphLoopGeometry.landing(0) == GraphLoopGeometry.landing(1))
        #expect(GraphLoopGeometry.above(1) > GraphLoopGeometry.above(0))
        #expect(GraphLoopGeometry.below(1) > GraphLoopGeometry.below(0))
    }

    @Test("Task navigation opens Work only; an explicit Session link keeps the Workflow and companions", .requiresDisplay)
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
        // The Task waits at `demo` after three Graph processes: one finished, one stopped, one running.
        var work = try object("task_work.json")
        var graph = try #require(work["graph"] as? [String: Any])
        graph["position"] = ["kind": "at", "step": "demo"]
        work["graph"] = graph
        let run = try object("graph_detail.json")
        let running = try #require((work["graph_processes"] as? [[String: Any]])?.first)
        let runId = try #require(running["id"] as? String)
        let started = try #require((run["steps"] as? [[String: Any]])?.first?["started_at"] as? Int)
        func process(_ id: String, _ name: String, _ state: String, seconds: Int) -> [String: Any] {
            running.merging(["id": id, "name": name, "state": state, "updated_at": started - 7_200,
                             "ended_at": started - 7_200 + seconds]) { $1 }
        }
        work["graph_processes"] = [
            process("22222222-2222-4222-8222-222222222222", "task-design", "completed", seconds: 725),
            process("44444444-4444-4444-8444-444444444444", "pursue", "stopped", seconds: 42),
            running,
        ]

        let reader = ScriptedReader(repo: repo, planning: ["roadmap": roadmap, "waves": waves],
                                    work: work, runs: [run])
        let calls = CallLog()
        let roadmapJSON = String(decoding: try JSONSerialization.data(withJSONObject: roadmap), as: UTF8.self)
        let query = RegistryQuery(watchWork: { await reader.open() }) { args, _ in
            await calls.add(args)
            switch (args.first, args.dropFirst().first) {
            case ("session", "connect") where args.contains("--task"):
                return try await reader.ensure(issue: args[3])
            case ("session", "attach"):
                return try await reader.ensure(issue: String(args[2].dropFirst("ensured-".count)))
            case ("desktop", "sessions"): return try await reader.inventory()
            case ("desktop", "plan"): return roadmapJSON
            case ("machine", "id"): return #"{"id":"\#(fixtureMachineId)"}"#
            default: throw RegistryQueryError("The fixture has no \(args.prefix(2).joined(separator: " "))")
            }
        }
        let model = WorkModel(query: query, repoPath: repo)
        let keeping = Task { await model.keepWorkCurrent() }
        defer { keeping.cancel() }
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let view = SessionsView(model: model, repoPath: repo, workspaces: registry, repository: "fixture-repository", query: query)
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
            do { return try view.inspect().find(viewWithAccessibilityIdentifier: id) }
            catch { throw RegistryQueryError("Missing workspace element: \(id): \(error)") }
        }
        func drawn() -> Set<String> {
            Set(accessible(window.contentView!).compactMap { ax($0, "Identifier") as? String })
        }

        try await eventually("planning arrives") { model.task(id: "issue-now") != nil }
        let task = try #require(model.task(id: "issue-now")?.task)
        let identity = try #require(task.reference.workspace?.identity)
        model.bindWorkspaces(registry, repository: "fixture-repository")
        model.select(.task(id: task.id))
        let key = model.workspaceKey(for: .task(id: task.id), repository: "fixture-repository")
        let panes = registry.workspace(for: key, title: task.task.name, location: identity).multiplexer
        try await settle()
        #expect(panes.focusedPane.sessionId == nil)
        #expect(await calls.calls.allSatisfy { !$0.starts(with: ["session", "connect", "--task"]) })

        // A separate explicit link opens the known conversation, not bare navigation.
        let record = try JSONDecoder().decode(SessionRecord.self, from: Data(reader.ensure(issue: task.task.identifier).utf8))
        let link = TaskLink(issue: task.task.identifier, repo: repo, session: record.id)
        await model.openTaskLink(link)
        try await eventually("the explicitly addressed Session opens") { panes.focusedPane.sessionId == record.id }
        #expect(await calls.calls.allSatisfy { !$0.starts(with: ["session", "connect", "--task"]) })

        // Task chrome leaves the Workflow out; the companion starts on Files.
        try await eventually("the Task part arrives") { model.taskWork[task.id].value != nil }
        try await settle()
        let header = drawn()
        #expect((try? find("task-lifecycle-menu")) == nil)
        #expect(!header.contains("task-workflow"))
        #expect(!header.contains("work-task-details"))
        #expect(panes.layout.allPanes.count == 2)

        // The navigator opens the Graph process log beside the retained Session.
        model.navigation.palette = .search
        let content = try view.inspect().find(SessionsContentView.self).actualView()
        content.navigate(.graphLog(task.id))
        try await settle()
        #expect(panes.focusedPane.content == .browser(taskId: task.id, tabs: [.graphLog], selected: "graph-log", location: task.reference.workspace?.identity))
        #expect(panes.layout.allPanes.count == 3)
        // A line per process; its id, graph and steps only when opened.
        model.confirmMachine(try #require(task.reference.workspace?.machineId))
        let log = GraphProcessLog(model: model, taskId: task.id, location: task.reference.workspace?.identity)
        let stopped = "44444444-4444-4444-8444-444444444444"
        #expect(try log.inspect().find(viewWithAccessibilityIdentifier: "task-work-\(stopped)")
            .find(text: "42s").string() == "42s")
        #expect(throws: (any Error).self) { try log.inspect().find(text: stopped) }
        model.navigation.expandedGraphProcesses.insert(runId)
        try await settle()
        #expect(try log.inspect().find(viewWithAccessibilityIdentifier: "compiled-process-id-\(runId)").text().string() == runId)
        #expect(drawn().isSuperset(of: ["task-compiled-processes", "compiled-process-status-\(runId)"]))

        // Files remains the same tool when hidden and shown through the View command.
        let workspace = try #require(registry.retained(key))
        let filesPane = try #require(workspace.multiplexer.browserPane)
        workspace.toggleBrowser()
        workspace.toggleBrowser()
        try await settle()
        #expect(panes.focusedPane == filesPane)
        panes.close(panes.focusedPaneId)
        try await settle()

        try captureIfRequested(window, name: "task-workspace")
    }
}

private actor CallLog {
    private(set) var calls: [[String]] = []
    func add(_ args: [String]) { calls.append(args) }
}

/// Scripted watches: every request is answered with the plan, Sessions and
/// Task status as they stand.
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

    /// `lf session connect --task --prepare`: the Task gets one conversation, its primary.
    func ensure(issue: String) throws -> String {
        let roadmap = try JSONDecoder().decode(
            RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: try #require(planning["roadmap"])))
        let task = try #require(roadmap.waves.flatMap(\.tasks.items).first { $0.task.identifier == issue })
        let workspace = try #require(task.reference.workspace)
        let work = WorkReference.task(id: try #require(task.runtime?.workId))
        var session = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(
            renameFixtureRecord("ensured-\(issue)", title: "task-conversation", work: work))) as? [String: Any])
        session["task_primary"] = true
        session["workspace"] = ["machine_id": try #require(workspace.machineId), "worktree": workspace.worktree,
                                "task_id": task.id, "unavailable": NSNull()]
        sessions = [session]
        return String(decoding: try JSONSerialization.data(withJSONObject: session), as: UTF8.self)
    }

    func inventory() throws -> String {
        String(decoding: try JSONSerialization.data(withJSONObject: ["entries": sessions, "next": NSNull()]), as: UTF8.self)
    }

    private func answer(_ request: WorkRequest) {
        let id: Int
        switch request {
        case .scope(let request, let scope): (id, task) = (request, scope.task)
        case .refresh(let request): id = request
        case .checkpoint(let request, _): id = request
        }
        send(.planning, answers: id, body: planning["roadmap"] as Any)
        send(.sessions, answers: id, body: sessions)
        if task != nil {
            send(.task, answers: id, body: ["overview": work, "graph_processes": runs, "execution": NSNull(),
                                            "comments": ["identifier": task as Any, "comments": [], "pending_sync": [],
                                                         "conflicts": [String: String](), "refresh_error": NSNull()]])
        }
        if case .checkpoint = request {
            sequence += 1
            continuation?.yield(WorkFrame(sequence: sequence, answers: id, unavailable: nil, content: .checkpoint, wire: nil))
        }
    }

    private func send(_ part: WorkPart, answers: Int, body: Any) {
        sequence += 1
        let line: [String: Any] = ["answers": answers, "unavailable": NSNull(), "body": body]
        let scope = WorkScope(repo: repo, headless: false, task: task, activity: nil)
        guard let data = try? JSONSerialization.data(withJSONObject: line),
              let frame = try? WorkFrame.decode(part: part, scope: scope, sequence: sequence, line: data) else { return }
        continuation?.yield(frame)
    }
}

#if canImport(GhosttyKit)
@Suite("Task Graph native proof", .requiresDisplay, .serialized)
@MainActor
struct TaskGraphProofTests {
    @Test("Template disclosure and the Task workspace keep the Session's terminal, draft and companion")
    func graphControlsRetainTerminals() async throws {
        _ = NSApplication.shared
        NSApp.accessibilitySetValue(true, forAttribute: NSAccessibility.Attribute(rawValue: "AXEnhancedUserInterface"))
        GhosttyManager.shared.initialize()
        let repo = "/src/loopflow"
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let workspace = fixtureWork(registry, fixtureWorkspace(repo))
        var terminals: [GhosttyMetalView] = []
        for _ in 0..<2 {
            workspace.multiplexer.newShell(location: workspace.location)
            let pane = workspace.multiplexer.focusedPane.content.selectedShell!.id
            let terminal = registry.surfaces.view(for: .shell(pane, machineId: fixtureMachineId))
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
        let source = try GraphSource(session: JSONSerialization.data(withJSONObject: [session]))
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
        let draft = "graph-proof-draft"
        draft.withCString { ghostty_surface_text(surfaces[0], $0, UInt(draft.utf8.count)) }

        func find(_ id: String) throws -> InspectableView<ViewType.ClassifiedView> {
            do {
                return try view.inspect().find(viewWithAccessibilityIdentifier: id)
            } catch {
                throw RegistryQueryError("Missing \(id): \(error)")
            }
        }
        func text(_ id: String) throws -> String { try find(id).text().string() }

        // Graph inspection keeps native Task panes retained; composition starts folded.
        await model.loadGraphCatalog()
        model.navigation.palette = .graph("feature")
        for _ in 0..<20 where model.graphCatalog.value == nil { try await settle(window) }
        try await settle(window)
        _ = try find("graph-composition-feature")
        #expect((try? find("compiled-node-4")) == nil, "composition starts folded")
        try find("composition-group-group-0").disclosureGroup().expand()
        try await settle(window)
        #expect(try find("compiled-node-4").accessibilityLabel().string() == "demo, interactive step, pending")

        let oldRevision = try #require(model.graphCatalog.value?.first?.composition?.revision)
        try await source.reviseTemplate()
        await model.loadGraphCatalog(force: true)
        try await settle(window)
        #expect(model.graphCatalog.value?.first?.composition?.revision != oldRevision)
        #expect((try? find("compiled-node-4")) == nil, "a new source revision starts folded")
        #expect(await source.controls.isEmpty)

        // A return outside a folded composition still names its semantic target.
        try await source.crossingReturns()
        await model.loadGraphCatalog(force: true)
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
                    #expect(try text("task-compiled-loop-\(key)") == "Loop \(number + 1)")
                    try pressElement("graph-node-\(key)", in: window)
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

        // Task controls stay out of the header; no Workflow bar is introduced.
        model.select(.task(id: "issue-review"))
        try await settle(window)
        #expect((try? find("task-lifecycle-menu")) == nil)
        #expect((try? find("task-compiled-start")) == nil)
        #expect((try? find("work-materials")) == nil)
        try captureIfRequested(window, name: "task-workspace-native")

        // The Session and its companion survived every Graph interaction.
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
private actor GraphSource {
    private var roadmap: [String: Any]
    private var catalog: String
    private let session: String
    private(set) var controls: [[String]] = []

    init(session: Data) throws {
        guard let roadmap = try JSONSerialization.jsonObject(with: fixture("roadmap_snapshot.json")) as? [String: Any],
              var entries = try JSONSerialization.jsonObject(with: fixture("graph_catalog.json")) as? [[String: Any]],
              var compiled = entries[0]["compiled"] as? [String: Any]
        else { throw RegistryQueryError("Graph proof fixtures are malformed") }
        self.roadmap = roadmap
        var build = entries[0]
        compiled["name"] = "build"
        build["name"] = "build"
        build["compiled"] = compiled
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
                 "interactive": false, "returns_to": index == 0 ? NSNull() : (prefix.isEmpty ? 0 : 4) as Any,
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
                      "interactive": false, "returns_to": NSNull(), "sources": ["feature"],
                      "paths": [["name": "fix", "description": "Repair", "steps": steps("3/fix/")]]])
        var tree = items("")
        tree.append(["kind": "node", "key": 3, "paths": ["fix": items("3/fix/")]])
        entries[0]["compiled"] = ["name": "feature", "steps": nodes]
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
        case ("desktop", "plan"):
            return String(decoding: try JSONSerialization.data(withJSONObject: roadmap), as: UTF8.self)
        case ("wave", "list"): return "[]"
        case ("history", _): return #"{"truncated":false,"items":[]}"#
        case ("desktop", "sessions"): return #"{"entries":\#(session),"next":null}"#
        case ("graph", "list"): return catalog
        case ("-b", "task") where args.dropFirst(2).first == "run":
            controls.append(args)
            // The fixture refuses W2-131's launch.
            if args[3] == "W2-131" { throw RegistryQueryError("Task graph \"build\" was refused by the fixture") }
            return ""
        default:
            throw RegistryQueryError("Unexpected Graph proof operation: \(args)")
        }
    }
}
#endif
#endif
