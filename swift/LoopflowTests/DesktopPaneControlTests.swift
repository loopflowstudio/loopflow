#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Exact Desktop pane visibility", .serialized)
@MainActor
struct DesktopPaneControlTests {
    private let repository = "selected-plan"
    private let identity = WorkspaceIdentity(machineId: "machine-a", worktree: "/same/path")

    private func register(_ router: WorkLinkRouter, window: UUID, registry: SessionsWorkspaceRegistry) {
        router.register(window, repository: repository, focus: {},
            inspect: { incarnation in
                DesktopWindowInspection(repository: repository, window: incarnation.uuidString, path: "/repo",
                    selectionKind: nil, selectionId: nil, reading: "unavailable", reason: "Offline",
                    task: nil, session: nil, supportedOperations: ["inspect", "hide", "restore"],
                    workspaces: registry.inspect(), layouts: registry.inspectLayouts())
            }, setVisibility: registry.setVisibility) { _ in }
    }

    private func target(_ pane: PaneState, window: UUID, machine: String = "machine-a") -> DesktopPaneTarget {
        DesktopPaneTarget(repository: repository, window: window.uuidString, machineId: machine,
            worktree: identity.worktree, pane: pane.id, incarnation: pane.incarnation)
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
            let report = try router.setVisibility(DesktopPaneVisibility(target: request, hidden: true))
            #expect(report.windows[0].workspaces[0].hiddenPanes == [shell.id])
            #expect(store.focusedPane == session)
        }
        let report = try router.setVisibility(DesktopPaneVisibility(target: request, hidden: false))
        #expect(report.windows[0].workspaces[0].hiddenPanes.isEmpty)
        #expect(store.layout == layout)
        #expect(store.focusedPane == session)
        #expect(store.shellCommands[shell.id] == ["fixture-shell"])
        #expect(registry.workspace(for: identity) === workspace)
        #expect(workspace.surfaces === registry.surfaces)
        #expect(!store.canUndoClose)
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
            try router.setVisibility(DesktopPaneVisibility(target: old, hidden: true))
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
            try router.setVisibility(DesktopPaneVisibility(target: target(shell, window: window), hidden: true))
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
            try router.setVisibility(DesktopPaneVisibility(target: target(pane, window: old, machine: "machine-b"), hidden: true))
        }
        #expect(registry.paths == [identity])
        register(router, window: replacement, registry: registry)
        router.remove(old, repository: repository)
        #expect(throws: RegistryQueryError.self) {
            try router.setVisibility(DesktopPaneVisibility(target: target(pane, window: old), hidden: true))
        }
        #expect(store.collapsedPaneIds.isEmpty)
        _ = try router.setVisibility(DesktopPaneVisibility(target: target(pane, window: replacement), hidden: true))
        #expect(store.collapsedPaneIds == [pane.id])
        router.remove(replacement, repository: repository)
        #expect(throws: RegistryQueryError.self) {
            try router.setVisibility(DesktopPaneVisibility(target: target(pane, window: replacement), hidden: false))
        }
        #expect(store.collapsedPaneIds == [pane.id])
    }
}
#endif
