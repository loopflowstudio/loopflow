import Loopflow
import SwiftUI

/// The Settings pane and the standalone Keyboard Shortcuts window share this reference.
struct KeyboardShortcutsView: View {
    var body: some View {
        Form {
            Section("Navigation") {
                LabeledContent("Navigate", value: "⌘K")
                LabeledContent("Toggle sidebar", value: "⌘\\")
                LabeledContent("Task actions", value: "⌘/")
                shortcuts(for: .window)
            }
            Section("Task Files") {
                LabeledContent("Choose Task file", value: "⌘P")
                LabeledContent("Save Task file", value: "⌘S")
            }
            Section("App") {
                LabeledContent("Open repository", value: "⇧⌘O")
                LabeledContent("Settings", value: "⌘,")
                shortcuts(for: .pane)
            }
        }
        .formStyle(.grouped)
        .frame(minWidth: 380, minHeight: 400)
    }

    private func shortcuts(for scope: DesktopActionScope) -> some View {
        ForEach(DesktopAction.all.filter { $0.scope == scope && !$0.keys.isEmpty }, id: \.name) { action in
            let keys = action.keys.map(\.display)
            let display = if action.name == "goto", let first = keys.first, let last = keys.last {
                "\(first)…\(last)"
            } else {
                keys.joined(separator: " / ")
            }
            LabeledContent(action.title, value: display)
        }
    }
}
