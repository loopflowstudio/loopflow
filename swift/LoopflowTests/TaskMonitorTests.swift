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
        let feed = ActiveSessionsTestFeed()
        let query = try query(feed: feed)
        let model = PodiumModel(query: query, repoPath: "/src/loopflow")
        await model.refresh()
        model.observeActiveSessions()
        try await waitForActiveSessions { model.activeSessions.value != nil }
        let view = TaskMonitorView(taskId: "issue-now", model: model)
        let other = TaskMonitorView(taskId: "issue-review", model: model)
        #expect(throws: Never.self) { try view.inspect().find(text: "This Task's Session") }
        #expect(throws: (any Error).self) { try view.inspect().find(text: "Other Task's Session") }
        #expect(throws: Never.self) { try other.inspect().find(text: "Other Task's Session") }
        model.select(.task(id: "issue-review"))
        model.setRepoPath("/src/another")
        model.setRepoPath("/src/loopflow")
        model.observeActiveSessions()
        #expect(await feed.starts == 1)
        // Recovery retains both panes' last observation; it cannot claim emptiness.
        try await feed.send(activeSessions(empty: true, discovery: "scanning"))
        try await waitForActiveSessions { model.activeSessions.errorMessage != nil }
        #expect(throws: Never.self) { try view.inspect().find(text: "This Task's Session") }
        #expect(throws: Never.self) { try other.inspect().find(text: "Other Task's Session") }
        #expect(throws: (any Error).self) { try view.inspect().find(text: "No active Sessions in this observation") }
        try await feed.send(activeSessions(empty: true, gaps: ["ownership unavailable"]))
        try await waitForActiveSessions { model.activeSessions.value?.sessions.isEmpty == true }
        #expect(throws: Never.self) { try view.inspect().find(text: "No matching Sessions in the available evidence") }
        try await feed.send(activeSessions(empty: true))
        try await waitForActiveSessions { model.activeSessions.value?.gaps.isEmpty == true }
        #expect(throws: Never.self) { try view.inspect().find(text: "No active Sessions in this observation") }
        await feed.fail()
        try await waitForActiveSessions { model.activeSessionsNeedsRetry && !model.isRefreshingActiveSessions }
        model.observeActiveSessions()
        #expect(await feed.starts == 1)
        #expect(throws: Never.self) { try view.inspect().find(text: "Showing the last successful observation.") }
        await model.refreshActiveSessions()
        try await waitForActiveSessions { model.activeSessions.value?.sessions.count == 2 }
        #expect(await feed.starts == 2)
        try await feed.send(activeSessions(empty: true, gaps: ["receipt root replaced"], discovery: "unavailable"))
        try await waitForActiveSessions { model.activeSessionsNeedsRetry && !model.isRefreshingActiveSessions }
        #expect(model.activeSessions.errorMessage?.contains("receipt root replaced") == true)
        #expect(model.activeSessions.value?.sessions.count == 2)
        #expect(await feed.cancellations == 2)
        await model.stopActiveSessions()
        #expect(await feed.cancellations == 2)
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
        session["kind"] = "conversation"
        session["work"] = ["kind": "task", "id": "ts_now00000000000000000000000000000"]
        session["cwd"] = repo
        session["state"] = "active"
        session["actions"] = []
        session["terminal_ids"] = [shells[0].id]
        let sessionJSON = String(decoding: try JSONSerialization.data(withJSONObject: [session]), as: UTF8.self)
        let query = try query(feed: ActiveSessionsTestFeed(), sessions: sessionJSON)
        let model = PodiumModel(query: query, repoPath: repo)
        await model.refresh()
        let view = SessionsView(model: model, repoPath: repo, workspaces: registry, query: query)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1400, height: 750),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: view)
        defer { window.contentView = nil }
        try await settle(window)
        // A Task with one open Session drills straight into that Session.
        try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-task-issue-now").button().tap()
        try await settle(window)
        #expect(model.navigation.selectedSessionId == "monitor-session")
        #expect(window.firstResponder === terminals[0])
        let draft = "draft-survives-monitor"
        draft.withCString { ghostty_surface_text(surfaces[0], $0, UInt(draft.utf8.count)) }

        try view.inspect().find(viewWithAccessibilityIdentifier: "task-show-monitor-issue-now").button().tap()
        // Splitting changes the representable hierarchy around retained views.
        // Observe attachment; a fixed delay cannot establish that remount finished.
        let mountDeadline = ContinuousClock.now + .seconds(3)
        repeat {
            window.contentView?.layoutSubtreeIfNeeded()
            window.layoutIfNeeded()
            await Task.yield()
        } while !terminals.allSatisfy({ $0.window === window }) && ContinuousClock.now < mountDeadline
        let monitor = store.focusedPane
        #expect(monitor.content == .monitor(taskId: "issue-now"))
        #expect(store.layout.allPanes.filter { $0.content == .shell } == shells)
        #expect(window.firstResponder !== terminals[0])
        for (index, terminal) in terminals.enumerated() {
            let ancestors = sequence(first: terminal.superview, next: { $0?.superview })
                .prefix(8).compactMap { view in view.map { "\(type(of: $0)) frame=\($0.frame)" } }
            #expect(terminal.window === window,
                "terminal=\(index) id=\(terminal.terminal) window=\(String(describing: terminal.window)) target=\(window.windowNumber) sameSurface=\(terminal.surface == surfaces[index]) focused=\(window.firstResponder === terminal) ancestors=\(ancestors) layout=\(store.layout) lifecycle=\(terminal.mountLifecycle)")
        }
        store.toggleZoom(monitor.id)
        window.setContentSize(NSSize(width: 1100, height: 650))
        try await settle(window)
        store.toggleZoom(monitor.id)
        try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-task-issue-now").button().tap()
        try await settle(window)
        #expect(window.firstResponder === terminals[0])
        // A started Task without Sessions opens its overview; terminals stay retained.
        try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-task-issue-review").button().tap()
        try await settle(window)
        #expect(model.navigation.content == .details)
        #expect(model.selection == .task(id: "issue-review"))
        #expect(window.firstResponder !== terminals[0])
        #expect(store.layout.allPanes.filter { $0.content == .shell } == shells)
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

    private func activeSessions(empty: Bool = false, gaps: [String] = [], discovery: String = "ready") throws -> String {
        var snapshot = try #require(JSONSerialization.jsonObject(with: fixture("active_runs")) as? [String: Any])
        var run = try #require((snapshot["sessions"] as? [[String: Any]])?.first)
        run["work"] = ["kind": "task", "id": "ts_now00000000000000000000000000000"]
        run["title"] = "This Task's Session"
        var other = run
        other["id"] = "other-session"
        other["work"] = ["kind": "task", "id": "ts_review00000000000000000000000000"]
        other["title"] = "Other Task's Session"
        snapshot["sessions"] = empty ? [] : [run, other]
        snapshot["gaps"] = gaps
        snapshot["discovery"] = discovery
        snapshot["task"] = NSNull()
        return String(decoding: try JSONSerialization.data(withJSONObject: snapshot), as: UTF8.self)
    }

    private func query(feed: ActiveSessionsTestFeed, sessions: String = "[]") throws -> RegistryQuery {
        let text = String(decoding: try placingTaskWorktrees(
            in: fixture("roadmap_snapshot"), at: "/src/loopflow"), as: UTF8.self)
        let initial = try activeSessions()
        return RegistryQuery(watchActiveSessions: { try await feed.open(initial: initial) }) { args, _ in
            switch args.first {
            case "roadmap": return text
            case "wave" where args.dropFirst().first == "list": return "[]"
            case "session" where args.dropFirst().first == "list": return sessions
            case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            default: throw RegistryQueryError("Unexpected action in Monitor proof")
            }
        }
    }
}

#endif
