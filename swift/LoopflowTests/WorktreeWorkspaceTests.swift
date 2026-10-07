#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

let fixtureMachineId = "home_00000000000000000000000000000001"
func fixtureWorkspace(_ path: String) -> WorkspaceIdentity {
    WorkspaceIdentity(machineId: fixtureMachineId, worktree: path)
}

@Suite("Worktree workspaces")
@MainActor
struct WorktreeWorkspaceTests {
    @Test("Files and the Flow process log open as panes beside a Session and are revealed, not duplicated")
    func taskPanes() {
        let panes = SessionsWorkspace().multiplexer
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

    @Test("Equal paths on different Machines retain separate layouts and documents")
    func machinesAreDistinct() {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let local = fixtureWorkspace("/repo")
        let remote = WorkspaceIdentity(machineId: "remote", worktree: "/repo")
        let first = registry.workspace(for: local)
        let other = registry.workspace(for: remote)
        first.multiplexer.newShell(command: ["server"])
        let files = first.files(taskId: "task", issue: "TASK", cwd: "/repo")
        let document = files.document("note.txt")
        document.editor.string = "draft"
        let outer = registry.layout(for: local)
        outer.split(outer.focusedSlotId, axis: .vertical)
        outer.select(remote)
        #expect(outer.layout.slots.compactMap(\.path) == [local, remote])
        #expect(other.multiplexer.focusedPane.content == .empty)
        #expect(other.files(taskId: "task", issue: "TASK", cwd: "/repo") !== files)
        outer.select(local)
        #expect(first.files(taskId: "task", issue: "TASK", cwd: "/repo").document("note.txt") === document)
        #expect(document.editor.string == "draft")
    }

    @Test("A failed Machine refresh retains the same workspace identity")
    func failedMachineRefresh() async {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let workspace = registry.workspace(for: fixtureWorkspace("/repo"))
        await registry.refreshMachine(query: RegistryQuery { _, _ in throw RegistryQueryError("offline") })
        #expect(registry.localMachineId == fixtureMachineId)
        #expect(registry.machineError == "offline")
        #expect(registry.workspace(for: fixtureWorkspace("/repo")) === workspace)
    }

    @Test("Reassociation removes only old placement and reuses the window-owned terminal")
    func reassociationKeepsSurface() throws {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let source = registry.workspace(for: fixtureWorkspace("/first"))
        let target = registry.workspace(for: fixtureWorkspace("/second"))
        let otherRepo = registry.workspace(for: fixtureWorkspace("/other-repo"))
        otherRepo.multiplexer.reveal(sessionId: "unrelated")
        let otherLayout = otherRepo.multiplexer.layout
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/session.json"))
        var record = try JSONDecoder().decode(SessionRecord.self, from: data)
        record.workspace = SessionWorkspace(machineId: fixtureMachineId, worktree: "/second", taskId: "task", unavailable: nil)
        source.multiplexer.reveal(sessionId: record.id)
        source.multiplexer.newShell(command: ["server"])
        let shell = source.multiplexer.focusedPaneId
        #if GHOSTTY_ENABLED
        let view = registry.surfaces.view(for: .session(record.id))
        #endif
        registry.reconcileMembership([record])
        target.multiplexer.reveal(sessionId: record.id)
        #expect(source.multiplexer.pane(forSessionId: record.id) == nil)
        #expect(source.multiplexer.shellCommands[shell] == ["server"])
        #expect(target.multiplexer.pane(forSessionId: record.id) != nil)
        #expect(otherRepo.multiplexer.layout == otherLayout)
        // Absence from another repository's reading cannot remove a retained pane.
        registry.reconcileMembership([])
        #expect(target.multiplexer.pane(forSessionId: record.id) != nil)
        #expect(otherRepo.multiplexer.layout == otherLayout)
        #if GHOSTTY_ENABLED
        #expect(target.surfaces.view(for: .session(record.id)) === view)
        #endif
    }

    @Test("A confirmed resolution removes a closed Session from Undo without touching other Sessions")
    func resolutionInvalidatesUndo() {
        let registry = SessionsWorkspaceRegistry()
        let panes = registry.workspace(for: fixtureWorkspace("/repo")).multiplexer
        panes.reveal(sessionId: "retained")
        panes.reveal(sessionId: "resolved")
        panes.close(panes.focusedPaneId)
        #expect(panes.canUndoClose)
        registry.removeSessions(["resolved"])
        #expect(!panes.canUndoClose)
        #expect(panes.pane(forSessionId: "retained") != nil)
    }

    @Test("Switching worktrees restores companion terminals, layout and focus")
    func restoresWholeWorkspace() {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let outer = registry.layout(for: fixtureWorkspace("/repo"))
        outer.select(fixtureWorkspace("/repo.design"))
        let design = registry.workspace(for: fixtureWorkspace("/repo.design"))
        design.multiplexer.load(sessionId: "design")
        design.multiplexer.newShell()
        design.multiplexer.newShell()
        let original = design.multiplexer.layout
        let focus = design.multiplexer.focusedPaneId

        outer.select(fixtureWorkspace("/repo.other"))
        registry.workspace(for: fixtureWorkspace("/repo.other")).multiplexer.newShell()
        outer.select(fixtureWorkspace("/repo.design"))

        #expect(registry.workspace(for: outer.focusedPath!) === design)
        #expect(design.multiplexer.layout == original)
        #expect(design.multiplexer.focusedPaneId == focus)
        #expect(registry.path(containingShell: focus) == fixtureWorkspace("/repo.design"))
    }

    @Test("Both split levels are independent and closing an outer slot retains its workspace")
    func independentSplits() {
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        let outer = registry.layout(for: fixtureWorkspace("/repo"))
        let firstSlot = outer.focusedSlotId
        let first = registry.workspace(for: fixtureWorkspace("/repo"))
        first.multiplexer.newShell()
        outer.split(firstSlot, axis: .vertical)
        outer.select(fixtureWorkspace("/repo.other"))
        let secondSlot = outer.focusedSlotId
        let second = registry.workspace(for: fixtureWorkspace("/repo.other"))
        second.multiplexer.newShell()
        let secondLayout = second.multiplexer.layout
        first.multiplexer.newShell()

        #expect(outer.layout.slots.count == 2)
        #expect(second.multiplexer.layout == secondLayout)
        outer.close(firstSlot)
        outer.select(fixtureWorkspace("/repo"))
        #expect(first.multiplexer.layout.allPanes.count == 2)
        #expect(second.multiplexer.layout == secondLayout)
        #expect(outer.focusedSlotId == secondSlot)
        #expect(registry.workspace(for: fixtureWorkspace("/repo")) === first)
    }

    @Test("Selecting an already visible worktree focuses it without duplicating its surfaces")
    func focusesExistingSlot() {
        let outer = WorktreeLayoutStore(path: fixtureWorkspace("/repo"))
        let first = outer.focusedSlotId
        outer.split(first, axis: .horizontal)
        let empty = outer.focusedSlotId
        outer.select(fixtureWorkspace("/repo"), in: empty)
        #expect(outer.focusedSlotId == first)
        #expect(outer.layout.slots.filter { $0.path == fixtureWorkspace("/repo") }.count == 1)
        #expect(outer.layout.slots.first { $0.id == empty }?.path == nil)
    }

    @Test("A conversation launch does not replace a live shell or another conversation")
    func launchingPreservesExistingPanes() {
        let store = MultiplexerStore()
        store.newShell()
        let shell = store.focusedPaneId
        store.newShell(command: ["lf", "--interactive", ":", "hello"])
        #expect(store.layout.allPanes.count == 2)
        #expect(store.layout.pane(for: shell)?.content == .shell)
        #expect(store.shellCommands[shell] == [])
        #expect(store.shellCommands[store.focusedPaneId]?.last == "hello")
        store.close(store.focusedPaneId)
        store.undoClose()
        #expect(store.shellCommands[store.focusedPaneId] == nil)
    }
}

@Suite("Scope-aware conversations")
struct ConversationLaunchTests {
    @Test("Each zoom level binds its subject and keeps the configured destination")
    func scopeDefaults() {
        let cases: [(ConversationScope, [String], String)] = [
            (.repo("/repo"), [], "repository"),
            (.wave(repo: "/repo", id: "product"), ["--wave", "product"], "Wave"),
            (.task(repo: "/repo.task", id: "LOO-123"), ["--task", "LOO-123"], "Task"),
        ]
        for (scope, binding, subject) in cases {
            let launch = ConversationLaunch(scope: scope)
            let command = launch.arguments(lf: "/App/lf")
            #expect(Array(command.dropFirst(2).dropLast(2)) == binding)
            #expect(command.last?.contains(subject) == true)
            #expect(command.contains("--interactive"))
            #expect(!command.contains("tui"))
            #expect(!command.contains("ide"))
            #expect(!command.contains("wave/operate"))
            #expect(!command.contains("project/operate"))
        }
    }
}
#endif
