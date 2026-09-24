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

@Suite("Task Monitor", .serialized)
@MainActor
struct TaskMonitorTests {
    @Test("Monitor isolates Task identity and distinguishes stale, incomplete and empty observations")
    func taskEvidence() async throws {
        let frames = MonitorFrames([
            try activeRuns(), nil,
            try activeRuns(empty: true, gaps: ["ownership unavailable"]),
            try activeRuns(empty: true),
        ])
        let query = try query(frames: frames)
        let model = PodiumModel(query: query, repoPath: "/src/loopflow")
        await model.refresh()
        await model.refreshActiveRuns()
        let view = TaskMonitorView(taskId: "issue-now", model: model)
        #expect(throws: Never.self) { try view.inspect().find(text: "This Task's Run") }
        #expect(throws: (any Error).self) { try view.inspect().find(text: "Other Task's Run") }
        model.select(.task(id: "issue-review"))
        // Changing selection never redirects the retained Monitor's subject.
        #expect(throws: Never.self) { try view.inspect().find(text: "This Task's Run") }
        await model.refreshActiveRuns()
        #expect(throws: Never.self) { try view.inspect().find(text: "Showing the last successful observation.") }
        #expect(throws: Never.self) { try view.inspect().find(text: "This Task's Run") }
        await model.refreshActiveRuns()
        #expect(throws: Never.self) { try view.inspect().find(text: "No matching Runs in the available evidence") }
        #expect(throws: (any Error).self) { try view.inspect().find(text: "No active Runs in this observation") }
        await model.refreshActiveRuns()
        #expect(throws: Never.self) { try view.inspect().find(text: "No active Runs in this observation") }
        #expect(model.activeRuns.errorMessage == nil)
    }

    @Test("Monitor shares split, focus, zoom, Close and Undo with terminals")
    func mixedPaneLayout() {
        let store = MultiplexerStore()
        store.load(sessionId: "session")
        let session = store.focusedPane
        store.newShell()
        let shell = store.focusedPane
        store.showMonitor(taskId: "task")
        let monitor = store.focusedPane
        #expect(store.layout.allPanes == [session, shell, monitor])
        store.showMonitor(taskId: "task")
        #expect(store.layout.allPanes.count == 3)
        store.toggleZoom(monitor.id)
        store.load(sessionId: "session")
        #expect(store.zoomedPaneId == session.id)
        store.showMonitor(taskId: "task")
        #expect(store.zoomedPaneId == monitor.id)
        store.close(monitor.id)
        #expect(store.layout.allPanes == [session, shell])
        store.undoClose()
        #expect(store.layout.allPanes == [session, shell, monitor])
        store.reconcileSessions([])
        #expect(store.layout.allPanes == [shell, monitor])
        store.load(sessionId: "another-session")
        #expect(store.layout.allPanes.contains(monitor))
    }

#if canImport(GhosttyKit)
    @Test("Task selection and Monitor preserve native Session input and its companion")
    func mixedMonitorRetainsInput() async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        let repo = "/src/loopflow"
        let registry = SessionsWorkspaceRegistry()
        let store = registry.workspace(for: repo).multiplexer
        var terminals: [GhosttyMetalView] = []
        for _ in 0..<2 {
            store.newShell()
            let pane = store.focusedPaneId
            let terminal = registry.surfaces.view(for: .shell(pane))
            terminal.frame = CGRect(x: 0, y: 0, width: 500, height: 350)
            terminal.workingDirectory = NSTemporaryDirectory()
            terminal.command = buildWorkspaceShellCommand(id: pane, argv: ["/bin/cat"], env: [:])
            terminal.createSurface(manager: GhosttyManager.shared)
            terminals.append(terminal)
        }
        defer { for terminal in terminals { registry.surfaces.release(terminal.terminal) } }
        let surfaces = try terminals.map { try #require($0.surface) }
        let shells = store.layout.allPanes
        var session = try #require(JSONSerialization.jsonObject(with: fixture("session")) as? [String: Any])
        session["id"] = "monitor-session"
        session["kind"] = "interactive"
        session["work"] = ["kind": "task", "id": "ts_now00000000000000000000000000000"]
        session["cwd"] = repo
        session["state"] = "active"
        session["actions"] = []
        session["terminal_ids"] = [shells[0].id]
        let sessionJSON = String(decoding: try JSONSerialization.data(withJSONObject: [session]), as: UTF8.self)
        let query = try query(frames: MonitorFrames([try activeRuns()]), sessions: sessionJSON)
        let model = PodiumModel(query: query, repoPath: repo)
        await model.refresh()
        let view = SessionsView(model: model, repoPath: repo, workspaces: registry, query: query)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1400, height: 750),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: view)
        defer { window.contentView = nil }
        try await settle(window)
        try view.inspect().find(viewWithAccessibilityIdentifier: "session-row-monitor-session").button().tap()
        try await settle(window)
        #expect(window.firstResponder === terminals[0])
        let draft = "draft-survives-monitor"
        draft.withCString { ghostty_surface_text(surfaces[0], $0, UInt(draft.utf8.count)) }

        try view.inspect().find(viewWithAccessibilityIdentifier: "task-show-monitor-issue-now").button().tap()
        try await settle(window)
        let monitor = store.focusedPane
        #expect(monitor.content == .monitor(taskId: "issue-now"))
        #expect(store.layout.allPanes.filter { $0.content == .shell } == shells)
        #expect(window.firstResponder !== terminals[0])
        #expect(terminals.allSatisfy { $0.window === window })
        store.toggleZoom(monitor.id)
        window.setContentSize(NSSize(width: 1100, height: 650))
        try await settle(window)
        store.toggleZoom(monitor.id)
        try view.inspect().find(viewWithAccessibilityIdentifier: "session-row-monitor-session").button().tap()
        try await settle(window)
        #expect(window.firstResponder === terminals[0])
        try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-task-issue-available").button().tap()
        try await settle(window)
        #expect(store.focusedPane.content == .monitor(taskId: "issue-available"))
        try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-task-issue-now").button().tap()
        try await settle(window)
        #expect(store.focusedPaneId == shells[0].id)
        #expect(window.firstResponder === terminals[0])
        store.close(monitor.id)
        try await settle(window)
        for (index, surface) in surfaces.enumerated() {
            #expect(terminals[index].surface == surface)
            let reply = index == 0 ? draft : "companion-survives-monitor"
            let input = index == 0 ? "\n" : reply + "\n"
            input.withCString { ghostty_surface_text(surface, $0, UInt(input.utf8.count)) }
            let deadline = ContinuousClock.now + .seconds(3)
            while terminalText(surface).components(separatedBy: reply).count < 3, ContinuousClock.now < deadline {
                try await Task.sleep(for: .milliseconds(20))
            }
            #expect(terminalText(surface).components(separatedBy: reply).count == 3)
        }
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

    private func settle(_ window: NSWindow) async throws {
        window.contentView?.layoutSubtreeIfNeeded()
        window.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(100))
    }
#endif

    private func fixture(_ name: String) throws -> Data {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        return try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/\(name).json"))
    }

    private func activeRuns(empty: Bool = false, gaps: [String] = []) throws -> String {
        var snapshot = try #require(JSONSerialization.jsonObject(with: fixture("active_runs")) as? [String: Any])
        var run = try #require((snapshot["runs"] as? [[String: Any]])?.first)
        run["work"] = ["kind": "task", "id": "ts_now00000000000000000000000000000"]
        run["label"] = "This Task's Run"
        var other = run
        other["id"] = "other-run"
        other["work"] = ["kind": "task", "id": "ts_review00000000000000000000000000"]
        other["label"] = "Other Task's Run"
        snapshot["runs"] = empty ? [] : [run, other]
        snapshot["gaps"] = gaps
        snapshot["task"] = NSNull()
        return String(decoding: try JSONSerialization.data(withJSONObject: snapshot), as: UTF8.self)
    }

    private func query(frames: MonitorFrames, sessions: String = "[]") throws -> RegistryQuery {
        var roadmap = try #require(JSONSerialization.jsonObject(with: fixture("roadmap_snapshot")) as? [String: Any])
        var waves = try #require(roadmap["waves"] as? [[String: Any]])
        var projects = try #require(waves[0]["projects"] as? [String: Any])
        var items = try #require(projects["items"] as? [[String: Any]])
        var tasks = try #require(items[0]["tasks"] as? [[String: Any]])
        for index in tasks.indices {
            // Tasks without local checkouts use the repository's existing workspace.
            tasks[index]["reference"] = ["issue_url": NSNull(), "workspace": NSNull()]
        }
        items[0]["tasks"] = tasks
        projects["items"] = items
        waves[0]["projects"] = projects
        roadmap["waves"] = waves
        let text = String(decoding: try JSONSerialization.data(withJSONObject: roadmap), as: UTF8.self)
        return RegistryQuery { args, _ in
            switch args.first {
            case "roadmap": return text
            case "ls": return "[]"
            case "session" where args.dropFirst().first == "list": return sessions
            case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            case "runs": return try await frames.next()
            default: throw RegistryQueryError("Unexpected action in Monitor proof")
            }
        }
    }
}

private actor MonitorFrames {
    private var frames: [String?]
    init(_ frames: [String?]) { self.frames = frames }
    func next() throws -> String {
        guard !frames.isEmpty, let frame = frames.removeFirst() else {
            throw RegistryQueryError("read failed")
        }
        return frame
    }
}
#endif
