import Loopflow
import Testing

@Suite("Multiplexer store")
@MainActor
struct MultiplexerStoreTests {
    @Test("Reopening a hidden companion reveals its retained occurrence", arguments: [
        PaneContent.files(taskId: "task"), .flowLog(taskId: "task"),
    ])
    func reopeningHiddenCompanion(content: PaneContent) {
        let store = MultiplexerStore()
        store.show(content)
        let pane = store.focusedPane
        store.setCollapsed(paneId: pane.id, collapsed: true)
        #expect(store.visibleLayout == nil)
        store.show(content)
        #expect(store.visibleLayout == .leaf(pane))
        #expect(store.focusedPane == pane)
        store.show(content)
        #expect(store.layout == .leaf(pane))
    }

    @Test("Explicit zoom preserves selection; focus reveals that same selected pane")
    func explicitZoomPreservesSelection() {
        let store = MultiplexerStore()
        store.load(sessionId: "selected")
        let selected = store.focusedPane
        store.reveal(sessionId: "other")
        let other = store.focusedPane
        store.setFocusedPane(selected.id)
        store.setZoom(other.id, enabled: true)
        store.setZoom(other.id, enabled: true)
        #expect(store.focusedPane == selected)
        #expect(store.zoomedPaneId == other.id)
        store.setZoom(selected.id, enabled: false)
        #expect(store.zoomedPaneId == other.id)
        store.setFocusedPane(selected.id)
        #expect(store.zoomedPaneId == selected.id)
        #expect(store.focusedPane == selected)
        store.setZoom(selected.id, enabled: false)
        #expect(store.zoomedPaneId == nil)
        #expect(store.focusedPane == selected)
    }

    @Test("Opening in the active pane preserves split geometry and other visible Sessions")
    func activePaneSelectionPreservesSplits() throws {
        let store = MultiplexerStore()
        store.load(sessionId: "design")
        let left = store.focusedPaneId
        store.reveal(sessionId: "review")
        let right = store.focusedPaneId
        store.updateRatio(between: left, and: right, ratio: 0.35)
        let arrangement = store.layout
        store.load(sessionId: "ask")
        #expect(store.layout.allPanes.map(\.id) == arrangement.allPanes.map(\.id))
        #expect(store.layout.pane(for: left) == arrangement.pane(for: left))
        #expect(store.layout.pane(for: right)?.content == .session(id: "ask"))
        #expect(store.layout.pane(for: right)?.incarnation != arrangement.pane(for: right)?.incarnation)
        store.load(sessionId: "design")
        #expect(store.focusedPaneId == left)
        #expect(store.visibleLayout?.allPanes.count == 2)
        store.setCollapsed(paneId: right, collapsed: true)
        store.load(sessionId: "ask")
        #expect(store.focusedPaneId == left)
        #expect(store.focusedPane.content == .session(id: "ask"))
        #expect(store.visibleLayout?.allPanes.count == 1)
        #expect(store.layout.pane(for: right)?.content == .session(id: "design"))
        store.reveal(sessionId: "design")
        #expect(store.layout.allPanes.map(\.id) == arrangement.allPanes.map(\.id))
        #expect(store.layout.allPanes.map(\.content) == [.session(id: "ask"), .session(id: "design")])
        guard case .split(.vertical, _, _, let ratio) = store.layout else {
            Issue.record("Opening a Session must preserve the split")
            return
        }
        #expect(ratio == 0.35)
        #expect(store.visibleLayout == store.layout)
        store.setCollapsed(paneId: left, collapsed: true)
        store.setCollapsed(paneId: right, collapsed: true)
        store.load(sessionId: "ask")
        #expect(store.visibleLayout?.allPanes.map(\.content) == [.session(id: "ask")])
    }

    @Test("Opening a Session preserves a running shell and restores a hidden final pane")
    func selectingBesideShell() {
        let store = MultiplexerStore()
        store.newShell(command: ["server"])
        let shell = store.focusedPaneId
        store.load(sessionId: "design")
        #expect(store.layout.pane(for: shell)?.content == .shell)
        #expect(store.shellCommands[shell] == ["server"])
        store.setCollapsed(paneId: shell, collapsed: true)
        store.setCollapsed(paneId: store.focusedPaneId, collapsed: true)
        store.load(sessionId: "design")
        #expect(store.visibleLayout?.allPanes.map(\.content) == [.session(id: "design")])
    }

    @Test("Focus restores the prior selection and collapsed arrangement")
    func focusCollapsedPaneAndRestore() {
        let store = MultiplexerStore()
        store.reveal(sessionId: "design")
        let design = store.focusedPaneId
        store.reveal(sessionId: "ask")
        let ask = store.focusedPaneId
        store.setCollapsed(paneId: design, collapsed: true)
        let original = store.layout
        store.toggleZoom(design)
        #expect(store.zoomedPaneId == design)
        #expect(store.layout.pane(for: design)?.content == .session(id: "design"))
        store.toggleZoom(design)
        #expect(store.focusedPaneId == ask)
        #expect(store.collapsedPaneIds == [design])
        #expect(store.layout == original)
        #expect(store.visibleLayout?.allPanes.map(\.id) == [ask])
    }

    @Test("Collapse preserves the layout, shell command and ratios; reveal restores the same pane")
    func collapseRetainsPane() throws {
        let store = MultiplexerStore()
        store.newShell(command: ["server"])
        let shell = store.focusedPaneId
        store.reveal(sessionId: "ask")
        let ask = store.focusedPaneId
        store.updateRatio(between: shell, and: ask, ratio: 0.37)
        let original = store.layout
        store.setCollapsed(paneId: shell, collapsed: true)
        #expect(store.layout == original)
        #expect(store.visibleLayout?.allPanes.map(\.id) == [ask])
        #expect(store.shellCommands[shell] == ["server"])
        store.setCollapsed(paneId: ask, collapsed: true)
        #expect(store.visibleLayout == nil)
        store.reveal(sessionId: "ask")
        #expect(store.focusedPaneId == ask)
        store.setCollapsed(paneId: shell, collapsed: false)
        #expect(store.visibleLayout == original)
        #expect(store.shellCommands[shell] == ["server"])
    }

    @Test("starts with one focused empty pane")
    func startsWithOnePane() {
        let store = MultiplexerStore()
        #expect(store.layout.allPanes.count == 1)
        #expect(store.focusedPane.content == .empty)
    }

    @Test("split adds and focuses a distinct pane")
    func splitAddsAndFocuses() throws {
        let store = MultiplexerStore()
        let first = store.focusedPaneId
        let pane = try #require(store.split(first, axis: .vertical))
        #expect(store.layout.allPanes.count == 2)
        #expect(store.focusedPaneId == pane.id)
        #expect(pane.content == .empty)
    }

    @Test("loading an open session jumps to its pane without duplicating it")
    func loadJumpsToOpenSession() throws {
        let store = MultiplexerStore()
        let first = store.focusedPaneId
        store.load(sessionId: "session-1")
        let second = try #require(store.split(first, axis: .vertical))

        store.load(sessionId: "session-1")

        #expect(store.focusedPaneId == first)
        #expect(store.layout.allPanes.count == 2)
        #expect(store.layout.allPanes.count {
            $0.content == .session(id: "session-1")
        } == 1)
        #expect(first != second.id)
    }

    @Test("loading from an occupied pane replaces its session")
    func loadReplacesOccupiedPane() {
        let store = MultiplexerStore()
        store.load(sessionId: "session-1")

        store.load(sessionId: "ask-2")

        #expect(store.layout.allPanes.map(\.content) == [.session(id: "ask-2")])
        #expect(store.focusedPane.content == .session(id: "ask-2"))
    }

    @Test("an explicit split keeps the first session when another is selected")
    func explicitSplitKeepsBothSessions() throws {
        let store = MultiplexerStore()
        store.load(sessionId: "session-1")
        _ = try #require(store.split(store.focusedPaneId, axis: .vertical))

        store.load(sessionId: "ask-2")

        #expect(store.layout.allPanes.map(\.content) == [
            .session(id: "session-1"),
            .session(id: "ask-2"),
        ])
    }

    @Test("new shell uses the empty pane then splits an occupied pane")
    func newShellUsesAvailableSpace() {
        let store = MultiplexerStore()
        store.newShell()
        #expect(store.focusedPane.content == .shell)
        #expect(store.layout.allPanes.count == 1)

        store.newShell()
        #expect(store.focusedPane.content == .shell)
        #expect(store.layout.allPanes.count == 2)
    }

    @Test("opening a Session keeps the shell pane alive")
    func loadingSessionPreservesShell() {
        let store = MultiplexerStore()
        store.newShell()

        store.load(sessionId: "session-1")

        #expect(store.layout.allPanes.map(\.content) == [
            .shell,
            .session(id: "session-1"),
        ])
    }

    @Test("close collapses the split and undo restores it")
    func closeUndo() throws {
        let store = MultiplexerStore()
        let first = store.focusedPane
        let second = try #require(store.split(first.id, axis: .vertical))

        store.close(second.id)
        #expect(store.layout.allPanes == [first])
        #expect(store.focusedPaneId == first.id)

        store.undoClose()
        #expect(store.layout.allPanes.map(\.id) == [first.id, second.id])
        #expect(store.layout.pane(for: first.id) == first)
        #expect(store.focusedPane.incarnation != second.incarnation)
        #expect(store.focusedPaneId == second.id)
        let restored = store.layout
        store.undoClose()
        #expect(store.layout == restored)
    }

    @Test("close removes the left pane as well as the right")
    func closeLeftPane() throws {
        let store = MultiplexerStore()
        let first = store.focusedPaneId
        let second = try #require(store.split(first, axis: .vertical))

        store.close(first)

        #expect(store.layout.allPanes.map(\.id) == [second.id])
        #expect(store.focusedPaneId == second.id)
    }

    @Test("close clears the final terminal and undo restores it")
    func closeClearsLastTerminal() {
        let store = MultiplexerStore()
        store.load(sessionId: "session-1")
        let original = store.focusedPane
        store.close(original.id)

        #expect(store.layout.allPanes.count == 1)
        #expect(store.focusedPane.content == .empty)

        store.undoClose()
        #expect(store.focusedPane.content == .session(id: "session-1"))
        #expect(store.focusedPane.id == original.id)
        #expect(store.focusedPane.incarnation != original.incarnation)
    }

    @Test("close leaves the final empty pane alone")
    func closeKeepsLastEmptyPane() {
        let store = MultiplexerStore()
        store.close(store.focusedPaneId)

        #expect(store.layout.allPanes.count == 1)
        #expect(store.focusedPane.content == .empty)
    }

    @Test("zoom keeps the tree intact and toggles off")
    func zoomToggle() throws {
        let store = MultiplexerStore()
        let first = store.focusedPaneId
        _ = try #require(store.split(first, axis: .vertical))

        store.toggleZoom(first)
        #expect(store.zoomedPaneId == first)
        #expect(store.layout.allPanes.count == 2)

        store.toggleZoom(first)
        #expect(store.zoomedPaneId == nil)
    }

    @Test("completed sessions leave an empty workspace")
    func completedSessionClearsPane() {
        let store = MultiplexerStore()
        store.load(sessionId: "session-1")

        store.removeSessions(["session-1"])

        #expect(store.layout.allPanes.count == 1)
        #expect(store.focusedPane.content == .empty)
        #expect(store.pane(forSessionId: "session-1") == nil)
    }

    @Test("Undo restores a hidden Session only while shared evidence retains it", arguments: [false, true])
    func hiddenSessionReconciliation(retained: Bool) {
        let store = MultiplexerStore()
        store.newShell()
        let shellPane = store.focusedPaneId
        store.load(sessionId: "session-1")
        store.close(store.focusedPaneId)

        store.removeSessions(retained ? [] : ["session-1"])

        #expect(store.canUndoClose == retained)
        store.undoClose()
        #expect((store.pane(forSessionId: "session-1") != nil) == retained)
        #expect(store.layout.pane(for: shellPane)?.content == .shell)
        if !retained {
            #expect(store.layout.allPanes.map(\.id) == [shellPane])
            #expect(store.focusedPaneId == shellPane)
        }
    }

    @Test("focus left follows visual geometry instead of tree order")
    func focusLeftIsSpatial() {
        let left = PaneState(id: "left", content: .shell)
        let upperRight = PaneState(id: "upper-right", content: .shell)
        let lowerRight = PaneState(id: "lower-right", content: .shell)
        let right = LayoutNode.split(
            .horizontal,
            first: .leaf(upperRight),
            second: .leaf(lowerRight),
            ratio: 0.5
        )
        let layout = LayoutNode.split(
            .vertical,
            first: .leaf(left),
            second: right,
            ratio: 0.3
        )
        let store = MultiplexerStore(layout: layout)
        store.setFocusedPane(lowerRight.id)

        store.focus(.left)

        #expect(store.focusedPaneId == left.id)
    }
}
