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

/// Each proof mounts
/// the production `SessionsView` over real Ghostty `/bin/cat` PTYs and checks
/// the terminal, its unfinished draft and its companion survive.
@Suite(.requiresDisplay, .serialized)
@MainActor
struct SessionChromeProofTests {
    @Test("⌘T and ⌘W add and close shell tabs without replacing the original terminal")
    func keybindsRetainShellTabs() async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        try #require(GhosttyManager.shared.state == .ready)
        let repo = "/tmp"
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let workspace = fixtureWork(registry, fixtureWorkspace(repo))
        workspace.multiplexer.newShell(location: workspace.location)
        let pane = workspace.multiplexer.focusedPane.content.selectedShell!.id
        let terminal = registry.surfaces.view(for: .shell(pane, machineId: fixtureMachineId))
        terminal.frame = CGRect(x: 0, y: 0, width: 600, height: 400)
        terminal.workingDirectory = NSTemporaryDirectory()
        terminal.command = buildWorkspaceShellCommand(id: pane, argv: ["/bin/cat"], env: [:])
        terminal.createSurface(manager: GhosttyManager.shared)
        defer { registry.surfaces.release(.shell(pane, machineId: fixtureMachineId)) }
        let surface = try #require(terminal.surface)
        let query = RegistryQuery { args, _ in
            switch args.first {
            case "desktop" where args.dropFirst().first == "plan": return #"{"generated_at":1,"waves":[]}"#
            case "wave" where args.dropFirst().first == "list": return "[]"
            case "desktop" where args.dropFirst().first == "sessions": return #"{"entries":[],"next":null}"#
            case "history": return #"{"truncated":false,"items":[]}"#
            default: throw RegistryQueryError("Keybinds must not launch or mutate: \(args)")
            }
        }
        let model = WorkModel(query: query, repoPath: repo)
        await model.refreshSessions()
        model.bindWorkspaces(registry, repository: workspace.key.repository)
        model.select(.task(id: workspace.key.id))
        model.navigation.content = .terminals
        let view = SessionsContentView(model: model, repoPath: repo, workspaces: registry,
            machineId: fixtureMachineId, repository: workspace.key.repository, query: query)
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

        let shellPane = workspace.multiplexer.focusedPaneId
        for count in [2, 3] {
            try press("t", keyCode: 17, modifiers: [.command], in: window)
            try await settle(window)
            #expect(workspace.multiplexer.layout.allPanes.count == 1)
            guard case .shell(let tabs, _) = workspace.multiplexer.focusedPane.content else {
                Issue.record("Missing Shell tool"); return
            }
            #expect(tabs.count == count)
            #expect(workspace.multiplexer.focusedPaneId == shellPane)
        }
        for _ in 0..<2 {
            try press("w", keyCode: 13, modifiers: [.command], in: window)
            try await settle(window)
        }
        #expect(workspace.multiplexer.focusedPane.content.selectedShell?.id == pane)
        #expect(workspace.multiplexer.focusedPaneId == shellPane)

        #expect(terminal.surface == surface)
        #expect(terminal.window === window)
        try await expectEcho(draft, on: surface)
    }

    @Test("Cmd-K searches over retained PTYs and restores terminal input")
    func paletteRetainsTerminalInput() async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        try #require(GhosttyManager.shared.state == .ready)
        let repo = "/src/loopflow"
        let registry = SessionsWorkspaceRegistry()
        let workspace = fixtureWork(registry, WorkspaceIdentity(machineId: "local", worktree: repo))
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
        let (roadmap, _) = try latestRoadmap()
        let record = try renameFixtureRecord("palette-session", title: "Retained conversation", source: "human",
                                             work: .task(id: "ts_review00000000000000000000000000"))
        let companion = try renameFixtureRecord("palette-companion", title: "Companion conversation", source: "human",
                                                work: .task(id: "ts_review00000000000000000000000000"))
        let records = try [record, companion].enumerated().map { index, record in
            var value = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(record)) as? [String: Any])
            value["state"] = "active"
            value["actions"] = sessionActionFixture(state: "active")
            if case .shell(let pane, _) = terminals[index].terminal { value["terminal_ids"] = [pane] }
            return value
        }
        let sessions = String(decoding: try JSONSerialization.data(withJSONObject: records), as: UTF8.self)
        let graphs = try String(contentsOf: repoRoot.appendingPathComponent("tests/fixtures/dto/graph_catalog.json"), encoding: .utf8)
        let inventory = PaletteSessionInventory(sessions)
        let query = RegistryQuery { args, _ in
            if args.prefix(2) == ["desktop", "plan"] { return roadmap }
            if args.prefix(2) == ["desktop", "sessions"] { return await inventory.read() }
            if args.first == "graph" { return graphs }
            if args.first == "wave" { return "[]" }
            throw RegistryQueryError("No launch or mutation authorized by palette inspection")
        }
        let model = WorkModel(query: query, repoPath: repo)
        await model.refresh()
        model.navigation.showsNavigator = false
        model.select(.task(id: "issue-review"))
        model.navigation.content = .terminals
        let view = SessionsView(model: model, repoPath: repo, workspaces: registry, query: query)
        let window = NSWindow(contentRect: .init(x: 0, y: 0, width: 1100, height: 800),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: view)
        window.makeKeyAndOrderFront(nil)
        defer { window.orderOut(nil); window.contentView = nil }
        try await settle(window)
        window.makeFirstResponder(terminals[1])
        let draft = "palette-retained-draft"
        draft.withCString { ghostty_surface_text(surfaces[1], $0, UInt(draft.utf8.count)) }
        try press("k", keyCode: 40, modifiers: [.command], in: window)
        try await settle(window)
        #expect(model.navigation.palette == .search)
        let sheet = try #require(NSApp.keyWindow)
        #expect(sheet.sheetParent === window)
        try await settle(sheet)
        for char in "never-pty" { try press(String(char), keyCode: 0, modifiers: [], in: sheet) }
        try await settle(sheet)
        #expect(terminalText(surfaces[0]).contains("never-pty") == false)
        #expect(terminalText(surfaces[1]).contains("never-pty") == false)
        try press("\u{1b}", keyCode: 53, modifiers: [], in: sheet)
        try await Task.sleep(for: .milliseconds(400))
        #expect(model.navigation.palette == nil)
        #expect(window.firstResponder === terminals[1])
        try await expectEcho(draft, on: surfaces[1])

        #expect(terminals[0].surface == surfaces[0])
        #expect(terminals[1].surface == surfaces[1])
        try await expectEcho(draft, on: surfaces[1])
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

    private func latestRoadmap() throws -> (roadmap: String, invocation: String) {
        let fixtures = repoRoot.appendingPathComponent("tests/fixtures/dto")
        let graphs = try #require(JSONSerialization.jsonObject(with: Data(contentsOf:
            fixtures.appendingPathComponent("graph_process_progress.json"))) as? [[String: Any]])
        let latest = graphs[0]
        let entry = try #require(latest["entry"] as? [String: Any])
        let invocation = try #require(entry["id"] as? String)
        var plan = try #require(JSONSerialization.jsonObject(with: placingTaskWorktrees(in: Data(contentsOf:
            fixtures.appendingPathComponent("roadmap_snapshot.json")), at: "/src/loopflow")) as? [String: Any])
        var tasks = try #require(plan["tasks"] as? [String: Any])
        var items = try #require(tasks["items"] as? [[String: Any]])
        let taskIndex = try #require(items.firstIndex { ($0["task"] as? [String: Any])?["id"] as? String == "issue-review" })
        items[taskIndex]["latest_graph_process"] = latest
        items[taskIndex]["execution"] = ["state": "running", "reason": "Running", "step": NSNull(), "captured": NSNull()]
        tasks["items"] = items; plan["tasks"] = tasks
        return (String(decoding: try JSONSerialization.data(withJSONObject: plan), as: UTF8.self), invocation)
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
private actor PaletteSessionInventory {
    private var value: String
    init(_ value: String) { self.value = value }
    func replace(_ value: String) { self.value = value }
    func read() -> String {
        return #"{"entries":\#(value),"next":null}"#
    }
}

#endif
