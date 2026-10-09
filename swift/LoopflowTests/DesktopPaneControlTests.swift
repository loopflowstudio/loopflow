#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Exact Desktop pane arrangement", .serialized)
@MainActor
struct DesktopPaneControlTests {
    private let repository = "selected-plan"
    private let identity = WorkspaceIdentity(machineId: "machine-a", worktree: "/same/path")

    private func register(_ router: WorkLinkRouter, window: UUID, registry: SessionsWorkspaceRegistry, model: WorkModel = WorkModel(query: RegistryQuery { _, _ in
        throw RegistryQueryError("Pane controls must not launch Work")
    })) {
        router.register(window, repository: repository, focus: {},
            inspect: { incarnation in
                DesktopWindowInspection(repository: repository, window: incarnation.uuidString, path: "/repo",
                    selectionKind: nil, selectionId: nil, reading: "unavailable", reason: "Offline",
                    task: nil, session: nil, supportedOperations: ["inspect", "hide", "restore", "focus", "split", "move", "resize", "zoom"],
                    workspaces: registry.inspect(), layouts: registry.inspectLayouts())
            }, controlPane: { try registry.controlPane($0, model: model) }) { _ in }
    }

    private func target(_ pane: PaneState, window: UUID, machine: String = "machine-a") -> DesktopPaneTarget {
        DesktopPaneTarget(repository: repository, window: window.uuidString, machineId: machine,
            worktree: identity.worktree, pane: pane.id, incarnation: pane.incarnation)
    }

    @Test func terminalInspectionDoesNotAllocateOrFollowFocus() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId)
        let workspace = registry.workspace(for: identity)
        workspace.multiplexer.newShell(command: ["retained-command"])
        let shell = workspace.multiplexer.focusedPane
        let terminal = TerminalIdentity.shell(shell.id, machineId: identity.machineId)
        workspace.multiplexer.load(sessionId: "other-session")
        workspace.multiplexer.setCollapsed(paneId: shell.id, collapsed: true)
        let before = workspace.multiplexer.layout
        let reading = try #require(registry.inspect().first)
        #expect(reading.layout.children.first?.surface == nil)
        #expect(registry.surfaces.programStatus(for: terminal) == nil)
        #expect(workspace.multiplexer.layout == before)
        #expect(workspace.multiplexer.shellCommands[shell.id] == ["retained-command"])
        #expect(workspace.multiplexer.focusedPane.content == .session(id: "other-session"))
    }

    #if canImport(GhosttyKit)
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
        let workspace = registry.workspace(for: identity), store = workspace.multiplexer
        store.load(sessionId: "retained-session")
        let session = store.focusedPane
        let source = target(session, window: window)
        store.newShell(command: ["retained-shell-command"])
        let selected = store.focusedPane
        store.setZoom(selected.id, enabled: true)
        register(router, window: window, registry: registry, model: model)
        let document = workspace.files(taskId: task.id, issue: task.id, cwd: identity.worktree).document("note.txt")
        document.editor.string = "unfinished draft"
        document.editor.setSelectedRange(NSRange(location: 2, length: 4))
        for action in [DesktopPaneAction.files(task: task.id), .flowLog(task: task.id)] {
            _ = try router.controlPane(.init(target: source, action: action))
            _ = try router.controlPane(.init(target: source, action: action))
        }
        #expect(store.layout.allPanes.count == 4)
        let files = try #require(store.layout.allPanes.first { $0.content == .files(taskId: task.id) })
        store.setCollapsed(paneId: files.id, collapsed: true)
        _ = try router.controlPane(.init(target: source, action: .files(task: task.id)))
        #expect(!store.collapsedPaneIds.contains(files.id))
        #expect(store.layout.pane(for: files.id) == files)
        _ = try router.controlPane(.init(target: source, action: .shell))
        #expect(store.layout.allPanes.count == 5)
        #expect(store.layout.pane(for: session.id) == session)
        #expect(store.focusedPane == selected)
        #expect(store.zoomedPaneId == selected.id)
        #expect(store.shellCommands[selected.id] == ["retained-shell-command"])
        #expect(document.editor.string == "unfinished draft")
        #expect(document.editor.selectedRange() == NSRange(location: 2, length: 4))
        #expect(model.selection == nil)
        let before = store.layout
        model.navigation.preparedTaskWorktrees[task.id] = WorkspaceIdentity(machineId: "other", worktree: identity.worktree)
        for action in [DesktopPaneAction.files(task: task.id), .flowLog(task: task.id)] {
            #expect(throws: RegistryQueryError.self) { try router.controlPane(.init(target: source, action: action)) }
            #expect(store.layout == before)
        }
    }

    @Test func remoteShellRefusalPreservesTheWorkspace() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: "other"), router = WorkLinkRouter(), window = UUID()
        let store = registry.workspace(for: identity).multiplexer
        let before = store.layout
        register(router, window: window, registry: registry)
        #expect(throws: RegistryQueryError.self) {
            try router.controlPane(.init(target: target(store.focusedPane, window: window), action: .shell))
        }
        #expect(store.layout == before)
        #expect(store.shellCommands.isEmpty)
    }

    @Test func hideRestoreRetainsContentAndIgnoresLaterFocus() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), window = UUID()
        let workspace = registry.workspace(for: identity), store = workspace.multiplexer
        store.newShell(command: ["fixture-shell"])
        let shell = store.focusedPane
        store.load(sessionId: "retained-draft")
        let session = store.focusedPane
        let layout = store.layout
        register(router, window: window, registry: registry)
        let request = target(shell, window: window)

        for _ in 0..<2 { // Repeating after a lost reply is idempotent.
            let report = try router.controlPane(DesktopPaneCommand(target: request, action: .hide))
            #expect(report.windows[0].workspaces[0].hiddenPanes == [shell.id])
            #expect(store.focusedPane == session)
        }
        let report = try router.controlPane(DesktopPaneCommand(target: request, action: .restore))
        #expect(report.windows[0].workspaces[0].hiddenPanes.isEmpty)
        #expect(store.layout == layout)
        #expect(store.focusedPane == session)
        #expect(store.shellCommands[shell.id] == ["fixture-shell"])
        #expect(registry.workspace(for: identity) === workspace)
        #expect(workspace.surfaces === registry.surfaces)
        #expect(!store.canUndoClose)
    }

    @Test func arrangementRetainsDocumentsSurfacesSelectionAndContentOccurrences() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), window = UUID()
        let workspace = registry.workspace(for: identity), store = workspace.multiplexer
        store.newShell(command: ["fixture-shell"])
        let shell = store.focusedPane
        store.load(sessionId: "retained-session")
        let session = store.focusedPane
        store.show(.files(taskId: "task"))
        let filesPane = store.focusedPane
        let document = workspace.files(taskId: "task", issue: "TASK", cwd: identity.worktree).document("note.txt")
        document.editor.string = "unfinished draft"
        document.editor.setSelectedRange(NSRange(location: 3, length: 5))
        #if GHOSTTY_ENABLED
        let shellView = registry.surfaces.view(for: .shell(shell.id, machineId: identity.machineId))
        let sessionView = registry.surfaces.view(for: .session("retained-session", machineId: identity.machineId))
        #endif
        register(router, window: window, registry: registry)
        let shellTarget = target(shell, window: window), sessionTarget = target(session, window: window)
        // Later focus must not redirect any of these requests.
        store.setFocusedPane(filesPane.id)
        _ = try router.controlPane(.init(target: shellTarget, action: .split(axis: .horizontal)))
        #expect(store.layout.allPanes.count == 4)
        #expect(store.focusedPane == filesPane)
        let empty = try #require(store.layout.allPanes.first { $0.content == .empty })
        _ = try router.controlPane(.init(target: shellTarget, action: .move(destination: sessionTarget, axis: .vertical)))
        #expect(store.layout.allPanes.map(\.id) == [empty.id, session.id, shell.id, filesPane.id])
        _ = try router.controlPane(.init(target: shellTarget, action: .resize(toward: sessionTarget, ratio: 0.7)))
        let reading = DesktopLayoutInspection(store.layout, surface: { _ in nil })
        let movedSplit = reading.children[1].children[0]
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
        #expect(store.shellCommands[shell.id] == ["fixture-shell"])
        #expect(document.editor.string == "unfinished draft")
        #expect(document.editor.selectedRange() == NSRange(location: 3, length: 5))
        #expect(registry.workspace(for: identity) === workspace)
        #expect(workspace.surfaces === registry.surfaces)
        #if GHOSTTY_ENABLED
        #expect(registry.surfaces.view(for: .shell(shell.id, machineId: identity.machineId)) === shellView)
        #expect(registry.surfaces.view(for: .session("retained-session", machineId: identity.machineId)) === sessionView)
        #endif
        _ = try router.controlPane(.init(target: shellTarget, action: .focus))
        #expect(store.focusedPane == shell)
    }

    @Test func staleSourceRejectsEveryActionWithoutChangingTheCurrentFocus() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), window = UUID()
        let store = registry.workspace(for: identity).multiplexer
        store.load(sessionId: "old")
        let old = target(store.focusedPane, window: window)
        store.load(sessionId: "replacement")
        store.newShell()
        let peer = target(store.focusedPane, window: window)
        register(router, window: window, registry: registry)
        let before = store.layout, focus = store.focusedPaneId
        let actions: [DesktopPaneAction] = [.hide, .restore, .focus, .split(axis: .horizontal),
            .move(destination: peer, axis: .vertical), .resize(toward: peer, ratio: 0.6), .zoom(enabled: true), .shell, .files(task: "task"), .flowLog(task: "task")]
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
        let store = registry.workspace(for: identity).multiplexer
        store.newShell()
        let shell = target(store.focusedPane, window: window)
        store.load(sessionId: "old")
        let stale = target(store.focusedPane, window: window)
        store.load(sessionId: "new")
        let peer = target(store.focusedPane, window: window)
        register(router, window: window, registry: registry)
        let before = store.layout
        let outside = DesktopPaneTarget(repository: "different-plan", window: window.uuidString,
            machineId: identity.machineId, worktree: identity.worktree, pane: peer.pane, incarnation: peer.incarnation)
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
        #expect(registry.paths == [identity])
    }

    @Test func focusingTheOnlyHiddenPaneRevealsItWithoutChangingItsContent() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), window = UUID()
        let store = registry.workspace(for: identity).multiplexer
        store.newShell()
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
        let store = registry.workspace(for: identity).multiplexer
        store.load(sessionId: "a")
        let old = target(store.focusedPane, window: window)
        register(router, window: window, registry: registry)
        store.load(sessionId: "b")
        store.load(sessionId: "a")
        #expect(store.focusedPane.id == old.pane)
        #expect(store.focusedPane.incarnation != old.incarnation)
        #expect(throws: RegistryQueryError.self) {
            try router.controlPane(DesktopPaneCommand(target: old, action: .hide))
        }
        #expect(store.collapsedPaneIds.isEmpty)
    }

    @Test func undoCloseKeepsContentsButInvalidatesTheRemovedOccurrence() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), window = UUID()
        let store = registry.workspace(for: identity).multiplexer
        store.newShell()
        let shell = store.focusedPane
        register(router, window: window, registry: registry)
        store.close(shell.id)
        store.undoClose()
        #expect(store.focusedPane.content == .shell)
        #expect(throws: RegistryQueryError.self) {
            try router.controlPane(DesktopPaneCommand(target: target(shell, window: window), action: .hide))
        }
        #expect(store.collapsedPaneIds.isEmpty)
    }

    @Test func wrongMachineAndReplacedWindowNeverChangeOrAllocateWorkspaces() throws {
        let router = WorkLinkRouter(), registry = SessionsWorkspaceRegistry(), old = UUID(), replacement = UUID()
        let store = registry.workspace(for: identity).multiplexer
        store.newShell()
        let pane = store.focusedPane
        register(router, window: old, registry: registry)
        #expect(throws: RegistryQueryError.self) {
            try router.controlPane(DesktopPaneCommand(target: target(pane, window: old, machine: "machine-b"), action: .hide))
        }
        #expect(registry.paths == [identity])
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
