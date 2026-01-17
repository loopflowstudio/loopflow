// SwiftUI wrapper for SwiftTerm's LocalProcessTerminalView.

import AppKit
import SwiftTerm
import SwiftUI

struct EmbeddedTerminalView: NSViewRepresentable {
    let command: String
    let workingDirectory: URL
    let onTerminate: () -> Void

    func makeNSView(context: Context) -> LocalProcessTerminalView {
        let terminal = LocalProcessTerminalView(frame: .zero)
        terminal.getTerminal().silentLog = true

        // Configure terminal appearance
        terminal.nativeBackgroundColor = NSColor.textBackgroundColor
        terminal.nativeForegroundColor = NSColor.textColor
        terminal.configureNativeColors()

        if let font = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular) as NSFont? {
            terminal.font = font
        }

        terminal.processDelegate = context.coordinator
        context.coordinator.terminal = terminal
        return terminal
    }

    func updateNSView(_ terminal: LocalProcessTerminalView, context: Context) {
        if !context.coordinator.hasStarted {
            context.coordinator.hasStarted = true
            startProcess(in: terminal)
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(onTerminate: onTerminate)
    }

    private func startProcess(in terminal: LocalProcessTerminalView) {
        let shell = ProcessInfo.processInfo.environment["SHELL"] ?? "/bin/zsh"
        terminal.startProcess(
            executable: shell,
            args: ["-l", "-c", command],
            environment: buildEnvironment(),
            execName: "shell"
        )
    }

    private func buildEnvironment() -> [String] {
        var env = ProcessInfo.processInfo.environment
        env["TERM"] = "xterm-256color"
        env["COLORTERM"] = "truecolor"
        // Remove TERM_PROGRAM to avoid confusing the shell
        env.removeValue(forKey: "TERM_PROGRAM")
        env.removeValue(forKey: "TERM_PROGRAM_VERSION")

        return env.map { "\($0.key)=\($0.value)" }
    }

    @MainActor
    class Coordinator: NSObject, LocalProcessTerminalViewDelegate {
        var terminal: LocalProcessTerminalView?
        var hasStarted = false
        let onTerminate: () -> Void

        init(onTerminate: @escaping () -> Void) {
            self.onTerminate = onTerminate
        }

        nonisolated func sizeChanged(source: LocalProcessTerminalView, newCols: Int, newRows: Int) {}
        nonisolated func setTerminalTitle(source: LocalProcessTerminalView, title: String) {}
        nonisolated func hostCurrentDirectoryUpdate(source: TerminalView, directory: String?) {}

        nonisolated func processTerminated(source: TerminalView, exitCode: Int32?) {
            Task { @MainActor [weak self] in
                self?.onTerminate()
            }
        }
    }
}
