#if os(macOS)
import Foundation
import Testing
#if canImport(GhosttyKit)
import AppKit
import GhosttyKit
#endif
@testable import Loopflow
@testable import LoopflowMac

@Suite("Exact Desktop pane arrangement", .serialized)
@MainActor
struct DesktopPaneControlTests {
    private let repository = "selected-plan"
    private let identity = WorkspaceIdentity(machineId: "machine-a", worktree: "/same/path")

    private var workKey: WorkWorkspaceKey { WorkWorkspaceKey(repository: repository, kind: .task, id: "task") }

    private func register(_ router: WorkLinkRouter, window: UUID, registry: SessionsWorkspaceRegistry, model: WorkModel = WorkModel(query: RegistryQuery { _, _ in
        throw RegistryQueryError("Pane controls must not launch Work")
    })) {
        router.register(window, repository: repository, focus: {},
            inspect: { repository, incarnation in
                DesktopWindowInspection(repository: repository, window: incarnation.uuidString, path: "/repo",
                    selectionKind: nil, selectionId: nil, reading: "unavailable", reason: "Offline",
                    task: nil, session: nil, supportedOperations: ["list", "hide", "restore", "focus", "move", "resize", "zoom"],
                    workspaces: registry.inspect(), opening: nil)
            }, controlPane: { try registry.controlPane($0, model: model) }, readText: { try registry.readText($0) }) { _ in }
    }

    private func target(_ pane: PaneState, window: UUID, machine: String = "machine-a") -> DesktopPaneTarget {
        DesktopPaneTarget(repository: repository, window: window.uuidString, work: WorkWorkspaceKey(repository: repository, kind: .task, id: machine == identity.machineId ? "task" : "other"), pane: pane.id, incarnation: pane.incarnation)
    }

    @Test("A Task starts with Files at the golden ratio; switching preserves resized and closed panes")
    func taskStartsWithRetainedCompanion() throws {
        let workspace = SessionsWorkspace(key: workKey, title: "Task", location: identity, localMachineId: identity.machineId)
        let store = workspace.multiplexer
        store.load(sessionId: "main", location: identity)
        let main = store.focusedPane
        workspace.prepareTaskPanes()
        let browser = try #require(store.layout.allPanes.last)
        #expect(browser.content == .files(taskId: "task", location: identity))
        #expect(workspace.browserPage == .files)
        #expect(store.focusedPane == main)
        guard case .split(.vertical, _, _, let ratio) = store.layout else { Issue.record("Missing vertical split"); return }
        #expect(ratio == LayoutNode.goldenRatio)
        store.updateRatio(between: main.id, and: browser.id, ratio: 0.6)
        try workspace.selectContent(.files(taskId: "task", location: identity), in: main)
        #expect(store.focusedPaneId == browser.id)
        #expect(store.layout.allPanes.count == 2)
        #expect(store.layout.pane(for: main.id) == main)
        let retained = store.layout
        workspace.prepareTaskPanes()
        #expect(store.layout == retained)
        workspace.closePane(browser.id)
        workspace.prepareTaskPanes()
        #expect(store.layout.allPanes == [main])
    }



    @Test func literalTextDoesNotInterpretEscapesOrAcceptControlKeys() throws {
        for text in ["", "héλ🙂 e\u{301}", #""$(literal)"; \n"#] {
            try validateDesktopLiteralText(text)
        }
        for text in ["line\nsubmit", "\r", "\t", "\0", "\u{1b}[A", "\u{7f}", "\u{85}"] {
            #expect(throws: RegistryQueryError.self) { try validateDesktopLiteralText(text) }
        }
    }

    @Test func inputRequiresAnExistingTerminalAndNeverAllocatesOrFollowsFocus() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId)
        let router = WorkLinkRouter(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        store.show(.files(taskId: "fixture"))
        let companion = target(store.focusedPane, window: window)
        store.newShell(command: ["retained-command"], location: identity)
        let shell = target(store.focusedPane, window: window)
        store.load(sessionId: "focused-elsewhere", location: identity)
        store.setCollapsed(paneId: shell.pane, collapsed: true)
        register(router, window: window, registry: registry)
        let before = store.layout, focus = store.focusedPaneId
        for destination in [companion, shell] {
            for action in [DesktopPaneAction.text(surface: "missing", text: "draft"),
                           .key(surface: "missing", key: .enter)] {
                #expect(throws: RegistryQueryError.self) {
                    try router.controlPane(.init(target: destination, action: action))
                }
            }
        }
        #expect(store.layout == before)
        #expect(store.focusedPaneId == focus)
        #expect(store.collapsedPaneIds == [shell.pane])
        #expect(store.layout.pane(for: shell.pane)?.content.selectedShell?.command == ["retained-command"])
        #expect(registry.surfaces.programStatus(for: .shell(shell.pane, machineId: identity.machineId)) == nil)
    }


    @Test("Shell observation and exit use the pane's location, not the workspace path")
    func shellOwnsExecutionLocation() throws {
        let workspace = fixtureWork(fixtureWorkspace("/repository"))
        let location = WorkspaceIdentity(machineId: "peer", worktree: "/remote/task")
        let shell = try #require(workspace.multiplexer.newShell(command: ["peer-shell"], location: location))
        let tab = try #require(shell.content.selectedShell)
        let terminal = TerminalIdentity.shell(tab.id, machineId: "peer")
        #expect(shell.terminal == terminal)
        NotificationCenter.default.post(name: .ghosttySurfaceClosed,
            object: TerminalIdentity.shell(tab.id, machineId: fixtureMachineId))
        #expect(workspace.multiplexer.layout.pane(for: shell.id) == shell)
        NotificationCenter.default.post(name: .ghosttySurfaceClosed, object: terminal)
        #expect(workspace.multiplexer.focusedPane.content == .empty)
        let unknown = try #require(workspace.multiplexer.newShell())
        #expect(unknown.terminal == nil)
    }

    @Test(arguments: [DesktopTextRegion.screen, .scrollback, .selection])
    func passiveReadKeepsMissingSurfaceDistinctAndDoesNotFollowFocus(region: DesktopTextRegion) throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId)
        let router = WorkLinkRouter(), window = UUID()
        let workspace = registry.open(workKey, title: "Task", location: identity), store = workspace.multiplexer
        store.newShell(command: ["retained-command"], location: identity)
        let shell = store.focusedPane
        let request = DesktopTextRequest(target: target(shell, window: window), surface: "old-surface",
                                         region: region, maxBytes: 16)
        store.load(sessionId: "other-session", location: identity)
        store.setCollapsed(paneId: shell.id, collapsed: true)
        let before = store.layout, focus = store.focusedPane
        register(router, window: window, registry: registry)
        let reading = try router.readText(request)
        #expect(reading.request == request)
        #expect(reading.result == .unavailable(reason: .missingSurface))
        #expect(reading.hidden)
        store.setCollapsed(paneId: shell.id, collapsed: false)
        #expect(try !router.readText(request).hidden)
        store.setZoom(focus.id, enabled: true)
        #expect(try router.readText(request).hidden)
        let inspection = try #require(registry.inspect().first)
        #expect(inspection.layout.children.first?.surface == nil)
        #expect(store.layout == before)
        #expect(store.focusedPane == focus)
        #expect(store.layout.pane(for: shell.id)?.content.selectedShell?.command == ["retained-command"])
        #expect(registry.surfaces.programStatus(for: shell.terminal!) == nil)
        #expect(registry.keys == [workKey])
    }

    @Test func readRejectsStaleWindowContentAndByteLimitsWithoutChangingWorkspace() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId)
        let router = WorkLinkRouter(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        let empty = store.focusedPane
        let request = DesktopTextRequest(target: target(empty, window: window), surface: "surface", region: .screen, maxBytes: 16)
        register(router, window: window, registry: registry)
        #expect(try router.readText(request).result == .unavailable(reason: .notTerminal))
        for bound in [0, -1, 1_048_577] {
            #expect(throws: RegistryQueryError.self) {
                try router.readText(.init(target: request.target, surface: request.surface, region: .screen, maxBytes: bound))
            }
        }
        store.load(sessionId: "replacement", location: identity)
        let before = store.layout
        #expect(throws: RegistryQueryError.self) { try router.readText(request) }
        let current = DesktopTextRequest(target: target(store.focusedPane, window: window), surface: "surface", region: .screen, maxBytes: 16)
        register(router, window: UUID(), registry: registry)
        #expect(throws: RegistryQueryError.self) { try router.readText(current) }
        #expect(store.layout == before)
    }

    #if canImport(GhosttyKit)
    @Test(.requiresDisplay) func exactInputCombinesDraftAtCursorWithoutFollowingFocusOrSubmitting() async throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let result = directory.appendingPathComponent("submitted")
        func quote(_ value: String) -> String { "'" + value.replacingOccurrences(of: "'", with: "'\\''") + "'" }
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId)
        let router = WorkLinkRouter(), windowID = UUID()
        let workspace = registry.open(workKey, title: "Task", location: identity), store = workspace.multiplexer
        store.newShell(location: identity)
        let pane = store.focusedPane
        store.load(sessionId: "other", location: identity)
        let otherPane = store.focusedPane
        let terminal = pane.terminal!
        let other = otherPane.terminal!
        let view = registry.surfaces.view(for: terminal), otherView = registry.surfaces.view(for: other)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 800, height: 300),
                              styleMask: [.titled], backing: .buffered, defer: false)
        let root = NSView(frame: window.contentLayoutRect)
        window.contentView = root
        for (index, view) in [view, otherView].enumerated() {
            view.frame = CGRect(x: CGFloat(index * 400), y: 0, width: 400, height: 300)
            root.addSubview(view)
            view.workingDirectory = directory.path
            view.command = "/usr/bin/env -i HOME=\(quote(directory.path)) PATH=/usr/bin:/bin TERM=xterm-256color PS1=fixture-ready /bin/zsh -df"
            view.createSurface(manager: manager)
            _ = try #require(view.surface)
        }
        defer {
            registry.surfaces.release(terminal)
            registry.surfaces.release(other)
            window.contentView = nil
        }
        let token = try #require(registry.surfaces.surfaceIncarnation(for: terminal))
        let destination = target(pane, window: windowID)
        register(router, window: windowID, registry: registry)
        func text(_ pane: PaneState) throws -> String {
            let terminal = try #require(pane.terminal)
            let surface = try #require(registry.surfaces.surfaceIncarnation(for: terminal))
            let result = try router.readText(.init(target: target(pane, window: windowID),
                surface: surface, region: .screen, maxBytes: 65536)).result
            guard case .available(let text, _) = result else {
                throw RegistryQueryError("Fixture terminal output is unavailable")
            }
            return text
        }
        let deadline = ContinuousClock.now + .seconds(5)
        while try !text(pane).contains("fixture-ready")
            || !text(otherPane).contains("fixture-ready") {
            try #require(ContinuousClock.now < deadline)
            try await Task.sleep(for: .milliseconds(20))
        }
        func send(_ action: DesktopPaneAction) throws {
            _ = try router.controlPane(.init(target: destination, action: action))
        }
        try send(.text(surface: token, text: "printf '%s' 'drft'"))
        for _ in 0..<3 { try send(.key(surface: token, key: .left)) }
        #expect(window.makeFirstResponder(otherView))
        store.setCollapsed(paneId: pane.id, collapsed: true)
        let before = store.layout
        view.setMarkedText("unfinished", selectedRange: NSRange(location: 10, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        #expect(throws: RegistryQueryError.self) { try send(.text(surface: token, text: "a")) }
        #expect(throws: RegistryQueryError.self) { try send(.key(surface: token, key: .enter)) }
        #expect(view.hasMarkedText())
        view.unmarkText()
        try send(.text(surface: token, text: "a"))
        try send(.key(surface: token, key: .end))
        try send(.text(surface: token, text: " > " + quote(result.path)))
        let draftDeadline = ContinuousClock.now + .seconds(3)
        while try !text(pane).contains("submitted") {
            try #require(ContinuousClock.now < draftDeadline)
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(!FileManager.default.fileExists(atPath: result.path))
        try send(.key(surface: token, key: .enter))
        let submitDeadline = ContinuousClock.now + .seconds(3)
        while (try? String(contentsOf: result, encoding: .utf8)) != "draft" {
            try #require(ContinuousClock.now < submitDeadline)
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(try !text(otherPane).contains("printf"))
        #expect(store.layout == before)
        #expect(store.focusedPaneId == otherPane.id)
        #expect(window.firstResponder === otherView)
        #expect(registry.surfaces.surfaceIncarnation(for: terminal) == token)
    }

    @Test(.requiresDisplay) func boundedReadPreservesRegionsSelectionAndExitedSurfaceUntilReplacement() throws {
        _ = NSApplication.shared
        let manager = GhosttyManager.shared
        manager.initialize()
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId)
        let router = WorkLinkRouter(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        store.newShell(location: identity)
        let pane = store.focusedPane
        let terminal = pane.terminal!
        let view = registry.surfaces.view(for: terminal)
        view.setFrameSize(CGSize(width: 400, height: 300))
        view.workingDirectory = NSTemporaryDirectory()
        // Leave the Unicode prefix in scrollback, away from the viewport.
        view.command = "/bin/sh -c 'printf aé界😀z; i=0; while [ $i -lt 100 ]; do printf \"\\r\\nline\"; i=$((i+1)); done; printf \"\\r\\nretained\"'"
        view.createSurface(manager: manager)
        defer { registry.surfaces.release(terminal) }
        let surface = try #require(view.surface)
        let token = try #require(registry.surfaces.surfaceIncarnation(for: terminal))
        register(router, window: window, registry: registry)
        store.newShell(location: identity)
        store.setCollapsed(paneId: pane.id, collapsed: true)
        let before = store.layout, focus = store.focusedPaneId
        // Observe the exited-but-retained surface before queued lifecycle cleanup.
        let deadline = Date().addingTimeInterval(3)
        while !ghostty_surface_process_exited(surface), Date() < deadline { Thread.sleep(forTimeInterval: 0.01) }
        try #require(ghostty_surface_process_exited(surface))
        func read(_ region: DesktopTextRegion, maxBytes: Int) throws -> DesktopTextResult {
            let reading = try router.readText(.init(target: target(pane, window: window), surface: token,
                                                   region: region, maxBytes: maxBytes))
            #expect(reading.hidden)
            return reading.result
        }
        #expect(try read(.selection, maxBytes: 1) == .available(text: "", truncated: false))
        let prefix = "aé界😀z"
        for limit in 1...prefix.utf8.count {
            var bytes = Array(prefix.utf8.prefix(limit))
            while String(bytes: bytes, encoding: .utf8) == nil { bytes.removeLast() }
            #expect(try read(.scrollback, maxBytes: limit) == .available(
                text: String(decoding: bytes, as: UTF8.self), truncated: true))
        }
        let result = try read(.screen, maxBytes: 65536)
        guard case .available(let screen, let truncated) = result else {
            throw RegistryQueryError("Fixture viewport is unavailable")
        }
        #expect(screen.contains("retained"))
        #expect(!screen.contains(prefix))
        #expect(!truncated)
        let action = "select_all"
        try #require(action.withCString { ghostty_surface_binding_action(surface, $0, UInt(action.utf8.count)) })
        let selected = try read(.selection, maxBytes: 65536)
        #expect(selected == (try read(.scrollback, maxBytes: 65536)))
        _ = try read(.screen, maxBytes: 1)
        _ = try read(.scrollback, maxBytes: 1)
        #expect(try read(.selection, maxBytes: 65536) == selected)
        #expect(store.layout == before)
        #expect(store.focusedPaneId == focus)
        #expect(registry.surfaces.surfaceIncarnation(for: terminal) == token)
        #expect(view.surface == surface)
        let request = DesktopTextRequest(target: target(pane, window: window), surface: token,
                                         region: .screen, maxBytes: 65536)
        for action in [DesktopPaneAction.text(surface: token, text: "must not reopen"), .key(surface: token, key: .enter)] {
            #expect(throws: RegistryQueryError.self) { try router.controlPane(.init(target: request.target, action: action)) }
        }
        #expect(view.surface == surface)
        #expect(registry.surfaces.surfaceIncarnation(for: terminal) == token)
        registry.surfaces.release(terminal)
        #expect(try router.readText(request).result == .unavailable(reason: .missingSurface))
        let replacement = registry.surfaces.view(for: terminal)
        replacement.setFrameSize(CGSize(width: 400, height: 300))
        replacement.workingDirectory = NSTemporaryDirectory()
        replacement.command = "/bin/cat"
        replacement.createSurface(manager: manager)
        _ = try #require(replacement.surface)
        #expect(throws: RegistryQueryError.self) { try router.readText(request) }
        for action in [DesktopPaneAction.text(surface: token, text: "stale"), .key(surface: token, key: .enter)] {
            #expect(throws: RegistryQueryError.self) { try router.controlPane(.init(target: request.target, action: action)) }
        }
        #expect(replacement.surface != nil)
    }

    @Test func sameTerminalIdOnAnotherMachineKeepsItsOwnViewAndLifetime() {
        let pool = GhosttySurfacePool()
        let local = TerminalIdentity.session("same-session", machineId: "local")
        let peer = TerminalIdentity.session("same-session", machineId: "peer")
        #expect(pool.surfaceIncarnation(for: local) == nil)
        #expect(pool.programStatus(for: local) == nil)
        let first = pool.view(for: local)
        let other = pool.view(for: peer)
        #expect(first !== other)
        #expect(first.programStatus.incarnation != other.programStatus.incarnation)
        // A retained view without a native surface must not advertise one.
        #expect(pool.surfaceIncarnation(for: local) == nil)
        pool.release(local)
        #expect(pool.programStatus(for: local) == nil)
        #expect(pool.programStatus(for: peer) === other.programStatus)
        let replacement = pool.view(for: local)
        #expect(replacement.programStatus.incarnation != first.programStatus.incarnation)
        pool.discard(first)
        #expect(pool.programStatus(for: local) === replacement.programStatus)
        pool.release(local)
        pool.release(peer)
    }
    #endif

    @Test func companionsRetainDraftsAndDoNotFollowFocusOrReplaceContent() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from:
            Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let model = WorkModel(query: RegistryQuery { _, _ in
            throw RegistryQueryError("Companion creation must not read or launch Work")
        }, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        model.navigation.preparedTaskWorktrees[task.id] = identity
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId), router = WorkLinkRouter(), window = UUID()
        let workKey = WorkWorkspaceKey(repository: repository, kind: .task, id: task.runtime?.workId ?? task.id)
        let workspace = registry.open(workKey, title: "Task", location: identity), store = workspace.multiplexer
        store.load(sessionId: "retained-session", location: identity)
        let session = store.focusedPane
        let source = DesktopPaneTarget(repository: repository, window: window.uuidString, work: workKey, pane: session.id, incarnation: session.incarnation)
        store.newShell(command: ["retained-shell-command"], location: identity)
        let selected = store.focusedPane
        store.setZoom(selected.id, enabled: true)
        register(router, window: window, registry: registry, model: model)
        let document = workspace.files(issue: task.id, cwd: identity.worktree).document("note.txt")
        document.editor.string = "unfinished draft"
        document.editor.setSelectedRange(NSRange(location: 2, length: 4))
        for action in [DesktopPaneAction.files(task: task.id), .graphLog(task: task.id)] {
            let created = try router.controlPane(.init(target: source, action: action))
            let reused = try router.controlPane(.init(target: source, action: action))
            #expect(created.pane == reused.pane)
            #expect(created.pane != source.pane)
            #expect(store.layout.pane(for: created.pane) != nil)
        }
        #expect(store.layout.allPanes.count == 4)
        let files = try #require(store.layout.allPanes.first { $0.content == .files(taskId: task.id, location: identity) })
        store.setCollapsed(paneId: files.id, collapsed: true)
        _ = try router.controlPane(.init(target: source, action: .files(task: task.id)))
        #expect(!store.collapsedPaneIds.contains(files.id))
        #expect(store.layout.pane(for: files.id) == files)
        let shellReceipt = try router.controlPane(.init(target: source, action: .shell))
        #expect(shellReceipt.pane == selected.id)
        #expect(store.layout.allPanes.count == 4)
        #expect(store.layout.pane(for: session.id) == session)
        #expect(store.focusedPaneId == selected.id)
        #expect(store.zoomedPaneId == selected.id)
        #expect(store.shell(selected.content.selectedShell!.id)?.command == ["retained-shell-command"])
        #expect(document.editor.string == "unfinished draft")
        #expect(document.editor.selectedRange() == NSRange(location: 2, length: 4))
        #expect(model.selection == nil)
        let before = store.layout
        model.navigation.preparedTaskWorktrees[task.id] = WorkspaceIdentity(machineId: "other", worktree: identity.worktree)
        for action in [DesktopPaneAction.files(task: task.id), .graphLog(task: task.id)] {
            #expect(throws: RegistryQueryError.self) { try router.controlPane(.init(target: source, action: action)) }
            #expect(store.layout == before)
        }
    }

    @Test func remoteShellRefusalPreservesTheWorkspace() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: "other"), router = WorkLinkRouter(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        let before = store.layout
        register(router, window: window, registry: registry)
        #expect(throws: RegistryQueryError.self) {
            try router.controlPane(.init(target: target(store.focusedPane, window: window), action: .shell))
        }
        #expect(store.layout == before)
        #expect(store.shellPane == nil)
    }

    @Test func hideRestoreRetainsContentAndIgnoresLaterFocus() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), window = UUID()
        let workspace = registry.open(workKey, title: "Task", location: identity), store = workspace.multiplexer
        store.newShell(command: ["fixture-shell"], location: identity)
        let shell = store.focusedPane
        store.load(sessionId: "retained-draft", location: identity)
        let session = store.focusedPane
        let layout = store.layout
        register(router, window: window, registry: registry)
        let request = target(shell, window: window)

        for _ in 0..<2 { // Repeating after a lost reply is idempotent.
            let report = try router.controlPane(DesktopPaneCommand(target: request, action: .hide))
            #expect(report.pane == shell.id)
            #expect(router.inspect().windows[0].workspaces[0].hiddenPanes == [shell.id])
            #expect(store.focusedPane == session)
        }
        let report = try router.controlPane(DesktopPaneCommand(target: request, action: .restore))
        #expect(report.pane == shell.id)
        #expect(router.inspect().windows[0].workspaces[0].hiddenPanes.isEmpty)
        #expect(store.layout == layout)
        #expect(store.focusedPane == session)
        #expect(store.layout.pane(for: shell.id)?.content.selectedShell?.command == ["fixture-shell"])
        #expect(registry.open(workKey, title: "Task", location: identity) === workspace)
        #expect(workspace.surfaces === registry.surfaces)
        #expect(!store.canUndoClose)
    }

    @Test func arrangementRetainsDocumentsSurfacesSelectionAndContentOccurrences() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId), window = UUID()
        let workspace = registry.open(workKey, title: "Task", location: identity), store = workspace.multiplexer
        store.newShell(command: ["fixture-shell"], location: identity)
        let shell = store.focusedPane
        store.load(sessionId: "retained-session", location: identity)
        let session = store.focusedPane
        store.show(.files(taskId: "task"))
        let filesPane = store.focusedPane
        let document = workspace.files(issue: "TASK", cwd: identity.worktree).document("note.txt")
        document.editor.string = "unfinished draft"
        document.editor.setSelectedRange(NSRange(location: 3, length: 5))
        #if GHOSTTY_ENABLED
        let shellView = registry.surfaces.view(for: shell.terminal!)
        let sessionView = registry.surfaces.view(for: .session("retained-session", machineId: identity.machineId))
        #endif
        register(router, window: window, registry: registry)
        let shellTarget = target(shell, window: window), sessionTarget = target(session, window: window)
        // Later focus must not redirect any of these requests.
        store.setFocusedPane(filesPane.id)
        _ = try router.controlPane(.init(target: shellTarget, action: .move(destination: sessionTarget, axis: .vertical)))
        #expect(store.layout.allPanes.map(\.id) == [session.id, shell.id, filesPane.id])
        _ = try router.controlPane(.init(target: shellTarget, action: .resize(toward: sessionTarget, ratio: 0.7)))
        let reading = DesktopLayoutInspection(store.layout, surface: { _ in nil })
        let movedSplit = reading.children[0]
        #expect(movedSplit.axis == "vertical")
        #expect(abs(try #require(movedSplit.ratio) - 0.3) < 0.0001)
        for _ in 0..<2 {
            _ = try router.controlPane(.init(target: shellTarget, action: .zoom(enabled: true)))
            #expect(store.zoomedPaneId == shell.id)
            #expect(store.focusedPane == filesPane)
        }
        _ = try router.controlPane(.init(target: sessionTarget, action: .zoom(enabled: false)))
        #expect(store.zoomedPaneId == shell.id) // Do not unzoom a different target.
        _ = try router.controlPane(.init(target: shellTarget, action: .zoom(enabled: false)))
        #expect(store.zoomedPaneId == nil)
        #expect(store.focusedPane == filesPane)
        #expect(store.layout.pane(for: shell.id) == shell)
        #expect(store.layout.pane(for: session.id) == session)
        #expect(store.layout.pane(for: shell.id)?.content.selectedShell?.command == ["fixture-shell"])
        #expect(document.editor.string == "unfinished draft")
        #expect(document.editor.selectedRange() == NSRange(location: 3, length: 5))
        #expect(registry.open(workKey, title: "Task", location: identity) === workspace)
        #expect(workspace.surfaces === registry.surfaces)
        #if GHOSTTY_ENABLED
        #expect(registry.surfaces.view(for: shell.terminal!) === shellView)
        #expect(registry.surfaces.view(for: .session("retained-session", machineId: identity.machineId)) === sessionView)
        #endif
        _ = try router.controlPane(.init(target: shellTarget, action: .focus))
        #expect(store.focusedPane == shell)
    }

    @Test func staleSourceRejectsEveryActionWithoutChangingTheCurrentFocus() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        store.load(sessionId: "old", location: identity)
        let old = target(store.focusedPane, window: window)
        store.load(sessionId: "replacement", location: identity)
        store.newShell(location: identity)
        let peer = target(store.focusedPane, window: window)
        register(router, window: window, registry: registry)
        let before = store.layout, focus = store.focusedPaneId
        let actions: [DesktopPaneAction] = [.hide, .restore, .focus, .shell,
            .move(destination: peer, axis: .vertical), .resize(toward: peer, ratio: 0.6), .zoom(enabled: true), .files(task: "task"), .graphLog(task: "task"),
            .text(surface: "stale", text: "draft"), .key(surface: "stale", key: .enter)]
        for action in actions {
            #expect(throws: RegistryQueryError.self) { try router.controlPane(.init(target: old, action: action)) }
            #expect(store.layout == before)
            #expect(store.focusedPaneId == focus)
            #expect(store.zoomedPaneId == nil)
            #expect(store.collapsedPaneIds.isEmpty)
        }
    }

    @Test func invalidSecondTargetsAndRatiosLeaveBothPanesUntouched() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        store.newShell(location: identity)
        let shell = target(store.focusedPane, window: window)
        store.load(sessionId: "old", location: identity)
        let stale = target(store.focusedPane, window: window)
        store.load(sessionId: "new", location: identity)
        let peer = target(store.focusedPane, window: window)
        register(router, window: window, registry: registry)
        let before = store.layout
        let outside = DesktopPaneTarget(repository: "different-plan", window: window.uuidString,
            work: workKey, pane: peer.pane, incarnation: peer.incarnation)
        for destination in [stale, shell, outside, target(store.focusedPane, window: UUID()),
                            target(store.focusedPane, window: window, machine: "peer")] {
            for action in [DesktopPaneAction.move(destination: destination, axis: .vertical),
                           .resize(toward: destination, ratio: 0.6)] {
                #expect(throws: RegistryQueryError.self) { try router.controlPane(.init(target: shell, action: action)) }
                #expect(store.layout == before)
                #expect(store.focusedPaneId == peer.pane)
            }
        }
        for ratio in [Double.nan, .infinity, 0, 1] {
            #expect(throws: RegistryQueryError.self) {
                try router.controlPane(.init(target: shell, action: .resize(toward: peer, ratio: ratio)))
            }
            #expect(store.layout == before)
        }
        #expect(registry.keys == [workKey])
    }

    @Test func focusingTheOnlyHiddenPaneRevealsItWithoutChangingItsContent() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        store.newShell(location: identity)
        let pane = store.focusedPane, request = target(pane, window: window)
        register(router, window: window, registry: registry)
        _ = try router.controlPane(.init(target: request, action: .hide))
        #expect(store.visibleLayout == nil)
        _ = try router.controlPane(.init(target: request, action: .focus))
        #expect(store.visibleLayout == .leaf(pane))
        #expect(store.focusedPane == pane)
    }

    @Test func samePaneAndSessionAfterReplacementDoesNotReviveOldTarget() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        store.load(sessionId: "a", location: identity)
        let old = target(store.focusedPane, window: window)
        register(router, window: window, registry: registry)
        store.load(sessionId: "b", location: identity)
        store.load(sessionId: "a", location: identity)
        #expect(store.focusedPane.id == old.pane)
        #expect(store.focusedPane.incarnation != old.incarnation)
        #expect(throws: RegistryQueryError.self) {
            try router.controlPane(DesktopPaneCommand(target: old, action: .hide))
        }
        #expect(store.collapsedPaneIds.isEmpty)
    }

    @Test func undoCloseKeepsContentsButInvalidatesTheRemovedOccurrence() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), window = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        store.newShell(location: identity)
        let shell = store.focusedPane
        register(router, window: window, registry: registry)
        store.close(shell.id)
        store.undoClose()
        #expect(store.focusedPane.content.selectedShell?.id == shell.content.selectedShell?.id)
        #expect(store.focusedPane.content.selectedShell?.command == nil)
        #expect(throws: RegistryQueryError.self) {
            try router.controlPane(DesktopPaneCommand(target: target(shell, window: window), action: .hide))
        }
        #expect(store.collapsedPaneIds.isEmpty)
    }

    @Test func wrongMachineAndReplacedWindowNeverChangeOrAllocateWorkspaces() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), old = UUID(), replacement = UUID()
        let store = registry.open(workKey, title: "Task", location: identity).multiplexer
        store.newShell(location: identity)
        let pane = store.focusedPane
        register(router, window: old, registry: registry)
        #expect(throws: RegistryQueryError.self) {
            try router.controlPane(DesktopPaneCommand(target: target(pane, window: old, machine: "machine-b"), action: .hide))
        }
        #expect(registry.keys == [workKey])
        register(router, window: replacement, registry: registry)
        router.remove(old, repository: repository)
        #expect(throws: RegistryQueryError.self) {
            try router.controlPane(DesktopPaneCommand(target: target(pane, window: old), action: .hide))
        }
        #expect(store.collapsedPaneIds.isEmpty)
        _ = try router.controlPane(DesktopPaneCommand(target: target(pane, window: replacement), action: .hide))
        #expect(store.collapsedPaneIds == [pane.id])
        router.remove(replacement, repository: repository)
        #expect(throws: RegistryQueryError.self) {
            try router.controlPane(DesktopPaneCommand(target: target(pane, window: replacement), action: .restore))
        }
        #expect(store.collapsedPaneIds == [pane.id])
    }
}
#endif
