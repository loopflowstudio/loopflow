#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Worktree workspaces")
@MainActor
struct WorktreeWorkspaceTests {
    @Test("Switching worktrees restores companion terminals, layout and focus")
    func restoresWholeWorkspace() {
        let registry = SessionsWorkspaceRegistry()
        let outer = registry.layout(for: "/repo")
        outer.select("/repo.design")
        let design = registry.workspace(for: "/repo.design")
        design.multiplexer.load(sessionId: "design")
        design.multiplexer.newShell()
        design.multiplexer.newShell()
        let original = design.multiplexer.layout
        let focus = design.multiplexer.focusedPaneId

        outer.select("/repo.other")
        registry.workspace(for: "/repo.other").multiplexer.newShell()
        outer.select("/repo.design")

        #expect(registry.workspace(for: outer.focusedPath!) === design)
        #expect(design.multiplexer.layout == original)
        #expect(design.multiplexer.focusedPaneId == focus)
        #expect(registry.path(containingShell: focus) == "/repo.design")
    }

    @Test("Both split levels are independent and closing an outer slot retains its workspace")
    func independentSplits() {
        let registry = SessionsWorkspaceRegistry()
        let outer = registry.layout(for: "/repo")
        let firstSlot = outer.focusedSlotId
        let first = registry.workspace(for: "/repo")
        first.multiplexer.newShell()
        outer.split(firstSlot, axis: .vertical)
        outer.select("/repo.other")
        let secondSlot = outer.focusedSlotId
        let second = registry.workspace(for: "/repo.other")
        second.multiplexer.newShell()
        let secondLayout = second.multiplexer.layout
        first.multiplexer.newShell()

        #expect(outer.layout.slots.count == 2)
        #expect(second.multiplexer.layout == secondLayout)
        outer.close(firstSlot)
        outer.select("/repo")
        #expect(first.multiplexer.layout.allPanes.count == 2)
        #expect(second.multiplexer.layout == secondLayout)
        #expect(outer.focusedSlotId == secondSlot)
        #expect(registry.workspace(for: "/repo") === first)
    }

    @Test("Selecting an already visible worktree focuses it without duplicating its surfaces")
    func focusesExistingSlot() {
        let outer = WorktreeLayoutStore(path: "/repo")
        let first = outer.focusedSlotId
        outer.split(first, axis: .horizontal)
        let empty = outer.focusedSlotId
        outer.select("/repo", in: empty)
        #expect(outer.focusedSlotId == first)
        #expect(outer.layout.slots.filter { $0.path == "/repo" }.count == 1)
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
            (.project(repo: "/repo", id: "desktop"), ["--project", "desktop"], "Project"),
            (.task(repo: "/repo.task", id: "LOO-123"), ["--task", "LOO-123"], "Task"),
        ]
        for (scope, binding, subject) in cases {
            let launch = ConversationLaunch(scope: scope)
            let command = launch.arguments(lf: "/App/lf")
            #expect(Array(command.dropFirst(2).dropLast(2)) == binding)
            #expect(command.last?.contains(subject) == true)
            #expect(command.contains("--interactive"))
            #expect(!command.contains("--tui"))
            #expect(!command.contains("--ide"))
            #expect(!command.contains("wave/operate"))
            #expect(!command.contains("project/operate"))
        }
    }
}
#endif
