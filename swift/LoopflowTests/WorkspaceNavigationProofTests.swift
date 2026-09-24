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

@Suite("Unified workspace navigation proof")
@MainActor
struct WorkspaceNavigationProofTests {
    @Test("The work list retains its viewport through presentations, refresh and repository return")
    func navigatorRetainsScroll() async throws {
        _ = NSApplication.shared
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json"))
        var snapshot = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var waves = try #require(snapshot["waves"] as? [[String: Any]])
        var projects = try #require(waves[0]["projects"] as? [String: Any])
        var items = try #require(projects["items"] as? [[String: Any]])
        let tasks = try #require(items[0]["tasks"] as? [[String: Any]])
        let template = try #require(tasks.last)
        items[0]["tasks"] = try (0..<80).map { index in
            var task = template
            var planning = try #require(task["task"] as? [String: Any])
            planning["id"] = "scroll-task-\(index)"
            planning["name"] = "Work item \(index)"
            planning["rank"] = Double(index)
            planning["completed"] = false
            task["task"] = planning
            task["runtime"] = NSNull()
            return task
        }
        projects["items"] = items
        waves[0]["projects"] = projects
        snapshot["waves"] = waves
        let roadmap = String(decoding: try JSONSerialization.data(withJSONObject: snapshot), as: UTF8.self)
        let query = RegistryQuery { args, _ in
            switch args.first {
            case "roadmap": return roadmap
            case "ls", "session": return "[]"
            case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            default: throw RegistryQueryError("Unexpected operation in navigator proof")
            }
        }
        let model = PodiumModel(query: query, repoPath: "/src/loopflow")
        await model.refresh()
        let registry = SessionsWorkspaceRegistry()
        let view = SessionsView(model: model, repoPath: "/src/loopflow", workspaces: registry, query: query)
        let host = NSHostingView(rootView: view.id("/src/loopflow"))
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1100, height: 600),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = host
        host.frame = window.contentLayoutRect
        defer { window.contentView = nil }
        try await settle(window)
        let scroll = try #require(_scrollView(in: host))
        scroll.contentView.scroll(to: CGPoint(x: 0, y: 1200))
        scroll.reflectScrolledClipView(scroll.contentView)
        try await settle(window)
        let offset = scroll.contentView.bounds.minY
        try #require(offset > 1000)
        model.select(.task(id: "scroll-task-30"))
        try await settle(window)
        try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-toggle-list").button().tap()
        try await settle(window)
        #expect(abs(scroll.contentView.bounds.minY - offset) < 1)
        await model.refresh()
        try await settle(window)
        #expect(abs(scroll.contentView.bounds.minY - offset) < 1)
        try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-all-work").button().tap()
        try await settle(window)
        #expect(abs(scroll.contentView.bounds.minY - offset) < 1)
        model.setRepoPath("/src/context")
        host.rootView = SessionsView(model: model, repoPath: "/src/context", workspaces: registry, query: query)
            .id("/src/context")
        try await settle(window)
        let other = try #require(_scrollView(in: host))
        #expect(abs(other.contentView.bounds.minY) < 1)
        model.setRepoPath("/src/loopflow")
        host.rootView = view.id("/src/loopflow")
        try await settle(window)
        let restored = try #require(_scrollView(in: host))
        #expect(abs(restored.contentView.bounds.minY - offset) < 1)
    }

    private func _scrollView(in view: NSView) -> NSScrollView? {
        if let scroll = view as? NSScrollView { return scroll }
        return view.subviews.lazy.compactMap { _scrollView(in: $0) }.first
    }

    private func settle(_ window: NSWindow) async throws {
        window.contentView?.layoutSubtreeIfNeeded()
        window.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(100))
    }

#if canImport(GhosttyKit)
    @Test("A hidden retained terminal relinquishes focus and keeps its unfinished PTY input")
    func hiddenTerminalPreservesDraft() async throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        try #require(manager.state == .ready)
        let pool = GhosttySurfacePool()
        let identity = TerminalIdentity.session("navigation-draft-proof")
        let terminal = pool.view(for: identity)
        let window = NSWindow(
            contentRect: CGRect(x: 0, y: 0, width: 500, height: 300),
            styleMask: [.titled], backing: .buffered, defer: false
        )
        let view = GhosttyTerminalView(
            workingDirectory: NSTemporaryDirectory(),
            argv: ["/bin/cat"],
            terminal: identity, surfacePool: pool, isFocused: true
        )
        terminal.setFrameSize(window.contentLayoutRect.size)
        terminal.workingDirectory = NSTemporaryDirectory()
        terminal.command = buildGhosttyShellCommand(argv: view.argv, env: [:])
        terminal.createSurface(manager: manager)
        let surface = try #require(terminal.surface)
        let host = NSHostingView(rootView: view.disabled(false))
        host.frame = window.contentLayoutRect
        window.contentView = host
        defer { pool.release(identity) }
        host.layoutSubtreeIfNeeded()
        window.layoutIfNeeded()
        try #require(terminal.surface == surface)
        #expect(window.makeFirstResponder(terminal))
        let draft = "retained-draft"
        draft.withCString { ghostty_surface_text(surface, $0, UInt(draft.utf8.count)) }
        let search = NSTextField(frame: CGRect(x: 0, y: 0, width: 200, height: 24))
        host.addSubview(search)
        #expect(window.makeFirstResponder(search))
        let editor = try #require(search.currentEditor())
        window.setContentSize(CGSize(width: 520, height: 300))
        window.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(50))
        #expect(window.firstResponder === editor)
        search.removeFromSuperview()
        pool.focus(identity)
        #expect(window.firstResponder === terminal)
        host.rootView = view.disabled(true)
        window.layoutIfNeeded()
        let hiddenDeadline = ContinuousClock.now + .seconds(3)
        while window.firstResponder === terminal, ContinuousClock.now < hiddenDeadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(window.firstResponder !== terminal)
        #expect(terminal.surface == surface)
        host.rootView = view.disabled(false)
        window.layoutIfNeeded()
        let focusDeadline = ContinuousClock.now + .seconds(3)
        while window.firstResponder !== terminal, ContinuousClock.now < focusDeadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(window.firstResponder === terminal)
        #expect(pool.view(for: identity) === terminal)
        "\n".withCString { ghostty_surface_text(surface, $0, 1) }
        let outputDeadline = ContinuousClock.now + .seconds(3)
        while _terminalText(surface).components(separatedBy: draft).count < 3, ContinuousClock.now < outputDeadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(_terminalText(surface).components(separatedBy: draft).count == 3)

        // Task terminals have no selected-pane binding. Manual focus must
        // survive a resize while the view remains enabled.
        host.rootView = GhosttyTerminalView(
            workingDirectory: NSTemporaryDirectory(), argv: ["/bin/cat"],
            terminal: identity, surfacePool: pool, isFocused: false
        ).disabled(false)
        window.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(50))
        #expect(window.makeFirstResponder(terminal))
        window.setContentSize(CGSize(width: 540, height: 300))
        window.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(50))
        #expect(window.firstResponder === terminal)
    }

    @Test("The workspace retains navigation and reconciles completion in its originating repository",
          .serialized, arguments: [false, true])
    func workspaceRetainsNativeSplit(switchBeforeCompletion: Bool) async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        try #require(GhosttyManager.shared.state == .ready)
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let roadmap = try String(contentsOf: root.appendingPathComponent(
            "tests/fixtures/dto/roadmap_snapshot.json"
        ), encoding: .utf8)
        let records = """
        [{"id":"navigation-split","kind":"interactive",
          "work":{"kind":"task","id":"ts_review00000000000000000000000000"},
          "title":"Navigation proof","detail":"Local cat PTY","cwd":"/tmp",
          "state":"active","ready_summary":null,"open_argv":["/bin/cat"]}]
        """
        let otherRecords = """
        [{"id":"context-session","kind":"interactive","work":null,
          "title":"Other repository conversation","detail":"Existing external client","cwd":"/src/context",
          "state":"active","ready_summary":null,"open_argv":["lf","session","open","context-session"]}]
        """
        let (completionResponses, completionResponse) = AsyncStream<Void>.makeStream()
        defer { completionResponse.finish() }
        let query = RegistryQuery { args, cwd in
            switch args.first {
            case "roadmap": return roadmap
            case "ls": return "[]"
            case "session" where args.dropFirst().first == "list":
                return cwd == "/src/context" ? otherRecords : records
            case "session" where args == ["session", "complete", "navigation-split"]:
                for await _ in completionResponses { break }
                return "Session completed"
            case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            default: throw RegistryQueryError("Unexpected operation in navigation proof")
            }
        }
        let model = PodiumModel(query: query, repoPath: "/src/loopflow")
        await model.refresh()
        let registry = SessionsWorkspaceRegistry()
        let workspace = registry.workspace(for: "/src/loopflow")
        workspace.multiplexer.load(sessionId: "navigation-split")
        let sessionPane = workspace.multiplexer.focusedPaneId
        _ = workspace.multiplexer.split(sessionPane, axis: .vertical)
        workspace.multiplexer.newShell()
        let shellPane = workspace.multiplexer.focusedPaneId
        workspace.multiplexer.load(sessionId: "navigation-split")
        let layout = workspace.multiplexer.layout
        let identities: [TerminalIdentity] = [.session("navigation-split"), .shell(shellPane)]
        let terminals = identities.map { workspace.surfaces.view(for: $0) }
        defer { for identity in identities { workspace.surfaces.release(identity) } }
        let history = (0..<120).map { "history-\($0)" }.joined(separator: " ")
        for (index, terminal) in terminals.enumerated() {
            terminal.setFrameSize(CGSize(width: 400, height: 300))
            terminal.workingDirectory = NSTemporaryDirectory()
            // Emit history from the child, without PTY input echo interleaving
            // with cat's output. Both children then retain ordinary input.
            let argv = index == 0 ? ["/bin/cat"] : [
                "/bin/sh", "-c", "printf '%s\\n' \(history); exec /bin/cat",
            ]
            terminal.command = buildGhosttyShellCommand(argv: argv, env: [:])
            terminal.createSurface(manager: GhosttyManager.shared)
        }
        let sessionSurface = try #require(terminals[0].surface)
        let shellSurface = try #require(terminals[1].surface)
        model.select(.task(id: "issue-review"))
        model.navigation.content = .terminals
        let view = SessionsView(model: model, repoPath: "/src/loopflow", workspaces: registry, query: query)
        let host = NSHostingView(rootView: view.id("/src/loopflow"))
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1100, height: 700),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = host
        host.frame = window.contentLayoutRect
        defer { window.contentView = nil }
        try await settle(window)
        try #require(terminals.allSatisfy { $0.window === window })
        let historyDeadline = ContinuousClock.now + .seconds(3)
        while !_terminalText(shellSurface).contains("history-119"),
              ContinuousClock.now < historyDeadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        try #require(_terminalText(shellSurface).contains("history-119"))
        let scroll = "scroll_to_top"
        #expect(scroll.withCString { ghostty_surface_binding_action(shellSurface, $0, UInt(scroll.utf8.count)) })
        let scrollDeadline = ContinuousClock.now + .seconds(3)
        while !_terminalText(shellSurface, viewport: true).contains("history-0"),
              ContinuousClock.now < scrollDeadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        let historyTop = Array(_terminalText(shellSurface, viewport: true).components(separatedBy: "\n").prefix(5))
        try #require(historyTop.contains { $0.contains("history-0") })
        let draft = "split-draft"
        draft.withCString { ghostty_surface_text(sessionSurface, $0, UInt(draft.utf8.count)) }

        for identifier in ["workspace-toggle-list", "workspace-work-details", "workspace-all-work",
                           "workspace-return-terminals", "workspace-toggle-list"] {
            try view.inspect().find(viewWithAccessibilityIdentifier: identifier).button().tap()
            try await settle(window)
            #expect(terminals[0].surface == sessionSurface)
            #expect(terminals[1].surface == shellSurface)
            #expect(Array(_terminalText(shellSurface, viewport: true).components(separatedBy: "\n").prefix(5)) == historyTop)
            #expect(workspace.multiplexer.layout == layout)
            #expect(workspace.multiplexer.focusedPaneId == sessionPane)
            if model.navigation.content != .terminals {
                #expect(!terminals.contains { window.firstResponder === $0 })
            }
        }
        model.setRepoPath("/src/context")
        host.rootView = SessionsView(model: model, repoPath: "/src/context", workspaces: registry, query: query)
            .id("/src/context")
        try await settle(window)
        model.setRepoPath("/src/loopflow")
        host.rootView = SessionsView(model: model, repoPath: "/src/loopflow", workspaces: registry, query: query)
            .id("/src/loopflow")
        try await settle(window)
        #expect(terminals.allSatisfy { $0.window === window })
        #expect(terminals[0].surface == sessionSurface)
        #expect(terminals[1].surface == shellSurface)
        #expect(Array(_terminalText(shellSurface, viewport: true).components(separatedBy: "\n").prefix(5)) == historyTop)
        #expect(workspace.multiplexer.layout == layout)
        #expect(workspace.multiplexer.focusedPaneId == sessionPane)
        #expect(window.firstResponder === terminals[0])
        #expect(model.selection == .task(id: "issue-review"))
        "\n".withCString { ghostty_surface_text(sessionSurface, $0, 1) }
        let companion = "companion-alive"
        let companionInput = companion + "\n"
        companionInput.withCString {
            ghostty_surface_text(shellSurface, $0, UInt(companionInput.utf8.count))
        }
        let deadline = ContinuousClock.now + .seconds(3)
        // Require both PTY input echo and cat's reply from each retained child.
        while (_terminalText(sessionSurface).components(separatedBy: draft).count < 3
               || _terminalText(shellSurface).components(separatedBy: companion).count < 3),
              ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(_terminalText(sessionSurface).components(separatedBy: draft).count == 3)
        #expect(_terminalText(shellSurface).components(separatedBy: companion).count == 3)

        try view.inspect().find(viewWithAccessibilityIdentifier: "session-action-complete").button().tap()
        try await settle(window)
        #expect(terminals[0].surface == sessionSurface)
        let otherWorkspace = registry.workspace(for: "/src/context")
        if switchBeforeCompletion {
            model.setRepoPath("/src/context")
            await model.refreshSessions()
            model.select(.wave(id: "wave-2"))
            otherWorkspace.multiplexer.load(sessionId: "context-session")
            host.rootView = SessionsView(model: model, repoPath: "/src/context", workspaces: registry, query: query)
                .id("/src/context")
            try await settle(window)
            #expect(terminals.allSatisfy { $0.window == nil })
        }
        let otherLayout = otherWorkspace.multiplexer.layout
        let otherFocus = otherWorkspace.multiplexer.focusedPaneId
        completionResponse.yield(())
        completionResponse.finish()
        let completionDeadline = ContinuousClock.now + .seconds(3)
        while terminals[0].surface != nil, ContinuousClock.now < completionDeadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        try await settle(window)
        #expect(workspace.multiplexer.pane(forSessionId: "navigation-split") == nil)
        #expect(terminals[0].surface == nil)
        #expect(terminals[1].surface == shellSurface)
        #expect(workspace.multiplexer.layout.allPanes.map(\.id) == [shellPane])
        #expect(workspace.multiplexer.focusedPaneId == shellPane)
        // Completing a Session is not an undoable Close view operation.
        workspace.multiplexer.undoClose()
        try await settle(window)
        #expect(workspace.multiplexer.pane(forSessionId: "navigation-split") == nil)
        #expect(workspace.multiplexer.layout.allPanes.map(\.id) == [shellPane])
        #expect(workspace.multiplexer.focusedPaneId == shellPane)
        if switchBeforeCompletion {
            #expect(model.repoPath == "/src/context")
            #expect(model.selection == .wave(id: "wave-2"))
            #expect(model.sessions.value?.map(\.id) == ["context-session"])
            #expect(otherWorkspace.multiplexer.layout == otherLayout)
            #expect(otherWorkspace.multiplexer.focusedPaneId == otherFocus)
            model.setRepoPath("/src/loopflow")
            host.rootView = SessionsView(model: model, repoPath: "/src/loopflow", workspaces: registry, query: query)
                .id("/src/loopflow")
            try await settle(window)
        }
        #expect(model.sessions.value?.isEmpty == true)
        #expect(model.selection == .task(id: "issue-review"))
        #expect(model.task(id: "issue-review")?.task.task.completed == false)
        try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-work-details").button().tap()
        try await settle(window)
        #expect(throws: Never.self) {
            try view.inspect().find(text: "No open Sessions for this Work")
        }
        try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-return-terminals").button().tap()
        try await settle(window)
        #expect(window.firstResponder === terminals[1])
        try #require(terminals[1].surface == shellSurface)
        let afterCompletion = "companion-after-completion"
        let input = afterCompletion + "\n"
        input.withCString { ghostty_surface_text(shellSurface, $0, UInt(input.utf8.count)) }
        let replyDeadline = ContinuousClock.now + .seconds(3)
        while _terminalText(shellSurface).components(separatedBy: afterCompletion).count < 3,
              ContinuousClock.now < replyDeadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(_terminalText(shellSurface).components(separatedBy: afterCompletion).count == 3)
    }

    private func _terminalText(_ surface: ghostty_surface_t, viewport: Bool = false) -> String {
        var text = ghostty_text_s()
        let selection = ghostty_selection_s(
            top_left: ghostty_point_s(tag: viewport ? GHOSTTY_POINT_VIEWPORT : GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_TOP_LEFT, x: 0, y: 0),
            bottom_right: ghostty_point_s(tag: viewport ? GHOSTTY_POINT_VIEWPORT : GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_BOTTOM_RIGHT, x: 0, y: 0),
            rectangle: false
        )
        guard ghostty_surface_read_text(surface, selection, &text) else { return "" }
        defer { ghostty_surface_free_text(surface, &text) }
        guard let bytes = text.text else { return "" }
        return String(decoding: Data(bytes: bytes, count: Int(text.text_len)), as: UTF8.self)
    }
#endif

}
#endif
