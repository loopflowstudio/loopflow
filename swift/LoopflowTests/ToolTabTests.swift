#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Retained Task tool tabs", .serialized)
@MainActor
struct ToolTabTests {
    let location = WorkspaceIdentity(machineId: "local", worktree: "/task")
    let work = WorkWorkspaceKey(repository: "repo", kind: .task, id: "task")

    @Test func twoShellsAndFilesRestoreIdentitiesSelectionAndDrafts() async throws {
        let store = MultiplexerStore(taskId: work.id)
        let firstPane = try #require(store.newShell(command: ["first"], location: location))
        let first = try #require(firstPane.content.selectedShell)
        let secondPane = try #require(store.newShell(command: ["second"], location: location))
        let second = try #require(secondPane.content.selectedShell)
        #expect(firstPane.id == secondPane.id)
        #expect(first.id != second.id)
        #expect(store.layout.allPanes.count == 3)
        store.selectShell(first.id)
        #expect(store.shell(second.id) == second)
        #expect(store.shellPane?.incarnation == firstPane.incarnation)
        let filesPane = try #require(store.showBrowser(.files, location: location))
        let files = TaskFilesStore(issue: work.id, cwd: location.worktree)
        files.autosave = false
        files.selectFile("first.txt", mode: .file)
        let document = files.document("first.txt")
        await document.reconcile(TaskFileSnapshot(recoveries: [], issueIdentifier: work.id, taskId: work.id,
            path: "first.txt", content: "base", state: .text, revision: "base-revision", readOnlyReason: nil, sizeBytes: 4))
        document.editor.string = "unsaved draft"
        files.selectFile("second.txt", mode: .file)
        let tabs = files.tabs
        files.selectFile("first.txt", mode: .file)
        #expect(files.tabs == tabs)
        #expect(files.document("first.txt") === document)
        #expect(document.text == "unsaved draft")
        let portableLocation = WorkViewPlan.Location(machineID: location.machineId, cwd: location.worktree)
        let saved = store.paneLayout(work: work, revision: 2,
            files: files.browserTab, drafts: files.drafts(location: portableLocation))
        let decoded = try JSONDecoder().decode(WorkPaneLayout.self, from: JSONEncoder().encode(saved))
        let restored = MultiplexerStore(taskId: work.id)
        restored.restore(decoded)
        #expect(restored.layout.allPanes.map(\.id) == store.layout.allPanes.map(\.id))
        #expect(restored.browserPane?.id == filesPane.id)
        #expect(restored.shellPane?.content.selectedShell?.id == first.id)
        #expect(restored.shell(second.id)?.location == location)
        #expect(restored.shell(first.id)?.command == nil)
        #expect(restored.shell(first.id)?.incarnation != first.incarnation)
        let reopenedFiles = TaskFilesStore(issue: work.id, cwd: location.worktree)
        reopenedFiles.autosave = false
        guard case .split(_, _, _, .split(_, _, .pane(_, .browser(_, let browserTabs, _)), _)) = decoded.tree,
              case .files(_, let retained, let selected)? = browserTabs.first else {
            Issue.record("Missing Files tab"); return
        }
        reopenedFiles.restoreTabs(retained, selected: selected)
        reopenedFiles.restoreDrafts(decoded.drafts, location: portableLocation)
        #expect(reopenedFiles.tabs == tabs)
        #expect(reopenedFiles.selection == "first.txt")
        #expect(reopenedFiles.document("first.txt").text == "unsaved draft")
        #expect(!reopenedFiles.document("first.txt").canSave)
        await reopenedFiles.document("first.txt").reconcile(TaskFileSnapshot(recoveries: [], issueIdentifier: work.id,
            taskId: work.id, path: "first.txt", content: "base", state: .text, revision: "base-revision", readOnlyReason: nil, sizeBytes: 4))
        #expect(reopenedFiles.document("first.txt").text == "unsaved draft")
        #expect(reopenedFiles.document("first.txt").canSave)
    }

    @Test func emptyFilesRetainTheirRecordedLocation() throws {
        let store = MultiplexerStore(taskId: work.id)
        let pane = try #require(store.showBrowser(.files, location: location))
        let saved = store.paneLayout(work: work, revision: 0, files: nil, drafts: [])
        let restored = MultiplexerStore(taskId: work.id)
        restored.restore(saved)
        #expect(restored.layout.pane(for: pane.id)?.content.location == location)
    }

    @Test func creationReceiptsAndBackgroundTargetsKeepTheirShell() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        let workspace = registry.open(work, title: "Task", location: location)
        workspace.multiplexer.load(sessionId: "primary", location: location)
        let session = workspace.multiplexer.focusedPane
        let source = DesktopPaneTarget(repository: work.repository, window: "window", work: work,
            pane: session.id, incarnation: session.incarnation, tab: nil)
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("No execution during control") })
        let first = try DesktopAction.perform(DesktopActionRequest("shell.new", at: source), model: model, workspaces: registry)
        let firstID = try #require(first.tab)
        let shell = try #require(workspace.multiplexer.shell(firstID))
        let exact = DesktopPaneTarget(repository: work.repository, window: "window", work: work,
            pane: try #require(first.pane), incarnation: shell.incarnation, tab: shell.id)
        let second = try DesktopAction.perform(DesktopActionRequest("shell.new", at: source), model: model, workspaces: registry)
        #expect(first.pane == second.pane)
        #expect(first.tab != second.tab)
        #expect(workspace.multiplexer.shellPane?.content.selectedShell?.id == second.tab)
        let request = DesktopTextRequest(target: exact, surface: "missing", region: .screen, maxBytes: 1024)
        let reading = try registry.readText(request)
        #expect(reading.result == .unavailable(reason: .missingSurface))
        #expect(reading.hidden)
        #expect(workspace.multiplexer.shellPane?.content.selectedShell?.id == second.tab)
        let tool = try #require(workspace.multiplexer.shellPane)
        let toolTarget = DesktopPaneTarget(repository: work.repository, window: "window", work: work,
            pane: tool.id, incarnation: tool.incarnation, tab: nil)
        let toolRead = DesktopTextRequest(target: toolTarget, surface: "missing", region: .screen, maxBytes: 1024)
        #expect(try registry.readText(toolRead).result == .unavailable(reason: .notTerminal))
        #expect(throws: RegistryQueryError.self) {
            try DesktopAction.perform(DesktopActionRequest("pane.send", at: toolTarget, args: ["text": "draft", "surface": "missing"]), model: model, workspaces: registry)
        }
        let hidden = try DesktopAction.perform(DesktopActionRequest("pane.hide", at: exact), model: model, workspaces: registry)
        #expect(hidden.tab == firstID)
        #expect(try registry.readText(request).hidden)
        _ = try DesktopAction.perform(DesktopActionRequest("pane.restore", at: exact), model: model, workspaces: registry)
        workspace.multiplexer.selectShell(shell.id)
        #expect(!(try registry.readText(request)).hidden)
        workspace.closePane(try #require(first.pane))
        #expect(workspace.multiplexer.shell(try #require(second.tab)) != nil)
        #expect(throws: DesktopTargetError.self) { try registry.readText(request) }
    }

    @Test func higherLevelShellRequestsMutateNothing() throws {
        for kind in [WorkWorkspaceKey.Kind.repository, .wave] {
            let workspace = SessionsWorkspace(key: WorkWorkspaceKey(repository: "repo", kind: kind, id: "owner"),
                title: "Owner", location: location, localMachineId: "local")
            workspace.multiplexer.load(sessionId: "retained", location: location)
            let before = workspace.multiplexer.layout
            #expect(throws: RegistryQueryError.self) { try workspace.newShell() }
            #expect(workspace.multiplexer.layout == before)
        }
    }
}
#endif
