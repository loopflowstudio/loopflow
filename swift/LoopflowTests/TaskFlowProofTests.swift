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

@Suite("Task Flow")
struct TaskFlowTests {
    @Test("Flow snapshots and the catalogue decode every record without defaults")
    func flowFixtures() throws {
        let snapshots = try JSONDecoder().decode([TaskFlowSnapshot].self, from: fixture("task_flow.json"))
        #expect(snapshots.map(\.recommended) == ["feature", "feature", "feature", "build", "feature"])
        guard case .pinned(let running) = snapshots[1].record else {
            Issue.record("second snapshot is pinned"); return
        }
        #expect(running.returns.map(\.traversals) == [2, 0])
        #expect(running.returns.map(\.decider) == ["3", "5"])
        #expect(running.graph.steps.filter { $0.returnsTo == "1" }.map(\.key) == ["3", "5"])
        #expect(snapshots[1].control(.start)?.unavailable != nil)
        #expect(snapshots[1].controls.map(\.kind) == [.start, .resume, .restart])
        #expect(snapshots[4].record == .finished(flow: "build"))

        var missing = try #require(JSONSerialization.jsonObject(with: fixture("task_flow.json")) as? [[String: Any]])
        let record = try #require(missing[1]["record"] as? [String: Any])
        for field in ["returns", "iterations"] {
            var incomplete = record
            incomplete.removeValue(forKey: field)
            missing[1]["record"] = incomplete
            #expect(throws: DecodingError.self) {
                try JSONDecoder().decode([TaskFlowSnapshot].self, from: JSONSerialization.data(withJSONObject: missing))
            }
        }

        let catalog = try JSONDecoder().decode([FlowCatalogEntry].self, from: fixture("flow_catalog.json"))
        #expect(catalog.map(\.name) == ["feature", "broken"])
        #expect(catalog[0].graph?.steps.count == 8 && catalog[1].unavailable != nil)
    }

    @Test("Occurrence state keeps pass completions while iteration keeps each edge count")
    func occurrenceStates() throws {
        let snapshots = try JSONDecoder().decode([TaskFlowSnapshot].self, from: fixture("task_flow.json"))
        guard case .pinned(let human) = snapshots[2].record else { Issue.record("pinned"); return }
        let states = flowNodeStates(human.graph, pinned: human)
        #expect(states["1"] == .completed && states["3"] == .completed)
        #expect(states["4"] == .waitingForHuman)
        // The second decision is pending again in this pass; the router pends too.
        #expect(states["5"] == .pending && states["6"] == .pending)
        #expect(states["6/fix/0"] == .pending)
        #expect(human.iterations == [[1, 1]])
        #expect(human.returns[1].traversals == 1)
        guard case .pinned(let running) = snapshots[1].record else { Issue.record("pinned"); return }
        #expect(running.iterations == [[2, 0]])
        #expect(flowIterationLabel([[2, 1], [3]]) == "(2, 1) / (3)")
        #expect(flowIterationLabel([[]]) == nil)
        #expect(running.returns.map(\.traversals) == [2, 0])

        guard case .pinned(let blocked) = snapshots[3].record else { Issue.record("pinned"); return }
        #expect(flowNodeStates(blocked.graph, pinned: blocked)["3"] == .blocked)
        // A preview only marks human boundaries.
        let preview = flowNodeStates(human.graph, pinned: nil)
        #expect(preview["4"] == .pendingHuman && preview["1"] == .pending)
    }
}

#if canImport(GhosttyKit)
@Suite("Task Flow native proof", .serialized)
@MainActor
struct TaskFlowProofTests {
    @Test("Flow preview, controls and execution updates keep the Session's terminal, draft and companion")
    func flowControlsRetainTerminals() async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        let repo = "/src/loopflow"
        let registry = SessionsWorkspaceRegistry()
        let workspace = registry.workspace(for: repo)
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
        session["actions"] = sessionActionFixture(kind: "interactive", state: "active")
        session["terminal_ids"] = [shells[0]]
        session["open_argv"] = ["must-not-launch"]
        let source = try FlowSource(session: JSONSerialization.data(withJSONObject: [session]))
        let query = RegistryQuery { args, _ in try await source.respond(args) }
        let model = PodiumModel(query: query, repoPath: repo)
        await model.refresh()
        model.navigation.content = .terminals
        let view = SessionsView(model: model, repoPath: repo, workspaces: registry, query: query)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1500, height: 820),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: view)
        defer { window.contentView = nil }
        try await settle(window)
        let draft = "flow-proof-draft"
        draft.withCString { ghostty_surface_text(surfaces[0], $0, UInt(draft.utf8.count)) }

        func find(_ id: String) throws -> InspectableView<ViewType.ClassifiedView> {
            try view.inspect().find(viewWithAccessibilityIdentifier: id)
        }
        func text(_ id: String) throws -> String { try find(id).text().string() }

        // Unstarted: the recommendation is previewed with both return edges.
        model.select(.task(id: "issue-available"))
        model.navigation.content = .details
        for _ in 0..<20 where model.flowCatalog.value == nil { try await settle(window) }
        try await settle(window)
        #expect(try text("task-flow-status") == "Not started · No runs yet.")
        #expect(try find("task-flow-loop-3").text().string() == "Loop 1")
        #expect(try find("task-flow-loop-5").text().string() == "Loop 2")
        #expect((try? find("task-flow-iteration")) == nil, "a preview has no iteration")
        #expect(try find("flow-node-4").accessibilityLabel().string() == "demo, human review, pending")

        // Typeahead: Cancel keeps the recommendation; choosing previews only.
        try find("task-flow-name").button().tap()
        try await settle(window)
        try find("task-flow-search").textField().setInput("bu")
        try await settle(window)
        #expect(throws: (any Error).self) { try find("task-flow-option-feature") }
        try find("task-flow-search-cancel").button().tap()
        try await settle(window)
        #expect(throws: (any Error).self) { try find("task-flow-search") }
        #expect(try find("task-flow-name").accessibilityLabel().string() == "Flow feature, choose another")
        try find("task-flow-name").button().tap()
        try await settle(window)
        #expect(try find("task-flow-option-broken").button().isDisabled())
        try find("task-flow-option-build").button().tap()
        try await settle(window)
        #expect(try find("task-flow-name").accessibilityLabel().string() == "Flow build, choose another")
        #expect(await source.controls.isEmpty, "choosing a preview mutates nothing")
        try find("task-flow-start").button().tap()
        for _ in 0..<20 where await source.controls.isEmpty { try await settle(window) }
        #expect(await source.controls == [["task", "run", "W2-156", "--flow", "build"]])

        // Pinned and waiting for review: the saved position, not the catalogue.
        model.select(.task(id: "issue-review"))
        try await settle(window)
        #expect(try text("task-flow-status") == "Waiting for your review at demo")
        #expect(try find("flow-node-4").accessibilityLabel().string() == "demo, waiting for your review")
        #expect(try find("flow-node-1").accessibilityLabel().string() == "implement, completed this pass")
        // Each loop labels its own returns; the header carries the tuple.
        #expect(try find("task-flow-loop-3").text().string() == "Loop 1 · 1 return")
        #expect(try find("task-flow-loop-5").text().string() == "Loop 2 · 1 return")
        #expect(try text("task-flow-iteration") == "Iteration (1, 1)")
        #expect(try find("task-flow-resume").button().isDisabled())
        try captureIfRequested(window, name: "task-flow-pinned")

        // Stop & restart: Cancel leaves everything; a rejected replacement keeps
        // the pinned Flow and names the failure.
        try find("task-flow-name").button().tap()
        try await settle(window)
        try find("task-flow-option-build").button().tap()
        try await settle(window)
        _ = try find("task-flow-restart-confirmation")
        try find("task-flow-restart-cancel").button().tap()
        try await settle(window)
        #expect(throws: (any Error).self) { try find("task-flow-restart-confirmation") }
        #expect(await source.controls.count == 1)
        try find("task-flow-name").button().tap()
        try await settle(window)
        try find("task-flow-option-build").button().tap()
        try await settle(window)
        try find("task-flow-restart-confirm").button().tap()
        for _ in 0..<20 where (try? find("task-flow-error")) == nil { try await settle(window) }
        #expect(try text("task-flow-error") == "Task flow \"build\" was refused by the fixture")
        #expect(await source.controls.last == ["task", "restart", "W2-131", "--flow", "build"])
        #expect(try find("flow-node-4").accessibilityLabel().string() == "demo, waiting for your review")

        // Execution advances through the shared read; the graph follows it.
        await source.advanceReview()
        await model.refresh()
        try await settle(window)
        #expect(try text("task-flow-status") == "Worker is running review-slice")
        #expect(try find("flow-node-2").accessibilityLabel().string() == "review-slice, running")
        #expect((try? find("task-flow-pause")) == nil)
        try captureIfRequested(window, name: "task-flow-running")

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

    private func settle(_ window: NSWindow) async throws {
        window.contentView?.layoutSubtreeIfNeeded()
        window.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(100))
    }

    private func captureIfRequested(_ window: NSWindow, name: String) throws {
        guard let directory = ProcessInfo.processInfo.environment["LOOPFLOW_FLOW_CAPTURE_DIR"],
              let content = window.contentView,
              let bitmap = content.bitmapImageRepForCachingDisplay(in: content.bounds) else { return }
        content.cacheDisplay(in: content.bounds, to: bitmap)
        let png = try #require(bitmap.representation(using: .png, properties: [:]))
        try png.write(to: URL(fileURLWithPath: directory).appendingPathComponent("\(name).png"))
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

/// Shared reads plus recorded controls. Only `task run`/`task restart`/
/// `task resume` count as controls; anything else unexpected fails loudly.
private actor FlowSource {
    private var roadmap: [String: Any]
    private let catalog: String
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

    func respond(_ args: [String]) throws -> String {
        switch (args.first, args.dropFirst().first) {
        case ("roadmap", _):
            return String(decoding: try JSONSerialization.data(withJSONObject: roadmap), as: UTF8.self)
        case ("ls", _): return "[]"
        case ("activity", _): return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
        case ("session", "list"): return session
        case ("flow", "list"): return catalog
        case ("task", "run"), ("task", "resume"):
            controls.append(args)
            return ""
        case ("task", "restart"):
            controls.append(args)
            throw RegistryQueryError("Task flow \"build\" was refused by the fixture")
        default:
            throw RegistryQueryError("Unexpected Flow proof operation: \(args)")
        }
    }

    /// Replace W2-131's Flow with the fixture's running position.
    func advanceReview() {
        let snapshots = (try? JSONSerialization.jsonObject(with: fixture("task_flow.json"))) as? [[String: Any]]
        guard let running = snapshots?[1],
              var waves = roadmap["waves"] as? [[String: Any]],
              var tasks = waves[0]["tasks"] as? [String: Any],
              var items = tasks["items"] as? [[String: Any]],
              let index = items.firstIndex(where: { ($0["task"] as? [String: Any])?["identifier"] as? String == "W2-131" })
        else { return }
        items[index]["flow"] = running
        tasks["items"] = items
        waves[0]["tasks"] = tasks
        roadmap["waves"] = waves
    }
}
#endif
#endif
