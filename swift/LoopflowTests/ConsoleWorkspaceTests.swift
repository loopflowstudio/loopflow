#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Console retained tools", .serialized)
@MainActor
struct ConsoleWorkspaceTests {
    @Test func togglesRetainShellTabsAndFileDrafts() throws {
        let workspace = fixtureWork(fixtureWorkspace("/task"))
        let store = workspace.multiplexer
        store.load(sessionId: "conversation", location: workspace.location)
        let conversation = store.focusedPane
        try workspace.newShell()
        let first = try #require(store.focusedPane.content.selectedShell)
        try workspace.newShell()
        let second = try #require(store.focusedPane.content.selectedShell)
        let shell = store.focusedPane
        #expect(first.id != second.id)
        #expect(store.layout.allPanes.count == 3)
        workspace.toggleBrowser()
        let files = workspace.files(issue: "TASK", cwd: "/task")
        files.selection = "a.txt"
        let document = files.document("a.txt")
        document.editor.string = "unfinished"
        document.editor.setSelectedRange(NSRange(location: 2, length: 3))
        files.selection = "b.txt"
        let layout = store.layout
        for _ in 0..<2 {
            try workspace.toggleShell()
            workspace.toggleBrowser()
        }
        #expect(store.layout == layout)
        #expect(store.collapsedPaneIds.isEmpty)
        #expect(store.layout.pane(for: conversation.id) == conversation)
        #expect(store.layout.pane(for: shell.id)?.content.selectedShell == second)
        store.selectShell(first.id)
        #expect(store.focusedPane.content.selectedShell == first)
        #expect(workspace.files(issue: "TASK", cwd: "/task") === files)
        #expect(files.tabs.map(\.path) == ["a.txt", "b.txt"])
        #expect(document.editor.string == "unfinished")
        #expect(document.editor.selectedRange() == NSRange(location: 2, length: 3))
    }

    @Test(arguments: [WorkWorkspaceKey.Kind.repository, .wave])
    func scopeNavigationCannotAllocateTools(kind: WorkWorkspaceKey.Kind) throws {
        let workspace = SessionsWorkspace(key: .init(repository: "repo", kind: kind, id: "scope"),
            title: "Scope", location: fixtureWorkspace("/repo"), localMachineId: fixtureMachineId)
        let before = workspace.multiplexer.layout
        workspace.toggleBrowser()
        try workspace.toggleShell()
        #expect(throws: RegistryQueryError.self) { try workspace.newShell() }
        #expect(workspace.multiplexer.layout == before)
    }
}
#endif
