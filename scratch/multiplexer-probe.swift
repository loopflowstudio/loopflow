import Foundation

@main
struct MultiplexerProbe {
    @MainActor
    static func main() {
        let store = MultiplexerStore()
        store.load(sessionId: "design")
        store.newShell(command: ["printf", "probe"])
        let shell = store.focusedPaneId
        let before = store.layout
        store.toggleZoom(shell)
        precondition(store.layout == before)
        store.toggleZoom(shell)
        precondition(store.layout == before)
        precondition(store.shellCommands[shell] == ["printf", "probe"])
        print("PASS: zoom/restore preserves layout, pane identity, and shell command")

        store.close(shell)
        precondition(store.layout.pane(for: shell) == nil)
        precondition(store.shellCommands[shell] == nil)
        store.undoClose()
        precondition(store.layout == before)
        precondition(store.shellCommands[shell] == nil)
        print("PASS: close removes pane and launch command; undo does not replay it")
        print("CONCLUSION: collapse must preserve panes; close/undo is not collapse")
    }
}
