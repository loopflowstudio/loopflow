import Foundation
import Observation
import Loopflow
import Testing

@Suite("Multiplexer store")
@MainActor
struct MultiplexerStoreTests {
    private let here = WorkspaceIdentity(machineId: "local", worktree: "/task")

    @Test("A Task workspace holds its three designated panes; tools start hidden")
    func designatedPanes() {
        let task = MultiplexerStore(taskId: "task_1")
        #expect(task.layout.allPanes.map(\.kind) == [.session, .browser, .shell])
        #expect(task.visibleLayout?.allPanes.map(\.id) == ["session"])
        #expect(task.focusedPaneId == "session")
        let wave = MultiplexerStore(taskId: nil)
        #expect(wave.layout.allPanes.map(\.kind) == [.session])
        #expect(!wave.canClose)
    }

    @Test("Opening a conversation switches the session pane in place")
    func sessionSwitchesInPlace() throws {
        let store = MultiplexerStore(taskId: "task_1")
        store.newShell(location: here)
        let before = store.layout
        store.load(sessionId: "first", location: here)
        store.load(sessionId: "second", location: here)
        #expect(store.layout.allPanes.count == 3)
        #expect(store.sessionPane?.sessionId == "second")
        #expect(store.focusedPaneId == "session")
        #expect(store.layout.pane(for: "shell") == before.pane(for: "shell"))
        let pane = try #require(store.sessionPane)
        store.reveal(sessionId: "second", location: here)
        #expect(store.sessionPane?.incarnation == pane.incarnation)
        store.removeSessions(["second"])
        #expect(store.sessionPane?.content == .session(id: nil))
    }

    @Test("Shell tabs accumulate in one pane; the last one closing hides it")
    func shellTabs() throws {
        let store = MultiplexerStore(taskId: "task_1")
        let first = try #require(store.newShell(location: here, focus: false))
        #expect(store.focusedPaneId == "session")
        #expect(store.isVisible(first))
        let second = try #require(store.newShell(location: here))
        #expect(second.id == first.id)
        #expect(store.focusedPaneId == "shell")
        guard case .shell(let tabs, let selected) = second.content else { throw TestFailure() }
        #expect(tabs.count == 2)
        #expect(selected == tabs[1].id)
        store.closeShell(tabs[1].id)
        #expect(store.shellPane?.content.selectedShell?.id == tabs[0].id)
        store.closeShell(tabs[0].id)
        #expect(store.collapsedPaneIds.contains("shell"))
        #expect(store.layout.allPanes.count == 3)
    }

    @Test("Browser tabs open once; tab changes keep the occurrence and missing evidence keeps the owner")
    func browserTabs() throws {
        let store = MultiplexerStore(taskId: "task_1")
        let files = try #require(store.showBrowser(.files, location: here, focus: false))
        #expect(store.focusedPaneId == "session")
        let page = try #require(BrowserTab.page(URL(string: "http://localhost:5173")!))
        let web = try #require(store.showBrowser(page))
        #expect(web.incarnation == files.incarnation)
        #expect(web.content.location == here)
        let again = try #require(store.showBrowser(.files))
        guard case .browser(_, let tabs, let selected, _, _) = again.content else { throw TestFailure() }
        #expect(tabs.map(\.id) == ["files", page.id])
        #expect(selected == "files")
        let moved = WorkspaceIdentity(machineId: "local", worktree: "/moved")
        #expect(store.showBrowser(.files, location: moved)?.incarnation != files.incarnation)
        #expect(BrowserTab.page(URL(string: "file:///etc/hosts")!) == nil)
    }

    @Test("Close hides a pane and Undo shows it again; the last visible pane stays")
    func closeHides() throws {
        let store = MultiplexerStore(taskId: "task_1")
        store.load(sessionId: "conversation", location: here)
        store.newShell(location: here)
        store.close("shell")
        #expect(store.collapsedPaneIds.contains("shell"))
        #expect(store.focusedPaneId == "session")
        #expect(store.shellPane?.content.selectedShell != nil)
        store.undoClose()
        #expect(!store.collapsedPaneIds.contains("shell"))
        #expect(store.focusedPaneId == "shell")
        store.close("shell")
        #expect(!store.canClose)
        store.close("session")
        #expect(store.isVisible(try #require(store.sessionPane)))
    }

    @Test("Moving, zooming and resizing rearrange the same three panes")
    func arrangement() {
        let store = MultiplexerStore(taskId: "task_1")
        store.setCollapsed(paneId: "browser", collapsed: false)
        store.move("browser", beside: "session", axis: .horizontal)
        #expect(Set(store.layout.allPanes.map(\.id)) == ["session", "shell", "browser"])
        store.setZoom("browser", enabled: true)
        #expect(store.zoomedPaneId == "browser")
        store.setZoom("browser", enabled: false)
        store.updateRatio(between: "session", and: "browser", ratio: 0.3)
        #expect(store.layout.allPanes.count == 3)
    }

    @Test("A portable layout restores the same panes, tabs and visibility")
    func portableRoundTrip() throws {
        let work = WorkWorkspaceKey(repository: "repo_1", kind: .task, id: "task_1")
        let store = MultiplexerStore(taskId: work.id)
        store.load(sessionId: "conversation", location: here)
        store.newShell(location: here, focus: false)
        store.showBrowser(.graphLog, location: here, focus: false)
        store.close("shell")
        let saved = store.paneLayout(work: work, revision: 3, files: nil, drafts: [])
        let restored = MultiplexerStore(taskId: work.id)
        restored.restore(saved)
        #expect(restored.paneLayout(work: work, revision: 3, files: nil, drafts: []) == saved)
        #expect(restored.collapsedPaneIds == ["shell"])
        #expect(restored.browserPane?.content.selectedBrowserTab == .graphLog)
    }

    @Test("Focus and close follow split geometry instead of tree order")
    func spatialFocusAndClose() {
        let store = MultiplexerStore(taskId: "task_1")
        store.setCollapsed(paneId: "browser", collapsed: false)
        store.setCollapsed(paneId: "shell", collapsed: false)
        store.setFocusedPane("shell")
        store.focus(.left)
        #expect(store.focusedPaneId == "session")
        store.setFocusedPane("browser")
        store.focus(.down)
        #expect(store.focusedPaneId == "shell")
        store.focus(.up)
        #expect(store.focusedPaneId == "browser")
        store.setCollapsed(paneId: "browser", collapsed: true)
        store.setFocusedPane("session")
        store.focus(.right)
        #expect(store.focusedPaneId == "shell")
        store.close("shell")
        #expect(store.focusedPaneId == "session")
    }
}

private struct TestFailure: Error {}

@Test @MainActor
func canonical_ids_reopening_session_preserves_pane_and_focus() throws {
    let upper = "SESSION_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    let store = MultiplexerStore(taskId: nil)
    store.load(sessionId: upper)
    let pane = store.focusedPane
    store.reveal(sessionId: upper.lowercased())
    #expect(store.layout.allPanes.count == 1)
    #expect(store.focusedPane.id == pane.id)
    #expect(store.focusedPane.incarnation == pane.incarnation)
    #expect(store.focusedPane.sessionId == upper.lowercased())
    let encoded = try JSONEncoder().encode(store.layout)
    let restored = try JSONDecoder().decode(LayoutNode.self, from: encoded)
    #expect(restored == store.layout)
}

@Test
func canonical_ids_workspace_keys_decode_canonically_without_changing_paths() throws {
    let raw = Data(#"{"machine_id":"HOME_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","worktree":"/Case/Repo"}"#.utf8)
    let workspace = try JSONDecoder().decode(WorkspaceIdentity.self, from: raw)
    #expect(workspace.machineId == "home_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    #expect(workspace.worktree == "/Case/Repo")
    let upper = WorkWorkspaceKey(repository: "REPO_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", kind: .task, id: "TASK_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB")
    let lower = WorkWorkspaceKey(repository: "repo_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", kind: .task, id: "task_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
    #expect(upper == lower)
    #expect([upper: "retained"][lower] == "retained")
    #expect(AgentSessionId(rawValue: "Case-Sensitive") != AgentSessionId(rawValue: "case-sensitive"))
}
