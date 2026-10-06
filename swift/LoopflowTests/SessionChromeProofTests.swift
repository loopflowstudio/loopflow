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
@Suite(.requiresDisplay, .serialized)
@MainActor
struct SessionChromeProofTests {
    @Test("A single pane has no chrome; two panes get a strip whose trio appears only on hover of the focused pane",
          arguments: [CGSize(width: 1440, height: 900), CGSize(width: 1100, height: 800)])
    func paneStripTrioFollowsHoverAndFocus(size: CGSize) async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        try #require(GhosttyManager.shared.state == .ready)
        let repo = "/src/loopflow"
        let registry = SessionsWorkspaceRegistry(localHomeId: fixtureHomeId)
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
                                    "step": "realign", "occurrence": "current", "node": 5, "iterations": [[2, 1]]]
        let sessions = String(decoding: try JSONSerialization.data(withJSONObject: [value]), as: UTF8.self)
        let query = RegistryQuery { args, _ in
            switch (args.first, args.dropFirst().first) {
            case ("roadmap", _): return roadmap
            case ("wave", "list"): return "[]"
            case ("activity", _): return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            case ("session", "list"): return #"{"entries":\#(sessions),"next":null}"#
            default: throw RegistryQueryError("Session chrome must not launch or mutate: \(args)")
            }
        }
        let model = WorkModel(query: query, repoPath: repo)
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
            #expect(label.contains("feature / realign"))
        }
        #expect(throws: Never.self, "Session worktree location") {
            let location = try view.inspect().find(viewWithAccessibilityIdentifier: "worktree-chip")
                .menu().labelView().find(text: "loopflow")
            #expect(try location.string() == "loopflow")
        }
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "task-worktree-location")
        }
        #expect(throws: Never.self, "complete") {
            _ = try view.inspect().find(viewWithAccessibilityIdentifier: "session-action-complete")
        }

        model.navigation.selectedSessionId = nil
        try await settle(window)
        #expect(throws: Never.self, "Task-only worktree location") {
            let location = try view.inspect().find(viewWithAccessibilityIdentifier: "task-worktree-location")
                .text().string()
            #expect(location == "loopflow")
        }
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "worktree-chip")
        }
        model.navigation.selectedSessionId = "release"
        try await settle(window)

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
        let registry = SessionsWorkspaceRegistry(localHomeId: fixtureHomeId)
        let workspace = registry.workspace(for: fixtureWorkspace(repo))
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
            case "wave" where args.dropFirst().first == "list": return "[]"
            case "session": return #"{"entries":[],"next":null}"#
            case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            default: throw RegistryQueryError("Keybinds must not launch or mutate: \(args)")
            }
        }
        let model = WorkModel(query: query, repoPath: repo)
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

    @Test("Cmd-K isolates search from retained PTYs and opens Task details")
    func paletteRetainsTerminalInput() async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        try #require(GhosttyManager.shared.state == .ready)
        let repo = "/src/loopflow"
        let registry = SessionsWorkspaceRegistry()
        let workspace = registry.workspace(for: WorkspaceIdentity(homeId: "local", worktree: repo))
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
        let (roadmap, _) = try pinnedRoadmap()
        let record = try renameFixtureRecord("palette-session", title: "Retained conversation", source: "human",
                                             work: .task(id: "ts_review00000000000000000000000000"))
        let companion = try renameFixtureRecord("palette-companion", title: "Companion conversation", source: "human",
                                                work: .task(id: "ts_review00000000000000000000000000"))
        let records = try [record, companion].enumerated().map { index, record in
            var value = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(record)) as? [String: Any])
            value["state"] = "active"
            value["actions"] = sessionActionFixture(kind: "conversation", state: "active")
            if case .shell(let pane) = terminals[index].terminal { value["terminal_ids"] = [pane] }
            return value
        }
        let sessions = String(decoding: try JSONSerialization.data(withJSONObject: records), as: UTF8.self)
        let flows = try String(contentsOf: repoRoot.appendingPathComponent("tests/fixtures/dto/flow_catalog.json"), encoding: .utf8)
        let inventory = PaletteSessionInventory(sessions)
        let query = RegistryQuery { args, _ in
            if args.first == "roadmap" { return roadmap }
            if args.first == "session" { return try await inventory.read() }
            if args.first == "flow" { return flows }
            if args.first == "wave" { return "[]" }
            throw RegistryQueryError("No launch or mutation authorized by palette inspection")
        }
        let model = WorkModel(query: query, repoPath: repo)
        await model.refresh()
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
        let sheet = try #require(window.attachedSheet)
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

        try press("k", keyCode: 40, modifiers: [.command], in: window)
        try await settle(window)
        let taskSheet = try #require(window.attachedSheet)
        try await settle(taskSheet)
        let issue = try #require(model.task(id: "issue-review")?.task.task.identifier)
        for char in issue { try press(String(char), keyCode: 0, modifiers: [], in: taskSheet) }
        try press("\r", keyCode: 36, modifiers: [], in: taskSheet)
        try await Task.sleep(for: .milliseconds(400))
        #expect(model.selection == .task(id: "issue-review"))
        #expect(model.navigation.content == .details)
        #expect(model.navigation.selectedSessionId == nil)
        #expect(terminals[0].surface == surfaces[0])
        #expect(terminals[1].surface == surfaces[1])
        #expect(throws: Never.self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "loopflow-detail-task")
        }

        // Arrow navigation operates on the visible ranked list, including recents.
        let second = model.searchDestinations("")[1].id
        try press("k", keyCode: 40, modifiers: [.command], in: window)
        try await settle(window)
        let arrows = try #require(window.attachedSheet)
        try await settle(arrows)
        try press("\u{f701}", keyCode: 125, modifiers: [], in: arrows)
        try press("\r", keyCode: 36, modifiers: [], in: arrows)
        try await Task.sleep(for: .milliseconds(400))
        if case .wave(let id) = second { #expect(model.selection == .wave(id: id)) }
        else { Issue.record("The fixture's second palette destination should be its Wave") }

        // Repeated Session activation focuses the exact retained shell each time.
        for (visit, index) in [0, 1, 0].enumerated() {
            let selected = [record, companion][index]
            try press("k", keyCode: 40, modifiers: [.command], in: window)
            try await settle(window)
            let sessionSheet = try #require(window.attachedSheet)
            try await settle(sessionSheet)
            for char in selected.title { try press(String(char), keyCode: 0, modifiers: [], in: sessionSheet) }
            try press("\r", keyCode: 36, modifiers: [], in: sessionSheet)
            try await Task.sleep(for: .milliseconds(400))
            #expect(model.navigation.selectedSessionId == selected.id)
            #expect(window.firstResponder === terminals[index])
            #expect(terminals[index].surface == surfaces[index])
            let reply = "visit-\(visit)-reply"
            reply.withCString { ghostty_surface_text(surfaces[index], $0, UInt(reply.utf8.count)) }
            try await expectEcho(reply, on: surfaces[index])
        }

        // Removing a highlighted Session must give Return to a visible row.
        model.navigation.content = .terminals
        try await settle(window)
        window.makeFirstResponder(terminals[1])
        try press("k", keyCode: 40, modifiers: [.command], in: window)
        try await settle(window)
        let changing = try #require(window.attachedSheet)
        try await settle(changing)
        let initialRows = model.searchDestinations("")
        let sessionIndex = try #require(initialRows.firstIndex { $0.id == .session(record.id) })
        for _ in 0..<sessionIndex { try press("\u{f701}", keyCode: 125, modifiers: [], in: changing) }
        await inventory.replace("[]")
        await model.refreshSessions()
        try await settle(changing)
        let remaining = try #require(model.searchDestinations("").first?.id)
        try press("\r", keyCode: 36, modifiers: [], in: changing)
        try await Task.sleep(for: .milliseconds(400))
        #expect(model.navigation.palette == nil)
        #expect(model.navigation.selectedSessionId == nil)
        if case .wave(let id) = remaining { #expect(model.selection == .wave(id: id)) }
        else if case .task(let id) = remaining { #expect(model.selection == .task(id: id)) }
        else { Issue.record("Expected a planning destination") }

        // Empty search results submit nothing; failure retains last-good rows.
        await inventory.replace(sessions)
        await model.refreshSessions()
        model.navigation.content = .terminals
        try await settle(window)
        window.makeFirstResponder(terminals[1])
        try press("k", keyCode: 40, modifiers: [.command], in: window)
        try await settle(window)
        let empty = try #require(window.attachedSheet)
        try await settle(empty)
        for char in record.title { try press(String(char), keyCode: 0, modifiers: [], in: empty) }
        await inventory.fail()
        await model.refreshSessions()
        try await settle(empty)
        #expect(model.paletteIsStale)
        #expect(model.searchDestinations(record.title).map(\.id) == [.session(record.id)])
        try press("\r", keyCode: 36, modifiers: [], in: empty)
        try await Task.sleep(for: .milliseconds(400))
        #expect(model.navigation.selectedSessionId == record.id)
        #expect(window.firstResponder === terminals[0])
        #expect(terminals[0].surface == surfaces[0])

        window.makeFirstResponder(terminals[1])
        try press("k", keyCode: 40, modifiers: [.command], in: window)
        try await settle(window)
        let removed = try #require(window.attachedSheet)
        try await settle(removed)
        for char in record.title { try press(String(char), keyCode: 0, modifiers: [], in: removed) }
        await inventory.replace("[]")
        await model.refreshSessions()
        try await settle(removed)
        try press("\r", keyCode: 36, modifiers: [], in: removed)
        #expect(model.navigation.palette == .search)
        #expect(model.searchDestinations(record.title).isEmpty)
        try press("\u{1b}", keyCode: 53, modifiers: [], in: removed)
        try await Task.sleep(for: .milliseconds(400))
        #expect(window.firstResponder === terminals[1])
        for surface in surfaces { #expect(!terminalText(surface).contains(record.title)) }

        // Flow inspection replaces search in the same sheet and dismisses once.
        model.navigation.content = .terminals
        try await settle(window)
        window.makeFirstResponder(terminals[1])
        try press("k", keyCode: 40, modifiers: [.command], in: window)
        try await settle(window)
        let flowSheet = try #require(window.attachedSheet)
        try await settle(flowSheet)
        for char in "feature" { try press(String(char), keyCode: 0, modifiers: [], in: flowSheet) }
        try press("\r", keyCode: 36, modifiers: [], in: flowSheet)
        try await settle(flowSheet)
        #expect(model.navigation.palette == .flow("feature"))
        #expect(window.attachedSheet === flowSheet)
        try press("\u{1b}", keyCode: 53, modifiers: [], in: flowSheet)
        try await Task.sleep(for: .milliseconds(400))
        #expect(model.navigation.palette == nil)
        #expect(window.attachedSheet == nil)
        #expect(window.firstResponder === terminals[1])

        // The palette opens the breadcrumb's existing permanent-bind picker.
        var unassigned = records
        unassigned[0]["work"] = NSNull()
        await inventory.replace(String(decoding: try JSONSerialization.data(withJSONObject: unassigned), as: UTF8.self))
        await model.refreshSessions()
        model.navigation.selectedSessionId = record.id
        try await settle(window)
        window.makeFirstResponder(terminals[0])
        let bindDraft = "binding-retained-draft"
        terminals[0].insertText(bindDraft, replacementRange: NSRange(location: NSNotFound, length: 0))
        try press("k", keyCode: 40, modifiers: [.command], in: window)
        try await settle(window)
        let bindSheet = try #require(window.attachedSheet)
        try await settle(bindSheet)
        for char in "Bind" { try press(String(char), keyCode: 0, modifiers: [], in: bindSheet) }
        #expect(model.searchDestinations("Bind").map(\.id) == [.bind(record.id)])
        try press("\r", keyCode: 36, modifiers: [], in: bindSheet)
        try await Task.sleep(for: .milliseconds(400))
        #expect(model.navigation.palette == nil)
        #expect(window.attachedSheet == nil)
        #expect(model.navigation.binding?.sessionId == record.id)
        #expect(model.navigation.binding?.preview == nil)
        #expect(model.navigation.selectedSessionId == record.id)
        model.cancelSessionBinding()
        #expect(model.navigation.binding == nil)
        try await expectEcho(bindDraft, on: surfaces[0])
        for surface in surfaces { #expect(!terminalText(surface).contains("Bind")) }
        #expect(terminals[0].surface == surfaces[0])
        #expect(terminals[1].surface == surfaces[1])
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
        var plan = try #require(JSONSerialization.jsonObject(with: placingTaskWorktrees(in: Data(contentsOf:
            fixtures.appendingPathComponent("roadmap_snapshot.json")), at: "/src/loopflow")) as? [String: Any])
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
private actor PaletteSessionInventory {
    private var value: String
    private var failed = false
    init(_ value: String) { self.value = value }
    func replace(_ value: String) { self.value = value; failed = false }
    func fail() { failed = true }
    func read() throws -> String {
        if failed { throw RegistryQueryError("Session read unavailable") }
        return #"{"entries":\#(value),"next":null}"#
    }
}

#endif
