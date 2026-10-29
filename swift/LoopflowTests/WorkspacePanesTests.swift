#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Retained workspace panes")
@MainActor
struct WorkspacePanesTests {
    private let location = WorkspaceIdentity(machineId: "fixture", worktree: "/fixture/task-a")

    private func session(_ id: String) throws -> SessionRecord {
        let path = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/session.json")
        var wire = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: path)) as? [String: Any])
        wire["id"] = id
        wire["task_primary"] = true
        wire["state"] = "unknown"
        wire["workspace"] = ["machine_id": location.machineId, "worktree": location.worktree,
                             "task_id": "task-a", "unavailable": NSNull()]
        return try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: wire))
    }

    @Test("Toggling both tools away keeps conversation entry in the session pane")
    func collapsedToolsKeepEntry() throws {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Rendering cannot open work") }
        let model = WorkModel(query: query, repoPath: location.worktree)
        let workspace = SessionsWorkspace(key: .init(repository: "repo", kind: .task, id: "task-a"),
            title: "Task A", location: location, localMachineId: location.machineId)
        let sessions = workspace.sessionStore(repoPath: location.worktree, query: query)
        try workspace.toggleShell()
        workspace.toggleBrowser()
        try workspace.toggleShell()
        workspace.toggleBrowser()
        let store = workspace.multiplexer
        #expect(store.visibleLayout?.allPanes.map(\.id) == ["session"])
        #expect(store.shellPane?.content.selectedShell != nil)
        let view = WorkspacePanesView(workspace: workspace, isFocused: true, sessions: sessions,
            model: model, onFocus: { id, _ in store.setFocusedPane(id) })
        let button = try view.inspect().find(viewWithAccessibilityIdentifier: "task-open-conversation").button()
        #expect(try button.labelView().text().string() == "Open conversation")
        #expect(sessions.sessions.isEmpty)
    }

    @Test("A failed Task conversation shows its reason beside Open conversation")
    func failedConversationShowsItsReason() throws {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Rendering cannot open work") }
        let model = WorkModel(query: query, repoPath: location.worktree)
        let workspace = SessionsWorkspace(key: .init(repository: "repo", kind: .task, id: "task-a"),
            title: "Task A", location: location, localMachineId: location.machineId)
        let sessions = workspace.sessionStore(repoPath: location.worktree, query: query)
        workspace.conversationError = "Conversation launcher unavailable"
        let view = WorkspacePanesView(workspace: workspace, isFocused: true, sessions: sessions,
            model: model, onFocus: { _, _ in })
        _ = try view.inspect().find(text: "Conversation launcher unavailable")
    }

    @Test("Rendering borrows Task A while B is selected, retaining Files and caller-owned focus")
    func renderRetainedOwner() async throws {
        var reads = 0
        let query = RegistryQuery { _, _ in
            await MainActor.run { reads += 1 }
            throw RegistryQueryError("Projection must not acquire or prepare work")
        }
        let model = WorkModel(query: query)
        let registry = SessionsWorkspaceRegistry(localMachineId: location.machineId)
        _ = registry.workspace(for: .init(repository: "repo", kind: .task, id: "task-b"),
            title: "Task B", location: nil)
        model.bindWorkspaces(registry, repository: "repo")
        model.select(.task(id: "task-b"))
        let openWork = registry.openWork
        let workspace = registry.workspace(for: .init(repository: "repo", kind: .task, id: "task-a"),
            title: "Task A", location: location)
        let sessions = workspace.sessionStore(repoPath: location.worktree, query: query)
        let primary = try session("primary")
        sessions.reconcile([primary])
        let store = workspace.multiplexer
        store.load(sessionId: primary.id, location: location)
        let primaryPane = store.focusedPane
        store.showBrowser(.files, location: location)
        let pane = store.focusedPane
        let files = try workspace.files(for: pane.content, task: nil, query: query)
        let document = files.document("note.txt")
        document.editor.string = "unfinished Task A draft"
        document.editor.setSelectedRange(NSRange(location: 3, length: 5))
        let layout = store.layout
        var focused: String?
        func view() -> WorkspacePanesView {
            WorkspacePanesView(workspace: workspace, isFocused: false, sessions: sessions,
                model: model, onFocus: { id, _ in focused = id })
        }
        let rendered = try view().inspect().findAll(SessionPaneView.self)
            .first { try $0.actualView().pane.id == primaryPane.id }?.actualView()
        let primaryView = try #require(rendered)
        #expect(primaryView.workspace === workspace)
        #expect(primaryView.sessions === sessions)
        #expect(!primaryView.isFocused)
        try primaryView.inspect().find(viewWithAccessibilityIdentifier: "pane-strip-\(primaryPane.id)").callOnTapGesture()
        #expect(focused == primaryPane.id)
        #expect(model.selection == .task(id: "task-b"))
        #expect(store.focusedPane == pane)

        store.setZoom(pane.id, enabled: true)
        let zoomed = try view().inspect().find(SessionPaneView.self).actualView()
        #expect(zoomed.pane == pane)
        let renderedFiles = try zoomed.inspect().find(TaskFilesView.self).actualView()
        #expect(renderedFiles.store === files)
        #expect(renderedFiles.store.document("note.txt") === document)
        store.setZoom(pane.id, enabled: false)
        let full = try view().inspect().findAll(SessionPaneView.self)
        #expect(try full.map { try $0.actualView().pane } == store.visibleLayout?.allPanes)
        #expect(store.layout == layout)
        #expect(document.editor.string == "unfinished Task A draft")
        #expect(document.editor.selectedRange() == NSRange(location: 3, length: 5))
        #expect(registry.retained(workspace.key) === workspace)
        #expect(registry.openWork == openWork)
        #expect(sessions.sessions.first?.state == .pending)
        await Task.yield()
        #expect(reads == 0)
    }

    @Test("The existing full-Task consumer keeps pane focus, Session context and controls")
    func fullTaskConsumer() throws {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Rendering cannot open work") }
        let model = WorkModel(query: query, repoPath: location.worktree)
        let registry = SessionsWorkspaceRegistry(localMachineId: location.machineId)
        let workspace = registry.workspace(for: .init(repository: "repo", kind: .task, id: "task-a"),
            title: "Task A", location: location)
        let other = registry.workspace(for: .init(repository: "repo", kind: .task, id: "task-b"),
            title: "Task B", location: location)
        let view = SessionsContentView(model: model, repoPath: location.worktree,
            workspaces: registry, machineId: location.machineId, repository: "repo", query: query)
        model.select(workspace.key.reference)
        workspace.presentation.content = .terminals
        let sessions = registry.sessionStore(repository: "repo", repoPath: location.worktree, query: query)
        let primary = try session("primary")
        sessions.reconcile([primary])
        workspace.multiplexer.load(sessionId: primary.id, location: location)
        let pane = workspace.multiplexer.focusedPane
        let shell = try #require(workspace.multiplexer.newShell(location: location))
        let retained = try view.inspect().findAll(WorkspacePanesView.self)
            .first { try $0.actualView().workspace === workspace }?.actualView()
        let panes = try #require(retained)
        let rendered = try panes.inspect().findAll(SessionPaneView.self)
            .first { try $0.actualView().pane.id == pane.id }?.actualView()
        let primaryView = try #require(rendered)
        #expect(!primaryView.isFocused)
        try primaryView.inspect().find(viewWithAccessibilityIdentifier: "pane-strip-\(pane.id)").callOnTapGesture()
        #expect(model.selection == workspace.key.reference)
        #expect(model.navigation.selectedSessionId == primary.id)
        #expect(workspace.multiplexer.focusedPane == pane)
        workspace.stripHover(pane.id).isHovered = true
        let active = try view.inspect().findAll(SessionPaneView.self)
            .first { try $0.actualView().pane.id == pane.id }?.actualView()
        let activeView = try #require(active)
        #expect(activeView.isFocused)
        #expect(other.multiplexer.sessionPane?.sessionId == nil)
        try activeView.inspect().find(viewWithAccessibilityIdentifier: "pane-hide").button().tap()
        #expect(workspace.multiplexer.collapsedPaneIds.contains(pane.id))
        #expect(workspace.multiplexer.layout.pane(for: shell.id) == shell)
        #expect(sessions.sessions.first?.id == primary.id)
    }

    @Test("Every Task pane hides on hover and the header's Show… restores it")
    func hoverHideAndShow() throws {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Rendering cannot open work") }
        let model = WorkModel(query: query, repoPath: location.worktree)
        let workspace = SessionsWorkspace(key: .init(repository: "repo", kind: .task, id: "task-a"),
            title: "Task A", location: location, localMachineId: location.machineId)
        let sessions = workspace.sessionStore(repoPath: location.worktree, query: query)
        try workspace.toggleShell()
        let store = workspace.multiplexer
        let shell = try #require(store.shellPane)
        let browser = try #require(store.browserPane)
        // Tools start hidden; Show… lists them as `lf desktop state` does.
        #expect(ShowHiddenMenu(workspace: workspace).items.map(\.title) == ["Browser"])
        // An unfocused pane offers Hide on hover too.
        let session = try #require(store.sessionPane)
        #expect(store.focusedPaneId == shell.id)
        workspace.stripHover(session.id).isHovered = true
        let view = SessionPaneView(pane: session, isFocused: false, sessions: sessions, workspace: workspace,
                                   model: model, onFocus: { _, _ in })
        try view.inspect().find(viewWithAccessibilityIdentifier: "pane-hide").button().tap()
        #expect(store.collapsedPaneIds == [session.id, browser.id])
        let menu = ShowHiddenMenu(workspace: workspace)
        #expect(menu.items.map(\.title) == ["Session", "Browser"])
        try menu.inspect().find(viewWithAccessibilityIdentifier: "show-pane-\(session.id)").button().tap()
        try ShowHiddenMenu(workspace: workspace).inspect()
            .find(viewWithAccessibilityIdentifier: "show-pane-\(browser.id)").button().tap()
        #expect(store.collapsedPaneIds.isEmpty)
        // Nothing hidden: the header has no Show… menu.
        #expect(throws: (any Error).self) {
            try ShowHiddenMenu(workspace: workspace).inspect().find(viewWithAccessibilityIdentifier: "show-hidden-panes")
        }
    }
}
#endif
