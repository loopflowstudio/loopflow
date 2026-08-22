import Loopflow
import Testing

@Suite("Multiplexer store")
@MainActor
struct MultiplexerStoreTests {
    @Test("starts with one focused, colored empty pane")
    func startsWithOnePane() {
        let store = MultiplexerStore()
        #expect(store.layout.allPanes.count == 1)
        #expect(store.focusedPane.content == .empty)
        #expect(store.color(for: store.focusedPaneId) == .blue)
    }

    @Test("split adds and focuses a distinct pane")
    func splitAddsAndFocuses() throws {
        let store = MultiplexerStore()
        let first = store.focusedPaneId
        let pane = try #require(store.split(first, axis: .vertical))
        #expect(store.layout.allPanes.count == 2)
        #expect(store.focusedPaneId == pane.id)
        #expect(store.color(for: pane.id) != store.color(for: first))
    }

    @Test("loading an open session jumps to its pane without duplicating it")
    func loadJumpsToOpenSession() throws {
        let store = MultiplexerStore()
        let first = store.focusedPaneId
        store.load(sessionId: "ask-1")
        let second = try #require(store.split(first, axis: .vertical))

        store.load(sessionId: "ask-1")

        #expect(store.focusedPaneId == first)
        #expect(store.layout.allPanes.count == 2)
        #expect(store.layout.allPanes.count {
            $0.content == .session(id: "ask-1")
        } == 1)
        #expect(first != second.id)
    }

    @Test("loading from an occupied pane creates a second pane")
    func loadSplitsOccupiedPane() {
        let store = MultiplexerStore()
        store.load(sessionId: "ask-1")

        store.load(sessionId: "ask-2")

        #expect(store.layout.allPanes.map(\.content) == [
            .session(id: "ask-1"),
            .session(id: "ask-2"),
        ])
        #expect(store.focusedPane.content == .session(id: "ask-2"))
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

    @Test("close collapses the split and undo restores it")
    func closeUndo() throws {
        let store = MultiplexerStore()
        let first = store.focusedPaneId
        let second = try #require(store.split(first, axis: .vertical))

        store.close(second.id)
        #expect(store.layout.allPanes.map(\.id) == [first])
        #expect(store.focusedPaneId == first)

        store.undoClose()
        #expect(store.layout.allPanes.map(\.id) == [first, second.id])
        #expect(store.focusedPaneId == second.id)
    }

    @Test("close never removes the last pane")
    func closeKeepsLastPane() {
        let store = MultiplexerStore()
        store.close(store.focusedPaneId)
        #expect(store.layout.allPanes.count == 1)
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

    @Test("settled sessions leave no stale session pane")
    func settledSessionCollapses() {
        let store = MultiplexerStore()
        store.load(sessionId: "ask-1")

        store.reconcileSessions([])

        #expect(store.layout.allPanes.count == 1)
        #expect(store.focusedPane.content == .shell)
        #expect(store.pane(forSessionId: "ask-1") == nil)
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
