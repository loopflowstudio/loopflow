#if os(macOS) && canImport(GhosttyKit)
import AppKit
import Foundation
import GhosttyKit
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    .deletingLastPathComponent().deletingLastPathComponent()

/// Session chrome (S3): the toolbar is the only chrome; one pane has none of
/// its own, two or more get a 24pt strip whose trio appears on hover of the
/// focused pane; split, close and zoom answer to keybinds. Every proof mounts
/// the production `SessionsView` over real Ghostty `/bin/cat` PTYs and checks
/// the terminal, its unfinished draft and its companion survive.
@Suite(.serialized)
@MainActor
struct SessionChromeProofTests {
    @Test("A single pane has no chrome; two panes get a strip whose trio appears only on hover of the focused pane",
          arguments: [CGSize(width: 1440, height: 900), CGSize(width: 1100, height: 800)])
    func paneStripTrioFollowsHoverAndFocus(size: CGSize) async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        try #require(GhosttyManager.shared.state == .ready)
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
        let panes = workspace.multiplexer.layout.allPanes.map(\.id)
        workspace.multiplexer.setFocusedPane(panes[0])

        let (roadmap, invocation) = try pinnedRoadmap()
        let task = WorkReference.task(id: "ts_review00000000000000000000000000")
        let record = try renameFixtureRecord("release", title: "Release outcomes", source: "human", work: task)
        var value = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(record)) as? [String: Any])
        value["state"] = "active"
        value["provider"] = "claude"
        value["actions"] = sessionActionFixture(kind: "conversation", state: "active")
        value["terminal_ids"] = [panes[0]]
        value["open_argv"] = ["must-not-launch"]
        value["flow_membership"] = ["kind": "step", "flow": "feature", "invocation_id": invocation,
                                    "step": "review-slice", "occurrence": "current", "node": "5", "iterations": [[2, 1]]]
        let sessions = String(decoding: try JSONSerialization.data(withJSONObject: [value]), as: UTF8.self)
        let query = RegistryQuery { args, _ in
            switch (args.first, args.dropFirst().first) {
            case ("roadmap", _): return roadmap
            case ("ls", _): return "[]"
            case ("activity", _): return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            case ("session", "list"): return sessions
            default: throw RegistryQueryError("Session chrome must not launch or mutate: \(args)")
            }
        }
        let model = PodiumModel(query: query, repoPath: repo)
        await model.refresh()
        model.select(.task(id: "issue-review"))
        model.navigation.selectedSessionId = "release"
        model.navigation.content = .terminals
        let view = SessionsView(model: model, repoPath: repo, workspaces: registry, query: query)
        let window = NSWindow(contentRect: CGRect(origin: .zero, size: size),
                              styleMask: [.titled], backing: .buffered, defer: false)
        let host = NSHostingView(rootView: view)
        window.contentView = host
        host.frame = window.contentLayoutRect
        defer { window.contentView = nil }
        try await settle(window)
        try #require(terminals.allSatisfy { $0.window === window })
        let draft = "chrome-draft"
        draft.withCString { ghostty_surface_text(surfaces[0], $0, UInt(draft.utf8.count)) }

        // The toolbar carries the location, the worktree, membership as text and Complete.
        #expect(throws: Never.self, "session crumb") {
            let name = try view.inspect().find(viewWithAccessibilityIdentifier: "breadcrumb-session").text().string()
            #expect(name == "Release outcomes")
        }
        #expect(throws: Never.self, "membership text") {
            let label = try view.inspect().find(viewWithAccessibilityIdentifier: "session-flow-membership").button()
                .labelView().text().string()
            #expect(label.contains("feature / review-slice"))
        }
        #expect(throws: Never.self, "worktree chip") {
            _ = try view.inspect().find(viewWithAccessibilityIdentifier: "worktree-chip")
        }
        #expect(throws: Never.self, "complete") {
            _ = try view.inspect().find(viewWithAccessibilityIdentifier: "session-action-complete")
        }

        // Two panes: a strip each, no trio at rest, none on the unfocused pane's hover.
        for pane in panes {
            #expect(throws: Never.self, "strip \(pane)") {
                _ = try view.inspect().find(viewWithAccessibilityIdentifier: "pane-strip-\(pane)")
            }
        }
        #expect(throws: (any Error).self) { try view.inspect().find(viewWithAccessibilityIdentifier: "pane-actions") }
        try hover(panes[1], inside: true, in: window)
        try await settle(window)
        #expect(throws: (any Error).self) { try view.inspect().find(viewWithAccessibilityIdentifier: "pane-actions") }
        try hover(panes[0], inside: true, in: window)
        try await settle(window)
        for id in ["pane-split-right", "pane-split-down", "pane-close"] {
            #expect(throws: Never.self, "\(id) after hover") {
                _ = try view.inspect().find(viewWithAccessibilityIdentifier: id).button()
            }
        }
        try snapshot(window, "two-panes-\(Int(size.width))x\(Int(size.height))")

        // Closing the companion from its own strip leaves one pane and no chrome.
        try hover(panes[0], inside: false, in: window)
        workspace.multiplexer.setFocusedPane(panes[1])
        try await settle(window)
        try hover(panes[1], inside: true, in: window)
        try await settle(window)
        try view.inspect().find(viewWithAccessibilityIdentifier: "pane-close").button().tap()
        try await settle(window)
        #expect(workspace.multiplexer.layout.allPanes.map(\.id) == [panes[0]])
        #expect(throws: (any Error).self) { try view.inspect().find(viewWithAccessibilityIdentifier: "pane-strip-\(panes[0])") }
        #expect(throws: (any Error).self) { try view.inspect().find(viewWithAccessibilityIdentifier: "pane-actions") }
        try snapshot(window, "one-pane-\(Int(size.width))x\(Int(size.height))")

        // The conversation's terminal, its draft and its reply are untouched.
        #expect(terminals[0].surface == surfaces[0])
        #expect(terminals[0].window === window)
        try await expectEcho(draft, on: surfaces[0])
    }

    @Test("⌘D, ⌘⇧D, ⌘⇧↩ and ⌘W act on the focused pane and keep its terminal")
    func keybindsSplitZoomAndClose() async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        try #require(GhosttyManager.shared.state == .ready)
        let repo = "/tmp"
        let registry = SessionsWorkspaceRegistry()
        let workspace = registry.workspace(for: repo)
        workspace.multiplexer.newShell()
        let pane = workspace.multiplexer.focusedPaneId
        let terminal = registry.surfaces.view(for: .shell(pane))
        terminal.frame = CGRect(x: 0, y: 0, width: 600, height: 400)
        terminal.workingDirectory = NSTemporaryDirectory()
        terminal.command = buildWorkspaceShellCommand(id: pane, argv: ["/bin/cat"], env: [:])
        terminal.createSurface(manager: GhosttyManager.shared)
        defer { registry.surfaces.release(.shell(pane)) }
        let surface = try #require(terminal.surface)
        let query = RegistryQuery { args, _ in
            switch args.first {
            case "roadmap": return #"{"generated_at":1,"waves":[]}"#
            case "ls": return "[]"
            case "session": return "[]"
            case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            default: throw RegistryQueryError("Keybinds must not launch or mutate: \(args)")
            }
        }
        let model = PodiumModel(query: query, repoPath: repo)
        await model.refreshSessions()
        model.navigation.content = .terminals
        let view = SessionsView(model: model, repoPath: repo, workspaces: registry, query: query)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1100, height: 700),
                              styleMask: [.titled], backing: .buffered, defer: false)
        let host = NSHostingView(rootView: view)
        window.contentView = host
        host.frame = window.contentLayoutRect
        defer { window.contentView = nil }
        try await settle(window)
        try #require(terminal.window === window)
        let draft = "keybind-draft"
        draft.withCString { ghostty_surface_text(surface, $0, UInt(draft.utf8.count)) }

        try press("d", keyCode: 2, modifiers: [.command], in: window)
        try await settle(window)
        #expect(workspace.multiplexer.layout.allPanes.count == 2)
        let split = workspace.multiplexer.focusedPaneId
        #expect(split != pane)
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "pane-strip-\(pane)")

        try press("\r", keyCode: 36, modifiers: [.command, .shift], in: window)
        try await settle(window)
        #expect(workspace.multiplexer.zoomedPaneId == split)
        try press("\r", keyCode: 36, modifiers: [.command, .shift], in: window)
        try await settle(window)
        #expect(workspace.multiplexer.zoomedPaneId == nil)

        try press("w", keyCode: 13, modifiers: [.command], in: window)
        try await settle(window)
        #expect(workspace.multiplexer.layout.allPanes.map(\.id) == [pane])
        #expect(workspace.multiplexer.focusedPaneId == pane)
        #expect(!registry.surfaces.hasSurface(.shell(split)))

        // Shift-D reaches AppKit as "D"; the binding must still split down.
        try press("D", keyCode: 2, modifiers: [.command, .shift], in: window)
        try await settle(window)
        #expect(workspace.multiplexer.layout.allPanes.count == 2)
        if case .split(let axis, _, _, _) = workspace.multiplexer.layout { #expect(axis == .horizontal) }
        try press("w", keyCode: 13, modifiers: [.command], in: window)
        try await settle(window)
        #expect(workspace.multiplexer.layout.allPanes.map(\.id) == [pane])

        #expect(terminal.surface == surface)
        #expect(terminal.window === window)
        try await expectEcho(draft, on: surface)
    }

    // MARK: - helpers

    /// Route a key through the application queue so the workspace's local
    /// event monitor sees it exactly as it would a real keystroke.
    private func press(_ character: String, keyCode: UInt16, modifiers: NSEvent.ModifierFlags, in window: NSWindow) throws {
        let event = try #require(NSEvent.keyEvent(
            with: .keyDown, location: .zero, modifierFlags: modifiers,
            timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil,
            characters: character, charactersIgnoringModifiers: character, isARepeat: false, keyCode: keyCode
        ))
        // Local event monitors run inside `sendEvent`, before dispatch.
        NSApp.sendEvent(event)
    }

    /// A real mouse-entered/exited event to the strip's AppKit tracking view.
    private func hover(_ paneId: String, inside: Bool, in window: NSWindow) throws {
        let region = try #require(findView(identifier: "pane-hover-\(paneId)", in: window.contentView))
        let point = region.convert(NSPoint(x: region.bounds.midX, y: region.bounds.midY), to: nil)
        let event = try #require(NSEvent.enterExitEvent(
            with: inside ? .mouseEntered : .mouseExited, location: point, modifierFlags: [], timestamp: 0,
            windowNumber: window.windowNumber, context: nil, eventNumber: 0, trackingNumber: 0, userData: nil
        ))
        if inside { region.mouseEntered(with: event) } else { region.mouseExited(with: event) }
    }

    private func findView(identifier: String, in view: NSView?) -> NSView? {
        guard let view else { return nil }
        if view.identifier?.rawValue == identifier { return view }
        for child in view.subviews {
            if let found = findView(identifier: identifier, in: child) { return found }
        }
        return nil
    }

    private func pinnedRoadmap() throws -> (roadmap: String, invocation: String) {
        let fixtures = repoRoot.appendingPathComponent("tests/fixtures/dto")
        let flows = try #require(JSONSerialization.jsonObject(with: Data(contentsOf:
            fixtures.appendingPathComponent("task_flow.json"))) as? [[String: Any]])
        let pinned = try #require(flows[1]["record"] as? [String: Any])
        let invocation = try #require(pinned["invocation_id"] as? String)
        var plan = try #require(JSONSerialization.jsonObject(with: Data(contentsOf:
            fixtures.appendingPathComponent("roadmap_snapshot.json"))) as? [String: Any])
        var waves = try #require(plan["waves"] as? [[String: Any]])
        let waveIndex = try #require(waves.firstIndex { wave in
            let tasks = wave["tasks"] as? [String: Any]
            return (tasks?["items"] as? [[String: Any]])?.contains { ($0["task"] as? [String: Any])?["id"] as? String == "issue-review" } == true
        })
        var tasks = try #require(waves[waveIndex]["tasks"] as? [String: Any])
        var items = try #require(tasks["items"] as? [[String: Any]])
        let taskIndex = try #require(items.firstIndex { ($0["task"] as? [String: Any])?["id"] as? String == "issue-review" })
        items[taskIndex]["flow"] = flows[1]
        tasks["items"] = items; waves[waveIndex]["tasks"] = tasks; plan["waves"] = waves
        return (String(decoding: try JSONSerialization.data(withJSONObject: plan), as: UTF8.self), invocation)
    }

    private func snapshot(_ window: NSWindow, _ name: String) throws {
        guard let directory = ProcessInfo.processInfo.environment["LOOPFLOW_SESSION_CHROME_CAPTURE_DIR"] else { return }
        _ = try SnapshotService().snapshotWindow(window, to: URL(fileURLWithPath: directory).appendingPathComponent("\(name).png"))
    }

    private func settle(_ window: NSWindow) async throws {
        window.contentView?.layoutSubtreeIfNeeded()
        window.layoutIfNeeded()
        try await Task.sleep(for: .milliseconds(100))
    }

    /// Both the PTY's input echo and cat's reply must arrive: the child is alive.
    private func expectEcho(_ text: String, on surface: ghostty_surface_t) async throws {
        "\n".withCString { ghostty_surface_text(surface, $0, 1) }
        let deadline = ContinuousClock.now + .seconds(3)
        while terminalText(surface).components(separatedBy: text).count < 3, ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(terminalText(surface).components(separatedBy: text).count == 3)
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
#endif
