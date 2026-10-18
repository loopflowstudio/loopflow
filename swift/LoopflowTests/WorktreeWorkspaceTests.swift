#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

let fixtureMachineId = "home_00000000000000000000000000000001"
func fixtureWorkspace(_ path: String) -> WorkspaceIdentity {
    WorkspaceIdentity(machineId: fixtureMachineId, worktree: path)
}

func fixtureWorkKey(_ location: WorkspaceIdentity) -> WorkWorkspaceKey {
    WorkWorkspaceKey(repository: "fixture-repository", kind: .task, id: "fixture-\(location.machineId)-\(location.worktree)")
}

@MainActor
func fixtureWork(_ registry: SessionsWorkspaceRegistry, _ location: WorkspaceIdentity) -> SessionsWorkspace {
    registry.workspace(for: fixtureWorkKey(location), title: "Fixture Work", location: location)
}

@MainActor
func fixtureWork(_ location: WorkspaceIdentity, isLocal: Bool = true, surfaces: GhosttySurfacePool = GhosttySurfacePool()) -> SessionsWorkspace {
    SessionsWorkspace(key: fixtureWorkKey(location), title: "Fixture Work", location: location,
        localMachineId: isLocal ? location.machineId : nil, surfaces: surfaces)
}

@Suite("Retained Work workspaces")
@MainActor
struct WorktreeWorkspaceTests {
    @Test("Inspection reads retained panes and drafts without allocating, focusing or opening a client")
    func inspectionIsPassive() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        #expect(registry.inspect().isEmpty)
        let workspace = fixtureWork(registry, fixtureWorkspace("/repo"))
        let panes = workspace.multiplexer
        panes.load(sessionId: "retained")
        let sessionPane = panes.focusedPaneId
        panes.show(.files(taskId: "task"))
        let filesPane = panes.focusedPaneId
        panes.setCollapsed(paneId: filesPane, collapsed: true)
        panes.toggleZoom(sessionPane)
        let files = workspace.files(issue: "TASK", cwd: "/repo")
        let document = files.document("note.txt")
        document.editor.string = "unfinished draft"
        let layout = panes.layout
        let before = registry.keys
        let reading = try #require(registry.inspect().first)
        #expect(reading.focusedPane == sessionPane)
        #expect(reading.zoomedPane == sessionPane)
        #expect(reading.hiddenPanes == [filesPane])
        #expect(reading.layout.children.map(\.content) == ["session", "files"])
        #expect(reading.layout.children.map(\.subject) == ["retained", "task"])
        #expect(registry.keys == before)
        #expect(panes.layout == layout)
        #expect(panes.focusedPaneId == sessionPane)
        #expect(document.editor.string == "unfinished draft")
    }

    @Test("Files and the Flow process log open as panes beside a Session and are revealed, not duplicated")
    func taskPanes() {
        let panes = fixtureWork(fixtureWorkspace("/repo")).multiplexer
        panes.show(.files(taskId: "task"))
        #expect(panes.layout.allPanes.map(\.content) == [.files(taskId: "task")])
        panes.load(sessionId: "design")
        panes.show(.flowLog(taskId: "task"))
        #expect(panes.layout.allPanes.map(\.content)
            == [.files(taskId: "task"), .session(id: "design"), .flowLog(taskId: "task")])
        let log = panes.focusedPaneId
        panes.setCollapsed(paneId: log, collapsed: true)
        panes.show(.flowLog(taskId: "task"))
        #expect(panes.layout.allPanes.count == 3)
        #expect(panes.focusedPaneId == log)
        #expect(!panes.collapsedPaneIds.contains(log))
    }

    @Test("Repository, Wave and two Tasks sharing one path keep distinct retained Work")
    func samePathWorkStaysSeparate() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let location = fixtureWorkspace("/repo")
        let keys = [
            WorkWorkspaceKey(repository: "repo", kind: .repository, id: "repo"),
            WorkWorkspaceKey(repository: "repo", kind: .wave, id: "wave"),
            WorkWorkspaceKey(repository: "repo", kind: .task, id: "one"),
            WorkWorkspaceKey(repository: "repo", kind: .task, id: "two")]
        for key in keys { registry.open(key, title: key.id, location: location, besideFocused: true) }
        #expect(registry.openWork == keys)
        let first = try #require(registry.retained(keys[2]))
        let shell = try #require(try first.newShell())
        let document = first.files(issue: "ONE", cwd: "/repo").document("draft.md")
        document.editor.string = "unfinished draft"
        first.multiplexer.toggleZoom(shell.id)
        let layout = first.multiplexer.layout
        let selected = first.multiplexer.focusedPaneId
        #if GHOSTTY_ENABLED
        let view = registry.surfaces.view(for: try #require(shell.terminal))
        #endif
        registry.hide(keys[2])
        #expect(!registry.openWork.contains(keys[2]))
        #expect(registry.retained(keys[2]) === first)
        #expect(registry.open(keys[2], title: "Renamed Task", location: nil, besideFocused: true) === first)
        #expect(registry.openWork.count == 4)
        #expect(first.title == "Renamed Task")
        #expect(first.multiplexer.layout == layout)
        #expect(first.multiplexer.focusedPaneId == selected)
        #expect(first.multiplexer.zoomedPaneId == shell.id)
        #expect(document.editor.string == "unfinished draft")
        #if GHOSTTY_ENABLED
        #expect(registry.surfaces.view(for: try #require(shell.terminal)) === view)
        #endif
        for key in keys { registry.hide(key) }
        #expect(registry.selectedWork == nil)
        #expect(registry.openWork.isEmpty)
        #expect(first.multiplexer.layout == layout)
        #expect(first.multiplexer.layout.pane(for: shell.id)?.content.selectedShell?.command == [])
        #expect(registry.inspect().allSatisfy { !$0.isOpen && !$0.selected })
    }

    @Test("Unplaced Work opens without acquiring a location or a shell")
    func unplacedWork() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let key = WorkWorkspaceKey(repository: "repo", kind: .task, id: "unplaced")
        let workspace = registry.open(key, title: "Unplaced", location: nil)
        let before = workspace.multiplexer.layout
        #expect(throws: RegistryQueryError.self) { try workspace.newShell() }
        #expect(workspace.multiplexer.layout == before)
        #expect(registry.selectedWork == key)
    }

    @Test("A failed Machine refresh retains the same workspace identity")
    func failedMachineRefresh() async {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let workspace = fixtureWork(registry, fixtureWorkspace("/repo"))
        await registry.refreshMachine(query: RegistryQuery { _, _ in throw RegistryQueryError("offline") })
        #expect(registry.localMachineId == fixtureMachineId)
        #expect(registry.machineError == "offline")
        #expect(fixtureWork(registry, fixtureWorkspace("/repo")) === workspace)
    }

    @Test("Session location changes renew exact targets without changing its Work or duplicating its surface")
    func sessionPresentationAndMembership() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let key = WorkWorkspaceKey(repository: "repo", kind: .task, id: "task")
        let first = registry.open(key, title: "Task", location: fixtureWorkspace("/first"))
        let otherKey = WorkWorkspaceKey(repository: "repo", kind: .task, id: "other")
        let other = registry.open(otherKey, title: "Other", location: fixtureWorkspace("/first"))
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        var object = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/session.json"))) as? [String: Any])
        object["task_ids"] = ["task", "other"]
        object["work"] = ["kind": "task", "id": "task"]
        var record = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: object))
        record.workspace = SessionWorkspace(machineId: fixtureMachineId, worktree: "/first", taskId: "task", unavailable: nil)
        registry.revealSession(record, in: first)
        let pane = try #require(first.multiplexer.pane(forSessionId: record.id))
        registry.reconcileMembership([])
        #expect(first.multiplexer.pane(forSessionId: record.id) == pane)
        record.workspace = SessionWorkspace(machineId: fixtureMachineId, worktree: "/moved", taskId: "task", unavailable: nil)
        registry.reconcileMembership([record])
        let moved = try #require(first.multiplexer.pane(forSessionId: record.id))
        #expect(moved.id == pane.id)
        #expect(moved.incarnation != pane.incarnation)
        #expect(moved.content.location?.worktree == "/moved")
        registry.revealSession(record, in: other)
        #expect(first.multiplexer.pane(forSessionId: record.id) == nil)
        #expect(other.multiplexer.pane(forSessionId: record.id) != nil)
        #expect(registry.key(containingSession: record.id) == otherKey)
        #expect(!first.multiplexer.canUndoClose)
    }

    @Test("Switching worktrees restores companion terminals, layout and focus")
    func restoresWholeWorkspace() {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let design = fixtureWork(registry, fixtureWorkspace("/repo.design"))
        design.multiplexer.load(sessionId: "design")
        design.multiplexer.newShell(location: design.location)
        design.multiplexer.newShell(location: design.location)
        let original = design.multiplexer.layout
        let focus = design.multiplexer.focusedPaneId

        fixtureWork(registry, fixtureWorkspace("/repo.other")).multiplexer.newShell()

        #expect(fixtureWork(registry, fixtureWorkspace("/repo.design")) === design)
        #expect(design.multiplexer.layout == original)
        #expect(design.multiplexer.focusedPaneId == focus)
        #expect(registry.key(containingShell: design.multiplexer.focusedPane.content.selectedShell?.id ?? "", machineId: fixtureMachineId) == fixtureWorkKey(fixtureWorkspace("/repo.design")))
    }

    @Test("Conversation launch adds a tab and undo never replays its command")
    func launchingPreservesExistingPanes() throws {
        let store = MultiplexerStore()
        store.newShell()
        let shell = store.focusedPaneId
        let first = store.focusedPane.content.selectedShell!
        store.newShell(command: ["lf", "--interactive", ":", "hello"])
        #expect(store.layout.allPanes.count == 1)
        #expect(store.shell(first.id) == first)
        #expect(store.layout.pane(for: shell)?.content.selectedShell != nil)
        #expect(store.shell(first.id)?.command == [])
        #expect(store.layout.pane(for: store.focusedPaneId)?.content.selectedShell?.command?.last == "hello")
        store.close(store.focusedPaneId)
        store.undoClose()
        #expect(store.layout.pane(for: store.focusedPaneId)?.content.selectedShell?.command == nil)
    }

}

@Suite("Independent Task conversations")
struct TaskConversationLaunchTests {
    @Test("New conversation binds the Task without selecting an operating skill")
    func taskContext() {
        let launch = TaskConversationLaunch(task: "LOO-123")
        #expect(launch.arguments(lf: "/App/lf") == [
            "/App/lf", "--interactive", "--task", "LOO-123", ":", launch.prompt,
        ])
        #expect(launch.prompt.contains("Task"))
        #expect(launch.prompt.contains("does not request an autonomous operating pass"))
    }
}
#endif
