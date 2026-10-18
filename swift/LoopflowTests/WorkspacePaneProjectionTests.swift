#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Retained pane projection")
@MainActor
struct WorkspacePaneProjectionTests {
    private let location = WorkspaceIdentity(machineId: "fixture", worktree: "/fixture/task-a")

    private func session(_ id: String, primary: Bool = false, closed: Bool = false) throws -> SessionRecord {
        let path = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/session.json")
        var wire = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: path)) as? [String: Any])
        wire["id"] = id
        wire["task_primary"] = primary
        wire["state"] = closed ? "closed" : "unknown"
        wire["workspace"] = ["machine_id": location.machineId, "worktree": location.worktree,
                             "task_id": "task-a", "unavailable": NSNull()]
        return try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: wire))
    }

    @Test("Focus cannot replace the primary face; zoom and unzoom project exact retained panes")
    func primaryAndZoom() throws {
        let primary = try session("primary", primary: true)
        let secondary = try session("secondary")
        let workspace = SessionsWorkspace(key: .init(repository: "repo", kind: .task, id: "task-a"),
            title: "Task A", location: location, localMachineId: location.machineId)
        let store = workspace.multiplexer
        store.load(sessionId: primary.id, location: location)
        let original = store.focusedPane
        store.reveal(sessionId: secondary.id, location: location)
        let secondaryPane = store.focusedPane
        let shell = try #require(store.newShell(location: location))
        store.show(.files(taskId: "task-a", location: location))
        let files = store.focusedPane
        let layout = store.layout
        for focused in [secondaryPane, shell, files] {
            store.setFocusedPane(focused.id)
            #expect(TaskPaneProjection.resolve(in: store, sessions: [secondary, primary]) == .pane(id: original.id))
            store.setZoom(focused.id, enabled: true)
            #expect(TaskPaneProjection.resolve(in: store, sessions: []) == .pane(id: focused.id))
            store.setZoom(focused.id, enabled: false)
            #expect(TaskPaneProjection.resolve(in: store, sessions: [secondary, primary]) == .pane(id: original.id))
            #expect(store.layout == layout)
            #expect(store.layout.pane(for: original.id) == original)
        }
        store.setCollapsed(paneId: original.id, collapsed: true)
        #expect(TaskPaneProjection.resolve(in: store, sessions: [primary]) == .pane(id: original.id))
        #expect(store.collapsedPaneIds == [original.id])
    }

    @Test("Missing, closed, ambiguous and unretained primaries never substitute another Session")
    func unavailablePrimary() throws {
        let store = MultiplexerStore()
        store.load(sessionId: "secondary", location: location)
        let primary = try session("primary", primary: true)
        let secondary = try session("secondary")
        #expect(TaskPaneProjection.resolve(in: store, sessions: [secondary]) == .missingPrimary)
        #expect(TaskPaneProjection.resolve(in: store, sessions: [try session("closed", primary: true, closed: true)]) == .missingPrimary)
        #expect(TaskPaneProjection.resolve(in: store, sessions: [primary, secondary]) == .missingPane(sessionId: primary.id))
        #expect(TaskPaneProjection.resolve(in: store, sessions: [primary, try session("other", primary: true)]) == .ambiguousPrimary)
        store.setZoom(store.focusedPaneId, enabled: true)
        store.close(store.focusedPaneId)
        #expect(TaskPaneProjection.resolve(in: store, sessions: [primary]) == .missingPane(sessionId: primary.id))
    }

    @Test("Full and compact rendering borrow Task A while B is selected, retaining Files and caller-owned focus")
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
        let primary = try session("primary", primary: true)
        sessions.reconcile([primary])
        let store = workspace.multiplexer
        store.load(sessionId: primary.id, location: location)
        let primaryPane = store.focusedPane
        store.show(.files(taskId: "task-a", location: location))
        let pane = store.focusedPane
        let files = try workspace.files(for: pane.content, task: nil, query: query)
        let document = files.document("note.txt")
        document.editor.string = "unfinished Task A draft"
        document.editor.setSelectedRange(NSRange(location: 3, length: 5))
        let layout = store.layout
        var focused: String?
        func view(_ presentation: WorkspacePanesView.Presentation) -> WorkspacePanesView {
            WorkspacePanesView(workspace: workspace, isFocused: false, sessions: sessions,
                model: model, presentation: presentation, onFocus: { id, _ in focused = id })
        }
        for _ in 0..<3 {
            let tile = view(.taskTile(sessions: [primary]))
            let rendered = try tile.inspect().find(SessionPaneView.self).actualView()
            #expect(rendered.pane == primaryPane)
            #expect(rendered.workspace === workspace)
            #expect(rendered.sessions === sessions)
            #expect(!rendered.isFocused)
            try rendered.inspect().find(viewWithAccessibilityIdentifier: "pane-strip-\(primaryPane.id)").callOnTapGesture()
            #expect(focused == primaryPane.id)
            #expect(model.selection == .task(id: "task-b"))
            #expect(store.focusedPane == pane)

            store.setZoom(pane.id, enabled: true)
            for presentation: WorkspacePanesView.Presentation in [.layout, .taskTile(sessions: [primary])] {
                let rendered = try view(presentation).inspect().find(SessionPaneView.self).actualView()
                #expect(rendered.pane == pane)
                let renderedFiles = try rendered.inspect().find(TaskFilesView.self).actualView()
                #expect(renderedFiles.store === files)
                #expect(renderedFiles.store.document("note.txt") === document)
            }
            store.setZoom(pane.id, enabled: false)
            let full = try view(.layout).inspect().findAll(SessionPaneView.self)
            #expect(try full.map { try $0.actualView().pane } == layout.allPanes)
            #expect(store.layout == layout)
        }
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
        model.select(other.key.reference, besideFocused: true)
        let sessions = registry.sessionStore(repository: "repo", repoPath: location.worktree, query: query)
        let primary = try session("primary", primary: true)
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
        workspace.hover.paneId = pane.id
        let active = try view.inspect().findAll(SessionPaneView.self)
            .first { try $0.actualView().pane.id == pane.id }?.actualView()
        let activeView = try #require(active)
        #expect(activeView.isFocused)
        try activeView.inspect().find(viewWithAccessibilityIdentifier: "pane-split-right").button().tap()
        #expect(workspace.multiplexer.layout.allPanes.count == 3)
        #expect(other.multiplexer.layout.allPanes.count == 1)
        try activeView.inspect().find(viewWithAccessibilityIdentifier: "pane-close").button().tap()
        #expect(workspace.multiplexer.layout.pane(for: pane.id) == nil)
        #expect(workspace.multiplexer.layout.pane(for: shell.id) == shell)
        #expect(sessions.sessions.first?.id == primary.id)
    }

    @Test("Tile rendering exposes unavailable primary and shell surfaces without opening them")
    func renderUnavailable() throws {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("No acquisition") }
        let model = WorkModel(query: query)
        let workspace = SessionsWorkspace(key: .init(repository: "repo", kind: .task, id: "task-a"),
            title: "Task A", location: location, localMachineId: location.machineId)
        let sessions = workspace.sessionStore(repoPath: location.worktree, query: query)
        func view(_ records: [SessionRecord]) -> WorkspacePanesView {
            WorkspacePanesView(workspace: workspace, isFocused: false, sessions: sessions,
                model: model, presentation: .taskTile(sessions: records), onFocus: { _, _ in })
        }
        _ = try view([]).inspect().find(text: "Primary Session is unavailable.")
        _ = try view([session("primary", primary: true)]).inspect().find(text: "Primary Session has no retained pane.")
        let shell = try #require(workspace.multiplexer.newShell(location: location))
        workspace.multiplexer.setZoom(shell.id, enabled: true)
        let rendered = try view([]).inspect().find(SessionPaneView.self).actualView()
        _ = try rendered.inspect().find(text: "Shell surface is not retained here.")
        #expect(try rendered.inspect().findAll(GhosttyTerminalView.self).isEmpty)
        #expect(workspace.multiplexer.layout.pane(for: shell.id) == shell)
    }
}
#endif
