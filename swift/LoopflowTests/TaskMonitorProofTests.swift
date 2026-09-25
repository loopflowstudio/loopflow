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

@Suite("Task Monitor integration proof", .serialized)
@MainActor
struct TaskMonitorProofTests {
    @Test("Focusing another Task's pane cannot replace the selected Task's saved choice")
    func paneFocusPreservesTaskChoice() async throws {
        _ = NSApplication.shared
        let query = try query()
        let model = PodiumModel(query: query, repoPath: "/src/loopflow")
        await model.refresh()
        let registry = SessionsWorkspaceRegistry()
        let multiplexer = registry.workspace(for: "/src/loopflow").multiplexer
        multiplexer.load(sessionId: "monitor-review")
        let ownPane = multiplexer.focusedPaneId
        _ = multiplexer.split(ownPane, axis: .vertical)
        multiplexer.load(sessionId: "monitor-other")
        let otherPane = multiplexer.focusedPaneId
        let view = SessionsView(model: model, repoPath: "/src/loopflow", workspaces: registry, query: query)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1100, height: 650),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: view)
        defer { window.contentView = nil }
        try await settle(window)
        let navigator = try view.inspect().find(WorkspaceNavigator.self).actualView()
        navigator.onOpenSession(try #require(model.sessions.value?.first))
        try await settle(window)
        try #require(model.navigation.taskPanes["issue-review"]?.pane.id == ownPane)
        multiplexer.setFocusedPane(otherPane)
        try await settle(window)
        let openTask = try #require(navigator.onOpenTask)
        openTask(.task(id: "issue-review"))
        try await settle(window)
        #expect(multiplexer.focusedPaneId == ownPane)
        #expect(model.navigation.selectedSessionId == "monitor-review")
    }

    @Test("Returning to a Task never restores another Task's Session in a reused pane")
    func returningTaskDoesNotRestoreReusedSessionPane() async throws {
        let query = try query()
        let model = PodiumModel(query: query, repoPath: "/src/loopflow")
        await model.refresh()
        let registry = SessionsWorkspaceRegistry()
        let view = SessionsView(model: model, repoPath: "/src/loopflow", workspaces: registry, query: query)
        let records = try #require(model.sessions.value)
        let store = registry.workspace(for: "/src/loopflow").sessionStore(repoPath: "/src/loopflow", query: query)
        store.reconcile(records)
        let navigator = try view.inspect().find(WorkspaceNavigator.self).actualView()
        navigator.onOpenSession(records[0])
        let multiplexer = registry.workspace(for: "/src/loopflow").multiplexer
        let firstPane = multiplexer.focusedPaneId
        navigator.onOpenSession(records[1])
        try #require(multiplexer.focusedPaneId == firstPane)
        try #require(multiplexer.focusedPane.content == .session(id: "monitor-other"))
        let openTask = try #require(navigator.onOpenTask)
        openTask(.task(id: "issue-review"))
        #expect(model.selection == .task(id: "issue-review"))
        #expect(multiplexer.focusedPane.content != .session(id: "monitor-other"))
        #expect(model.navigation.selectedSessionId != "monitor-other")
    }

#if canImport(GhosttyKit)
    @Test("Chapter transfer preserves Monitor attribution, Session draft and companion in the same split tree")
    func monitorRetainsNativeDraftAndCompanion() async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        try #require(GhosttyManager.shared.state == .ready)
        let feed = ActiveRunsTestFeed()
        let query = try query(feed: feed)
        let model = PodiumModel(query: query, repoPath: "/src/loopflow")
        await model.refresh()
        let registry = SessionsWorkspaceRegistry()
        let workspace = registry.workspace(for: "/src/loopflow")
        let multiplexer = workspace.multiplexer
        multiplexer.load(sessionId: "monitor-review")
        let sessionPane = multiplexer.focusedPaneId
        multiplexer.newShell()
        let shellPane = multiplexer.focusedPaneId
        multiplexer.setFocusedPane(sessionPane)
        let identities: [TerminalIdentity] = [.session("monitor-review"), .shell(shellPane)]
        let terminals = identities.map { registry.surfaces.view(for: $0) }
        defer { for identity in identities { registry.surfaces.release(identity) } }
        for terminal in terminals {
            terminal.setFrameSize(CGSize(width: 500, height: 350))
            terminal.workingDirectory = NSTemporaryDirectory()
            terminal.command = buildGhosttyShellCommand(argv: ["/bin/cat"], env: [:])
            terminal.createSurface(manager: GhosttyManager.shared)
        }
        let surfaces = try terminals.map { try #require($0.surface) }
        model.select(.task(id: "issue-review"))
        model.navigation.content = .terminals
        let view = SessionsView(model: model, repoPath: "/src/loopflow", workspaces: registry, query: query)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1400, height: 750),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: view)
        defer { window.contentView = nil }
        try await settle(window)
        let draft = "monitor-retained-draft"
        draft.withCString { ghostty_surface_text(surfaces[0], $0, UInt(draft.utf8.count)) }
        try view.inspect().find(ViewType.Button.self, where: {
            try $0.accessibilityIdentifier() == "task-show-monitor-issue-review"
        }).tap()
        try await settle(window)
        let monitorPane = multiplexer.focusedPaneId
        try #require(multiplexer.focusedPane.content == .monitor(taskId: "issue-review"))
        #expect(multiplexer.layout.allPanes.count == 3)
        #expect(multiplexer.layout.pane(for: sessionPane)?.content == .session(id: "monitor-review"))
        #expect(multiplexer.layout.pane(for: shellPane)?.content == .shell)
        #expect(!terminals.contains { window.firstResponder === $0 })
        let sessionRun = model.sessions.value?.first?.runId
        let taskWork = model.task(id: "issue-review")?.task.runtime?.workId
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json"))
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var waves = try #require(wire["waves"] as? [[String: Any]])
        var chapter = try #require(waves[0]["chapter"] as? [String: Any])
        var tasks = try #require(waves[0]["tasks"] as? [String: Any])
        var items = try #require(tasks["items"] as? [[String: Any]])
        for index in items.indices {
            if var runtime = items[index]["runtime"] as? [String: Any] {
                runtime["project_id"] = "successor-work"
                items[index]["runtime"] = runtime
            }
        }
        tasks["items"] = items
        chapter["id"] = "next-chapter"
        chapter["source_project_id"] = "successor-project"
        chapter["source_work_id"] = "successor-work"
        for transferring in [true, false] {
            chapter["phase"] = transferring ? "transferring" : "complete"
            waves[0]["chapter"] = chapter
            waves[0]["tasks"] = transferring ? ["state": "ok", "items": [], "truncated": false] : tasks
            wire["waves"] = waves
            let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: wire))
            model.applyFixture(roadmap: .available(snapshot), waves: model.waves,
                processActivity: model.processActivity, workActivity: model.workActivity, repos: model.repos)
            try await settle(window)
            #expect(model.selection == .task(id: "issue-review"))
            #expect(model.task(id: "issue-review")?.task.runtime?.workId == taskWork)
            if !transferring { #expect(model.task(id: "issue-review")?.task.runtime?.projectId == "successor-work") }
            #expect(model.sessions.value?.first?.runId == sessionRun)
            #expect(multiplexer.focusedPaneId == monitorPane)
            #expect(terminals[0].surface == surfaces[0])
            #expect(terminals[1].surface == surfaces[1])
            #expect(throws: Never.self) { try TaskMonitorView(taskId: "issue-review", model: model).inspect().find(text: "Retained Task Run") }
        }
        // Automatic delivery and recovery occur while the original unfinished input
        // and companion are retained. No Refresh or pane reopening drives these frames.
        let original = try #require(model.activeRuns.value)
        let scanning = ActiveRunsSnapshot(discovery: .scanning, home: original.home,
            observedAt: original.observedAt, task: nil, runs: [], gaps: ["Recovering coverage"])
        try await feed.send(String(decoding: JSONEncoder().encode(scanning), as: UTF8.self))
        try await waitForActiveRuns { model.isRefreshingActiveRuns }
        #expect(model.activeRuns.value == original)
        #expect(!terminals.contains { window.firstResponder === $0 })
        let empty = ActiveRunsSnapshot(discovery: .ready, home: original.home,
            observedAt: original.observedAt + 1, task: nil, runs: [], gaps: [])
        try await feed.send(String(decoding: JSONEncoder().encode(empty), as: UTF8.self))
        try await waitForActiveRuns { model.activeRuns.value?.runs.isEmpty == true }
        try await settle(window)
        #expect(terminals[0].surface == surfaces[0])
        #expect(terminals[1].surface == surfaces[1])
        #expect(!terminals.contains { window.firstResponder === $0 })
        try await feed.send(String(decoding: JSONEncoder().encode(original), as: UTF8.self))
        try await waitForActiveRuns { model.activeRuns.value?.runs.count == original.runs.count }
        let stray = "monitor-only-input"
        for character in stray {
            let event = try #require(NSEvent.keyEvent(
                with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
                windowNumber: window.windowNumber, context: nil,
                characters: String(character), charactersIgnoringModifiers: String(character),
                isARepeat: false, keyCode: 0
            ))
            window.sendEvent(event)
        }
        try await settle(window)
        #expect(!terminalText(surfaces[0]).contains(stray))
        #expect(!terminalText(surfaces[1]).contains(stray))
        multiplexer.toggleZoom(monitorPane)
        try await settle(window)
        multiplexer.toggleZoom(monitorPane)
        multiplexer.updateRatio(between: sessionPane, and: monitorPane, ratio: 0.65)
        try await settle(window)
        let navigator = try view.inspect().find(WorkspaceNavigator.self).actualView()
        navigator.onOpenSession(try #require(model.sessions.value?.first))
        try await settle(window)
        #expect(multiplexer.focusedPaneId == sessionPane)
        #expect(window.firstResponder === terminals[0])
        #expect(terminals[0].surface == surfaces[0])
        #expect(terminals[1].surface == surfaces[1])
        "\n".withCString { ghostty_surface_text(surfaces[0], $0, 1) }
        let companion = "monitor-companion-alive"
        let input = companion + "\n"
        input.withCString { ghostty_surface_text(surfaces[1], $0, UInt(input.utf8.count)) }
        let deadline = ContinuousClock.now + .seconds(3)
        while (terminalText(surfaces[0]).components(separatedBy: draft).count < 3
            || terminalText(surfaces[1]).components(separatedBy: companion).count < 3), ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(terminalText(surfaces[0]).components(separatedBy: draft).count == 3)
        #expect(terminalText(surfaces[1]).components(separatedBy: companion).count == 3)
        multiplexer.close(monitorPane)
        #expect(terminals[0].surface == surfaces[0])
        #expect(terminals[1].surface == surfaces[1])
    }

    private func terminalText(_ surface: ghostty_surface_t) -> String {
        var text = ghostty_text_s()
        let selection = ghostty_selection_s(
            top_left: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_TOP_LEFT, x: 0, y: 0),
            bottom_right: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_BOTTOM_RIGHT, x: 0, y: 0), rectangle: false)
        guard ghostty_surface_read_text(surface, selection, &text) else { return "" }
        defer { ghostty_surface_free_text(surface, &text) }
        guard let bytes = text.text else { return "" }
        return String(decoding: Data(bytes: bytes, count: Int(text.text_len)), as: UTF8.self)
    }
#endif

    private func settle(_ window: NSWindow) async throws {
        window.contentView?.layoutSubtreeIfNeeded()
        window.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(100))
    }

    private func query(feed: ActiveRunsTestFeed = ActiveRunsTestFeed()) throws -> RegistryQuery {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let roadmap = try String(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json"), encoding: .utf8)
        let records = """
        [{"id":"monitor-review","run_id":"monitor-review","kind":"interactive",
          "work":{"kind":"task","id":"ts_review00000000000000000000000000"},
          "title":"Review Session","detail":"Owned cat PTY","cwd":"/src/loopflow",
          "state":"active","ready_summary":null,"work_path":null,"actions":\(sessionActionFixtureJSON(kind: "interactive", state: "active")),"terminal_ids":[],"open_argv":["/bin/cat"]},
         {"id":"monitor-other","run_id":"monitor-other","kind":"interactive",
          "work":{"kind":"task","id":"ts_now00000000000000000000000000000"},
          "title":"Other Task Session","detail":"Owned cat PTY","cwd":"/src/loopflow",
          "state":"active","ready_summary":null,"work_path":null,"actions":\(sessionActionFixtureJSON(kind: "interactive", state: "active")),"terminal_ids":[],"open_argv":["/bin/cat"]}]
        """
        var active = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/active_runs.json"))) as? [String: Any])
        var run = try #require((active["runs"] as? [[String: Any]])?.first)
        run["id"] = "monitor-review"
        run["work"] = ["kind": "task", "id": "ts_review00000000000000000000000000"]
        run["label"] = "Retained Task Run"
        active["runs"] = [run]
        active["gaps"] = []
        let activeJSON = String(decoding: try JSONSerialization.data(withJSONObject: active), as: UTF8.self)
        return RegistryQuery(watchActiveRuns: { try await feed.open(initial: activeJSON) }) { args, _ in
            switch args.first {
            case "roadmap": return roadmap
            case "ls": return "[]"
            case "session" where args.dropFirst().first == "list": return records
            case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            default: throw RegistryQueryError("No provider launch is available in this proof")
            }
        }
    }
}
#endif
