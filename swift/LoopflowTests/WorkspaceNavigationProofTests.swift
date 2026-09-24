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

    @Test("The mounted workspace retains a Session and companion split through Work navigation")
    func workspaceRetainsNativeSplit() async throws {
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
        let query = RegistryQuery { args, _ in
            switch args.first {
            case "roadmap": return roadmap
            case "ls": return "[]"
            case "session" where args.dropFirst().first == "list": return records
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
        "companion-alive\n".withCString { ghostty_surface_text(shellSurface, $0, 16) }
        let deadline = ContinuousClock.now + .seconds(3)
        while _terminalText(sessionSurface).components(separatedBy: draft).count < 3,
              ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(_terminalText(sessionSurface).components(separatedBy: draft).count == 3)
        #expect(_terminalText(shellSurface).contains("companion-alive"))
    }

    private func settle(_ window: NSWindow) async throws {
        window.contentView?.layoutSubtreeIfNeeded()
        window.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(100))
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
