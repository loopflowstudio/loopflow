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
        var evidence = try #require(waves[0]["tasks"] as? [String: Any])
        let tasks = try #require(evidence["items"] as? [[String: Any]])
        let template = try #require(tasks.last)
        evidence["items"] = try (0..<80).map { index in
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
        waves[0]["tasks"] = evidence
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
        model.navigation.content = .terminals
        try await settle(window)
        #expect(abs(scroll.contentView.bounds.minY - offset) < 1)
        await model.refresh()
        try await settle(window)
        #expect(abs(scroll.contentView.bounds.minY - offset) < 1)
        model.navigation.content = .overview
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
    @Test("Renaming a Session and returning through its ancestors keeps its terminal, draft and companion")
    func namedSessionDrillDownRetainsTerminal() async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        let repo = "/src/loopflow"
        let registry = SessionsWorkspaceRegistry()
        let workspace = registry.workspace(for: repo)
        var terminals: [GhosttyMetalView] = []
        for _ in 0..<3 {
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
        let task = WorkReference.task(id: "ts_review00000000000000000000000000")
        let named = try NamedSessionSource(records: JSONSerialization.data(withJSONObject: ["first", "second"].enumerated().map { index, id in
            let record = try renameFixtureRecord(id, title: index == 0 ? "review-design" : "lyric-cadenza", work: task)
            var value = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(record)) as? [String: Any])
            value["state"] = "active"
            value["actions"] = sessionActionFixture(kind: "interactive", state: "active")
            value["terminal_ids"] = [shells[index]]
            value["open_argv"] = ["must-not-launch"]
            return value
        }))
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let roadmap = try String(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json"), encoding: .utf8)
        let query = RegistryQuery { args, _ in
            switch (args.first, args.dropFirst().first) {
            case ("roadmap", _): return roadmap
            case ("ls", _): return "[]"
            case ("activity", _): return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            case ("session", "list"): return try await named.list()
            case ("session", "rename"): return try await named.rename(args)
            default: throw RegistryQueryError("A named local Session must not relaunch: \(args)")
            }
        }
        let model = PodiumModel(query: query, repoPath: repo)
        await model.refresh()
        model.navigation.content = .terminals
        let view = SessionsView(model: model, repoPath: repo, workspaces: registry, query: query)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1400, height: 700),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: view)
        defer { window.contentView = nil }
        try await settle(window)
        let draft = "named-session-draft"
        draft.withCString { ghostty_surface_text(surfaces[0], $0, UInt(draft.utf8.count)) }

        try view.inspect().find(viewWithAccessibilityIdentifier: "session-row-first").button().tap()
        try await settle(window)
        #expect(model.navigation.selectedSessionId == "first")
        #expect(window.firstResponder === terminals[0])
        #expect(try view.inspect().find(viewWithAccessibilityIdentifier: "breadcrumb-task").button().labelView().text().string()
            == "Show the all-wave roadmap and launch or attach")
        #expect(try view.inspect().find(viewWithAccessibilityIdentifier: "session-flow-membership").text().string() == "Independent")

        try view.inspect().find(viewWithAccessibilityIdentifier: "session-rename").button().tap()
        try await settle(window)
        #expect(model.navigation.renaming?.sessionId == "first")
        model.navigation.renaming?.text = "Launch design"
        try view.inspect().find(viewWithAccessibilityIdentifier: "session-rename-save").button().tap()
        for _ in 0..<10 where model.navigation.renaming != nil { try await settle(window) }
        #expect(model.navigation.renaming == nil)
        #expect(model.sessions.value?.first { $0.id == "first" }?.title == "Launch design")
        #expect(await named.renames == 1)
        #expect(workspace.multiplexer.layout == layout)
        #expect(terminals[0].surface == surfaces[0])

        // Ancestor return shows the Task; the Session row drills back into the
        // same terminal rather than opening another client.
        try view.inspect().find(viewWithAccessibilityIdentifier: "breadcrumb-task").button().tap()
        try await settle(window)
        #expect(model.navigation.content == .details)
        #expect(model.navigation.selectedSessionId == nil)
        #expect(window.firstResponder !== terminals[0])
        try view.inspect().find(viewWithAccessibilityIdentifier: "session-row-first").button().tap()
        try await settle(window)
        #expect(window.firstResponder === terminals[0])

        try view.inspect().find(viewWithAccessibilityIdentifier: "breadcrumb-session-second").button().tap()
        try await settle(window)
        #expect(model.navigation.selectedSessionId == "second")
        #expect(window.firstResponder === terminals[1])
        try view.inspect().find(viewWithAccessibilityIdentifier: "breadcrumb-session-first").button().tap()
        try await settle(window)
        #expect(window.firstResponder === terminals[0])
        #expect(workspace.multiplexer.layout == layout)

        for (index, surface) in surfaces.enumerated() {
            #expect(terminals[index].surface == surface)
            let reply = index == 0 ? draft : "companion-\(index)-responds"
            let input = index == 0 ? "\n" : reply + "\n"
            input.withCString { ghostty_surface_text(surface, $0, UInt(input.utf8.count)) }
            let deadline = ContinuousClock.now + .seconds(3)
            while _terminalText(surface).components(separatedBy: reply).count < 3, ContinuousClock.now < deadline {
                try await Task.sleep(for: .milliseconds(20))
            }
            #expect(_terminalText(surface).components(separatedBy: reply).count == 3)
        }
    }

    @Test("Session rows restore their shell and companion layout across worktree slots")
    func sessionRowRestoresWorktree() async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        let repo = "/src/loopflow"
        let paths = [repo, "/src/loopflow.task"]
        let registry = SessionsWorkspaceRegistry()
        let outer = registry.layout(for: repo)
        var terminals: [GhosttyMetalView] = []
        var records: [[String: Any]] = []
        for (index, path) in paths.enumerated() {
            let workspace = registry.workspace(for: path)
            for _ in 0..<2 {
                workspace.multiplexer.newShell()
                let pane = workspace.multiplexer.focusedPaneId
                let terminal = registry.surfaces.view(for: .shell(pane))
                terminal.frame = CGRect(x: 0, y: 0, width: 500, height: 350)
                terminal.workingDirectory = NSTemporaryDirectory()
                terminal.command = buildWorkspaceShellCommand(id: pane, argv: ["/bin/cat"], env: [:])
                terminal.createSurface(manager: GhosttyManager.shared)
                terminals.append(terminal)
            }
            let shell = workspace.multiplexer.layout.firstPane.id
            workspace.multiplexer.setFocusedPane(shell)
            records.append([
                "id": "row-\(index)", "run_id": "row-\(index)", "kind": "interactive", "work": NSNull(),
                "title": "Conversation \(index)", "detail": "Local shell", "cwd": path,
                "state": "active", "ready_summary": NSNull(), "work_path": NSNull(), "actions": sessionActionFixture(kind: "interactive", state: "active"), "title_source": "generated", "flow_membership": ["kind": "independent"], "terminal_ids": [shell],
                "open_argv": ["must-not-launch"],
            ])
        }
        defer { for terminal in terminals { registry.surfaces.release(terminal.terminal) } }
        let surfaces = try terminals.map { try #require($0.surface) }
        let layouts = paths.map { registry.workspace(for: $0).multiplexer.layout }
        let sessionJSON = String(decoding: try JSONSerialization.data(withJSONObject: records), as: UTF8.self)
        let query = RegistryQuery { args, _ in
            switch args.first {
            case "roadmap": return #"{"generated_at":1,"waves":[]}"#
            case "ls": return "[]"
            case "session" where args.dropFirst().first == "list": return sessionJSON
            default: throw RegistryQueryError("Local row must focus its existing shell")
            }
        }
        let model = PodiumModel(query: query, repoPath: repo)
        await model.refreshSessions()
        model.navigation.content = .terminals
        let view = SessionsView(model: model, repoPath: repo, workspaces: registry, query: query)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1400, height: 700),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: view)
        defer { window.contentView = nil }
        try await settle(window)
        let draft = "retained-row-draft"
        draft.withCString { ghostty_surface_text(surfaces[0], $0, UInt(draft.utf8.count)) }

        for index in [1, 0] {
            try view.inspect().find(viewWithAccessibilityIdentifier: "session-row-row-\(index)").button().tap()
            try await settle(window)
            #expect(outer.focusedPath == paths[index])
            #expect(window.firstResponder === terminals[index * 2])
            #expect(terminals[index * 2].window === window)
            #expect(terminals[(1 - index) * 2].window == nil)
            #expect(paths.map { registry.workspace(for: $0).multiplexer.layout } == layouts)
        }

        let firstSlot = outer.focusedSlotId
        try view.inspect().find(viewWithAccessibilityLabel: "Split worktrees right").button().tap()
        outer.select(paths[1])
        try await settle(window)
        #expect(terminals.allSatisfy { $0.window === window })
        #expect(paths.map { registry.workspace(for: $0).multiplexer.layout } == layouts)
        try view.inspect().find(viewWithAccessibilityIdentifier: "session-row-row-0").button().tap()
        try await settle(window)
        #expect(outer.layout.slots.count == 2)
        #expect(outer.focusedSlotId == firstSlot)
        #expect(window.firstResponder === terminals[0])
        outer.close(firstSlot)
        try await settle(window)
        try view.inspect().find(viewWithAccessibilityIdentifier: "session-row-row-0").button().tap()
        try await settle(window)
        #expect(window.firstResponder === terminals[0])
        #expect(paths.map { registry.workspace(for: $0).multiplexer.layout } == layouts)

        let store = registry.workspace(for: repo).sessionStore(repoPath: repo, query: query)
        #expect(store.sessions.allSatisfy { $0.state == .live })
        for (index, surface) in surfaces.enumerated() {
            #expect(terminals[index].surface == surface)
            let reply = index == 0 ? draft : "companion-\(index)-responds"
            let input = index == 0 ? "\n" : reply + "\n"
            input.withCString { ghostty_surface_text(surface, $0, UInt(input.utf8.count)) }
            let deadline = ContinuousClock.now + .seconds(3)
            while _terminalText(surface).components(separatedBy: reply).count < 3, ContinuousClock.now < deadline {
                try await Task.sleep(for: .milliseconds(20))
            }
            #expect(_terminalText(surface).components(separatedBy: reply).count == 3)
        }
    }

    @Test("Shell-attached Sessions can complete without closing their terminal", .serialized,
          arguments: [false, true])
    func shellSessionCompletion(rejected: Bool) async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        let registry = SessionsWorkspaceRegistry()
        let workspace = registry.workspace(for: "/tmp")
        workspace.multiplexer.newShell()
        let pane = workspace.multiplexer.focusedPaneId
        let terminal = registry.surfaces.view(for: .shell(pane))
        terminal.frame = CGRect(x: 0, y: 0, width: 800, height: 500)
        terminal.workingDirectory = "/tmp"
        terminal.command = buildWorkspaceShellCommand(id: pane, argv: ["/bin/cat"], env: [:])
        terminal.createSurface(manager: GhosttyManager.shared)
        defer { registry.surfaces.release(.shell(pane)) }
        let surface = try #require(terminal.surface)
        let records = try String(decoding: JSONSerialization.data(withJSONObject: ["shell-conversation", "second-conversation"].map { id in
            ["id": id, "run_id": id, "kind": "interactive", "work": NSNull(),
             "title": id, "detail": "Local PTY", "cwd": "/tmp",
             "state": "active", "ready_summary": NSNull(), "work_path": NSNull(), "actions": sessionActionFixture(kind: "interactive", state: "active"), "title_source": "generated", "flow_membership": ["kind": "independent"], "terminal_ids": [pane],
             "open_argv": ["unused"]] as [String: Any]
        }), as: UTF8.self)
        let query = RegistryQuery { args, _ in
            switch args.first {
            case "roadmap": return #"{"generated_at":1,"waves":[]}"#
            case "ls": return "[]"
            case "session" where args.dropFirst().first == "list": return records
            case "session" where args.dropFirst().first == "complete":
                if rejected { throw RegistryQueryError("Completion rejected") }
                return "Session completed"
            case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            default: throw RegistryQueryError("Unexpected operation in shell completion proof")
            }
        }
        let model = PodiumModel(query: query, repoPath: "/tmp")
        await model.refreshSessions()
        model.navigation.content = .terminals
        let view = SessionsView(model: model, repoPath: "/tmp", workspaces: registry, query: query)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1100, height: 700),
                              styleMask: [.titled], backing: .buffered, defer: false)
        let host = NSHostingView(rootView: view)
        window.contentView = host
        host.frame = window.contentLayoutRect
        defer { window.contentView = nil }
        try await settle(window)
        try view.inspect().find(viewWithAccessibilityIdentifier: "session-action-complete").button().tap()
        try await settle(window)
        if rejected {
            // An ordinary inventory refresh must not erase a rejected action.
            let store = workspace.sessionStore(repoPath: "/tmp", query: query)
            store.reconcile(try await query.sessions(cwd: "/tmp"))
            #expect(store.sessions.first?.state == .live)
            #expect(store.sessions.first?.resolutionError == "Completion rejected")
            try await settle(window)
            #expect(model.sessions.value?.map(\.id) == ["shell-conversation", "second-conversation"])
            #expect(throws: Never.self) { try view.inspect().find(text: "Completion rejected") }
            #expect(try !view.inspect().find(viewWithAccessibilityIdentifier: "session-action-complete").button().isDisabled())
        } else {
            #expect(model.sessions.value?.map(\.id) == ["second-conversation"])
            #expect(try !view.inspect().find(viewWithAccessibilityIdentifier: "session-action-complete").button().isDisabled())
            try view.inspect().find(viewWithAccessibilityIdentifier: "session-action-complete").button().tap()
            try await settle(window)
            #expect(model.sessions.value?.isEmpty == true)
            // A later conversation in the same retained shell must still be completable.
            await model.refreshSessions()
            try await settle(window)
            #expect(try !view.inspect().find(viewWithAccessibilityIdentifier: "session-action-complete").button().isDisabled())
        }
        #expect(terminal.surface == surface)
        #expect(workspace.multiplexer.layout.pane(for: pane)?.content == .shell)
        let reply = "shell-after-completion"
        let input = reply + "\n"
        input.withCString { ghostty_surface_text(surface, $0, UInt(input.utf8.count)) }
        let deadline = ContinuousClock.now + .seconds(3)
        while _terminalText(surface).components(separatedBy: reply).count < 3, ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(_terminalText(surface).components(separatedBy: reply).count == 3)
    }

    @Test("Rejected Flow decisions retain their terminal and remain visible after polling",
          .serialized, arguments: [true, false])
    func rejectedFlowDecisionRetainsTerminal(approving: Bool) async throws {
        _ = NSApplication.shared
        GhosttyManager.shared.initialize()
        let registry = SessionsWorkspaceRegistry()
        let workspace = registry.workspace(for: "/tmp")
        workspace.multiplexer.load(sessionId: "review-decision")
        let terminal = registry.surfaces.view(for: .session("review-decision"))
        terminal.frame = CGRect(x: 0, y: 0, width: 800, height: 500)
        terminal.workingDirectory = "/tmp"
        terminal.command = buildGhosttyShellCommand(argv: ["/bin/cat"], env: [:])
        terminal.createSurface(manager: GhosttyManager.shared)
        defer { registry.surfaces.release(.session("review-decision")) }
        let surface = try #require(terminal.surface)
        let records = """
        [{"id":"review-decision", "run_id": "review-decision","kind":"flow","work":null,"work_path":null,
          "title":"Review decision","detail":"Human review","cwd":"/tmp",
          "state":"ready","ready_summary":"Ready for review","title_source":"generated","flow_membership":{"kind":"independent"},"terminal_ids":[],
          "actions":\(sessionActionFixtureJSON(kind: "flow", state: "ready")),"open_argv":["/bin/cat"]}]
        """
        let query = RegistryQuery { args, _ in
            switch args.first {
            case "session" where args.dropFirst().first == "list": return records
            case "session": throw RegistryQueryError("Decision rejected")
            default: throw RegistryQueryError("Unexpected read in decision proof")
            }
        }
        let model = PodiumModel(query: query, repoPath: "/tmp")
        await model.refreshSessions()
        model.navigation.content = .terminals
        let view = SessionsView(model: model, repoPath: "/tmp", workspaces: registry, query: query)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1100, height: 700),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: view)
        defer { window.contentView = nil }
        try await settle(window)
        let store = workspace.sessionStore(repoPath: "/tmp", query: query)
        for _ in 0..<2 {
            let accepted = await store.decideFlow("review-decision", approving: approving, text: "Reviewed")
            #expect(!accepted)
            #expect(store.sessions.first?.state == .live)
            store.reconcile(try await query.sessions(cwd: "/tmp"))
            try await settle(window)
            #expect(store.sessions.first?.state == .live)
            #expect(throws: Never.self) { try view.inspect().find(text: "Decision rejected") }
            for id in ["session-action-approve", "session-action-iterate"] {
                #expect(try !view.inspect().find(viewWithAccessibilityIdentifier: id).button().isDisabled())
            }
            #expect(model.sessions.value?.map(\.id) == ["review-decision"])
            #expect(terminal.surface == surface)
        }
        let input = "review-still-responds\n"
        input.withCString { ghostty_surface_text(surface, $0, UInt(input.utf8.count)) }
        let deadline = ContinuousClock.now + .seconds(3)
        while _terminalText(surface).components(separatedBy: "review-still-responds").count < 3,
              ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(_terminalText(surface).components(separatedBy: "review-still-responds").count == 3)
    }

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
        [{"id":"navigation-split", "run_id": "navigation-split","kind":"interactive",
          "work":{"kind":"task","id":"ts_review00000000000000000000000000"},
          "title":"Navigation proof","detail":"Local cat PTY","cwd":"/tmp",
          "state":"active","ready_summary":null,"work_path":null,"actions":\(sessionActionFixtureJSON(kind: "interactive", state: "active")),"title_source":"generated","flow_membership":{"kind":"independent"},"terminal_ids":[],"open_argv":["/bin/cat"]}]
        """
        let otherRecords = """
        [{"id":"context-session", "run_id": "context-session","kind":"interactive","work":null,
          "title":"Other repository conversation","detail":"Existing external client","cwd":"/src/context",
          "state":"active","ready_summary":null,"work_path":null,"actions":\(sessionActionFixtureJSON(kind: "interactive", state: "active")),"title_source":"generated","flow_membership":{"kind":"independent"},"terminal_ids":[],"open_argv":["lf","session","open","context-session"]}]
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
                "/bin/sh", "-c", "printf '\\033]2;Retained companion\\007'; printf '%s\\n' \(history); exec /bin/cat",
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
        #expect(throws: Never.self) { try view.inspect().find(text: "Retained companion") }
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

        for presentation in WorkspacePresentation.allCases {
            model.navigation.presentation = presentation
            model.navigation.content = presentation == .full ? .details : .terminals
            try await settle(window)
            if let directory = ProcessInfo.processInfo.environment["LOOPFLOW_OUTLINE_CAPTURE_DIR"] {
                let path = URL(fileURLWithPath: directory)
                    .appendingPathComponent("\(presentation.rawValue)-\(switchBeforeCompletion).png")
                _ = try SnapshotService().snapshotWindow(window, to: path)
            }
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
        #expect(throws: Never.self) { try view.inspect().find(text: "Retained companion") }
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
        model.select(.task(id: "issue-review"))
        try await settle(window)
        #expect(model.workspace.subject(for: "human") == nil)
        model.navigation.content = .terminals
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

/// The shared Session authority for the named drill-down proof.
private actor NamedSessionSource {
    private var records: [[String: Any]]
    private(set) var renames = 0

    init(records: Data) throws {
        self.records = try JSONSerialization.jsonObject(with: records) as? [[String: Any]] ?? []
    }

    func list() throws -> String {
        String(decoding: try JSONSerialization.data(withJSONObject: records), as: UTF8.self)
    }

    func rename(_ args: [String]) throws -> String {
        renames += 1
        let id = args[4], name = args[5]
        guard let index = records.firstIndex(where: { $0["id"] as? String == id }) else {
            throw RegistryQueryError("Session \(id) was not found")
        }
        records[index]["title"] = name
        records[index]["title_source"] = "human"
        return String(decoding: try JSONSerialization.data(withJSONObject: records[index]), as: UTF8.self)
    }
}
