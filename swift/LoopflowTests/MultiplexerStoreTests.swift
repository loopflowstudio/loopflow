import Foundation
import Observation
import Loopflow
import Testing

@Suite("Multiplexer store")
@MainActor
struct MultiplexerStoreTests {
    @Test("Files retain location through hide and Undo; replacement invalidates exact targets")
    func fileLocationSurvivesLayoutChanges() throws {
        let store = MultiplexerStore()
        let first = WorkspaceIdentity(machineId: "local", worktree: "/first")
        let second = WorkspaceIdentity(machineId: "local", worktree: "/second")
        let content = PaneContent.files(taskId: "task", location: first)
        let files = try #require(store.show(content))
        let shell = try #require(store.newShell(location: first))
        store.setCollapsed(paneId: files.id, collapsed: true)
        store.setZoom(shell.id, enabled: true)
        store.close(files.id)
        store.undoClose()
        let restored = try #require(store.layout.pane(for: files.id))
        #expect(restored.content == content)
        #expect(restored.incarnation != files.incarnation)
        #expect(store.show(.files(taskId: "task"), focus: false)?.content == content)
        let replacement = try #require(store.show(.files(taskId: "task", location: second), focus: false))
        #expect(replacement.id == files.id)
        #expect(replacement.incarnation != restored.incarnation)
        #expect(replacement.content.location == second)
        #expect(store.focusedPaneId == shell.id)
        #expect(store.zoomedPaneId == shell.id)
        let decoded = try JSONDecoder().decode(LayoutNode.self, from: JSONEncoder().encode(store.layout))
        #expect(decoded == store.layout)
    }

    @Test("Session location travels through layout and Undo; a new reading fences old targets")
    func sessionLocationSurvivesLayoutChanges() throws {
        let store = MultiplexerStore()
        let local = WorkspaceIdentity(machineId: "local", worktree: "/same/path")
        let peer = WorkspaceIdentity(machineId: "peer", worktree: "/same/path")
        store.load(sessionId: "conversation", location: peer)
        let session = store.focusedPane
        let shell = try #require(store.newShell(location: local))
        store.move(session.id, beside: shell.id, axis: .horizontal)
        store.setCollapsed(paneId: session.id, collapsed: true)
        store.setZoom(shell.id, enabled: true)
        #expect(store.layout.pane(for: session.id) == session)
        store.close(session.id)
        store.updateSessionLocation("conversation", location: local)
        store.undoClose()
        let restored = try #require(store.pane(forSessionId: "conversation"))
        #expect(restored.content.location == local)
        #expect(restored.incarnation != session.incarnation)
        #expect(store.focusedPaneId == shell.id)
        #expect(store.zoomedPaneId == shell.id)
        #expect(store.collapsedPaneIds.contains(session.id))
        store.updateSessionLocation("conversation", location: local)
        #expect(store.pane(forSessionId: "conversation") == restored)
        store.updateSessionLocation("conversation", location: nil)
        #expect(store.pane(forSessionId: "conversation")?.content.location == nil)
        #expect(store.pane(forSessionId: "conversation")?.incarnation != restored.incarnation)
        let decoded = try JSONDecoder().decode(LayoutNode.self, from: JSONEncoder().encode(store.layout))
        #expect(decoded == store.layout)
    }

    @Test("Shell location survives arrangement and Undo without replaying its launch")
    func shellLocationSurvivesLayoutChanges() throws {
        let store = MultiplexerStore()
        let local = WorkspaceIdentity(machineId: "local", worktree: "/same/path")
        let peer = WorkspaceIdentity(machineId: "peer", worktree: "/same/path")
        let first = try #require(store.newShell(command: ["local-command"], location: local))
        let second = try #require(store.split(first.id, axis: .vertical,
            command: ["remote-command"], location: peer))
        store.move(first.id, beside: second.id, axis: .horizontal)
        store.setCollapsed(paneId: second.id, collapsed: true)
        store.setZoom(first.id, enabled: true)
        #expect(store.layout.pane(for: first.id) == first)
        #expect(store.layout.pane(for: second.id) == second)
        let decoded = try JSONDecoder().decode(LayoutNode.self, from: JSONEncoder().encode(store.layout))
        #expect(decoded == store.layout)
        store.close(second.id)
        store.undoClose()
        let restored = try #require(store.layout.pane(for: second.id))
        #expect(restored.content.location == peer)
        #expect(restored.incarnation != second.incarnation)
        #expect(store.shellCommands[second.id] == nil)
        #expect(store.layout.pane(for: first.id)?.content.location == local)
    }

    @Test("Visibility follows retained occurrences, collapse and zoom, not focus")
    func exactPaneVisibility() throws {
        let store = MultiplexerStore()
        store.load(sessionId: "retained")
        let retained = store.focusedPane
        let other = try #require(store.split(retained.id, axis: .vertical))
        #expect(store.isVisible(retained))
        #expect(store.isVisible(other))
        store.setZoom(other.id, enabled: true)
        #expect(!store.isVisible(retained))
        #expect(store.isVisible(other))
        store.setZoom(other.id, enabled: false)
        store.setCollapsed(paneId: retained.id, collapsed: true)
        #expect(!store.isVisible(retained))
        store.setCollapsed(paneId: retained.id, collapsed: false)
        #expect(store.isVisible(retained))
        store.close(retained.id)
        #expect(!store.isVisible(retained))
        store.undoClose()
        #expect(!store.isVisible(retained))
        #expect(store.isVisible(try #require(store.layout.pane(for: retained.id))))
        #expect(store.isVisible(other))
    }

    @Test("Observed shell panes carry their launch command")
    func shellLaunchIsPublishedTogether() async throws {
        let store = MultiplexerStore()
        await confirmation("layout changes are observable") { changed in
            withObservationTracking {
                _ = store.layout
            } onChange: { changed() }
            store.newShell(command: ["fixture"])
        }
        store.newShell(command: ["fixture"])
        #expect(store.layout.allPanes.count == 2)
        for pane in store.layout.allPanes {
            #expect(pane.content == .shell(location: nil))
            #expect(store.shellCommands[pane.id] == ["fixture"])
        }
    }

    @Test("Companion insertion clears Undo and preserves the requested focus", arguments: [false, true], [
        PaneContent.shell(location: nil), .files(taskId: "task"), .flowLog(taskId: "task"),
    ])
    func emptyCompanionPublishesCompleteState(focus: Bool, content: PaneContent) throws {
        let store = MultiplexerStore()
        let empty = store.focusedPane
        store.split(empty.id, axis: .vertical)
        store.load(sessionId: "selected")
        let selected = store.focusedPane
        let temporary = try #require(store.split(selected.id, axis: .horizontal))
        store.close(temporary.id)
        store.setFocusedPane(selected.id)
        store.setZoom(selected.id, enabled: true)
        store.setCollapsed(paneId: empty.id, collapsed: true)
        #expect(store.canUndoClose)

        if content == .shell(location: nil) { store.newShell(command: ["fixture"], beside: empty.id, focus: focus) }
        else { store.show(content, beside: empty.id, focus: focus) }
        #expect(!store.canUndoClose)
        #expect(store.layout.pane(for: empty.id)?.content == content)
        #expect(store.layout.pane(for: empty.id)?.incarnation != empty.incarnation)
        #expect(!store.collapsedPaneIds.contains(empty.id))
        #expect(store.focusedPaneId == (focus ? empty.id : selected.id))
        #expect(store.zoomedPaneId == (focus ? empty.id : selected.id))
        if content == .shell(location: nil) { #expect(store.shellCommands[empty.id] == ["fixture"]) }
        let published = store.layout
        store.undoClose()
        #expect(store.layout == published)
        #expect(store.layout.pane(for: selected.id) == selected)
    }

    @Test("Targeted companions fill empty panes and preserve unrelated focus")
    func targetedCompanions() {
        let store = MultiplexerStore()
        let empty = store.focusedPane
        store.split(empty.id, axis: .horizontal)
        store.load(sessionId: "selected")
        let selected = store.focusedPane
        store.setZoom(selected.id, enabled: true)
        store.newShell(command: ["fixture"], beside: empty.id, focus: false)
        let shell = store.layout.pane(for: empty.id)
        #expect(shell?.content == .shell(location: nil))
        #expect(shell?.incarnation != empty.incarnation)
        #expect(store.shellCommands[empty.id] == ["fixture"])
        store.show(.files(taskId: "task"), beside: empty.id, focus: false)
        store.show(.flowLog(taskId: "task"), beside: empty.id, focus: false)
        store.show(.files(taskId: "task"), beside: empty.id, focus: false)
        #expect(store.layout.allPanes.count == 5)
        #expect(store.layout.pane(for: empty.id) == shell)
        #expect(store.layout.pane(for: selected.id) == selected)
        #expect(store.focusedPane == selected)
        #expect(store.zoomedPaneId == selected.id)
    }

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
        #expect(store.layout.pane(for: shell)?.content == .shell(location: nil))
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
        #expect(pane.content == .shell(location: nil))
        #expect(store.shellCommands[pane.id] == [])
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

    @Test("opening a conversation after split retains both the shell and original conversation")
    func explicitSplitKeepsBothSessions() throws {
        let store = MultiplexerStore()
        store.load(sessionId: "session-1")
        _ = try #require(store.split(store.focusedPaneId, axis: .vertical))

        store.load(sessionId: "ask-2")

        #expect(store.layout.allPanes.map(\.content) == [
            .session(id: "session-1"),
            .shell(location: nil),
            .session(id: "ask-2"),
        ])
    }

    @Test("new shell uses the empty pane then splits an occupied pane")
    func newShellUsesAvailableSpace() {
        let store = MultiplexerStore()
        store.newShell()
        #expect(store.focusedPane.content == .shell(location: nil))
        #expect(store.layout.allPanes.count == 1)

        store.newShell()
        #expect(store.focusedPane.content == .shell(location: nil))
        #expect(store.layout.allPanes.count == 2)
    }

    @Test("Shell, Files and process companions alternate splits and preserve the conversation")
    func companionsAlternateSplitsAndPreserveTheConversation() throws {
        let store = MultiplexerStore()
        let location = WorkspaceIdentity(machineId: "local", worktree: "/task")
        store.load(sessionId: "main", location: location)
        let main = store.focusedPane
        let shell = try #require(store.newShell(location: location))
        let files = try #require(store.show(.files(taskId: "task", location: location)))
        let log = try #require(store.show(.flowLog(taskId: "task", location: location)))
        guard case .split(.vertical, .leaf(let retained),
            .split(.horizontal, .leaf(let retainedShell),
                .split(.vertical, .leaf(let retainedFiles), .leaf(let retainedLog), _), _), _) = store.layout else {
            Issue.record("Companions must alternate right, down, right")
            return
        }
        #expect(retained == main)
        #expect(retainedShell == shell)
        #expect(retainedFiles == files)
        #expect(retainedLog == log)
        #expect(store.focusedPaneId == log.id)
        store.close(log.id)
        let next = try #require(store.newShell(location: location))
        #expect(store.layout.allPanes.count == 4)
        #expect(store.focusedPaneId == next.id)
        #expect(store.layout.pane(for: main.id) == main)
    }

    @Test("Reset replaces the Session in place and preserves companion state")
    func replacingSessionKeepsCompanions() throws {
        let store = MultiplexerStore()
        store.load(sessionId: "old")
        let main = store.focusedPane
        let shell = try #require(store.newShell(command: ["retained-command"]))
        store.updateRatio(between: main.id, and: shell.id, ratio: 0.63)
        store.replace(main.id, with: .session(id: "fresh", location: nil))
        #expect(store.focusedPaneId == main.id)
        #expect(store.focusedPane.sessionId == "fresh")
        #expect(store.focusedPane.incarnation != main.incarnation)
        #expect(store.layout.pane(for: shell.id) == shell)
        #expect(store.shellCommands[shell.id] == ["retained-command"])
        guard case .split(_, _, _, let ratio) = store.layout else { Issue.record("Lost split"); return }
        #expect(ratio == 0.63)
    }

    @Test("opening a Session keeps the shell pane alive")
    func loadingSessionPreservesShell() {
        let store = MultiplexerStore()
        store.newShell()

        store.load(sessionId: "session-1")

        #expect(store.layout.allPanes.map(\.content) == [
            .shell(location: nil),
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

    @Test("Removing all Sessions retains the last empty slot with a fresh occurrence", arguments: [1, 3])
    func removedSessionsLeaveEmptySlot(count: Int) {
        let store = MultiplexerStore()
        let ids = (0..<count).map { "session-\($0)" }
        for id in ids { store.reveal(sessionId: id) }
        let last = store.focusedPane
        store.setZoom(last.id, enabled: true)
        store.removeSessions(Set(ids))

        #expect(store.layout.allPanes.count == 1)
        #expect(store.focusedPane.content == .empty)
        #expect(store.focusedPane.id == last.id)
        #expect(store.focusedPane.incarnation != last.incarnation)
        #expect(store.zoomedPaneId == nil)
        #expect(!store.canUndoClose)
    }

    @Test("Batch membership moves preserve companions, ratios and focus while retiring stale Undo")
    func batchRemovalPreservesCompanions() throws {
        let store = MultiplexerStore()
        store.newShell(command: ["retained-command"])
        let shell = store.focusedPane
        store.reveal(sessionId: "first")
        let first = store.focusedPane
        store.reveal(sessionId: "second")
        let second = store.focusedPane
        store.show(.files(taskId: "task"))
        let files = store.focusedPane
        store.updateRatio(between: shell.id, and: files.id, ratio: 0.3)
        let temporary = try #require(store.split(files.id, axis: .horizontal))
        store.close(temporary.id)
        store.setCollapsed(paneId: first.id, collapsed: true)
        store.setFocusedPane(files.id)
        store.setZoom(second.id, enabled: true)
        #expect(store.canUndoClose)

        store.removeSessions(["first", "second"])

        let retained = LayoutNode.split(.vertical, first: .leaf(shell), second: .leaf(files), ratio: 0.3)
        #expect(store.layout == retained)
        #expect(store.focusedPane == files)
        #expect(store.collapsedPaneIds.isEmpty)
        #expect(store.zoomedPaneId == nil)
        #expect(store.shellCommands[shell.id] == ["retained-command"])
        #expect(!store.canUndoClose)
        store.undoClose()
        store.removeSessions(["first", "second"])
        #expect(store.layout == retained)
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
        #expect(store.layout.pane(for: shellPane)?.content == .shell(location: nil))
        if !retained {
            #expect(store.layout.allPanes.map(\.id) == [shellPane])
            #expect(store.focusedPaneId == shellPane)
        }
    }

    @Test("Focus and close follow split geometry instead of tree order")
    func spatialFocusAndClose() {
        let left = PaneState(id: "left", content: .shell(location: nil))
        let upperRight = PaneState(id: "upper-right", content: .shell(location: nil))
        let lowerRight = PaneState(id: "lower-right", content: .shell(location: nil))
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
        store.setFocusedPane(upperRight.id)
        store.focus(.down)
        #expect(store.focusedPaneId == lowerRight.id)
        store.focus(.up)
        #expect(store.focusedPaneId == upperRight.id)
        store.focus(.left)
        #expect(store.focusedPaneId == left.id)
        store.setCollapsed(paneId: upperRight.id, collapsed: true)
        store.focus(.right)
        #expect(store.focusedPaneId == lowerRight.id)
        store.close(lowerRight.id)
        #expect(store.focusedPaneId == left.id)
    }
}

@Test @MainActor
func canonical_ids_reopening_session_preserves_pane_and_focus() throws {
    let upper = "SESSION_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    let store = MultiplexerStore()
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
