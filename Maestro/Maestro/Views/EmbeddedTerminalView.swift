// SwiftUI wrapper for SwiftTerm's LocalProcessTerminalView.

import AppKit
import SwiftTerm
import SwiftUI

struct EmbeddedTerminalView: NSViewRepresentable {
    @Binding var process: Process?
    @Binding var isRunning: Bool
    let command: String
    let workingDirectory: URL
    let onTerminate: () -> Void

    func makeNSView(context: Context) -> LocalProcessTerminalView {
        let terminal = LocalProcessTerminalView(frame: .zero)
        terminal.getTerminal().silentLog = true

        // Configure terminal appearance using native colors
        terminal.nativeBackgroundColor = NSColor.textBackgroundColor
        terminal.nativeForegroundColor = NSColor.textColor
        terminal.configureNativeColors()

        // Set terminal font
        if let font = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular) as NSFont? {
            terminal.font = font
        }

        // Set process delegate to receive termination notification
        terminal.processDelegate = context.coordinator

        context.coordinator.terminal = terminal
        return terminal
    }

    func updateNSView(_ terminal: LocalProcessTerminalView, context: Context) {
        // Start process if not already running
        if process == nil && !context.coordinator.hasStarted {
            context.coordinator.hasStarted = true
            startProcess(in: terminal, context: context)
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    private func startProcess(in terminal: LocalProcessTerminalView, context: Context) {
        // Build shell command
        let shell = ProcessInfo.processInfo.environment["SHELL"] ?? "/bin/zsh"

        // Start the process with PTY
        terminal.startProcess(
            executable: shell,
            args: ["-l", "-c", command],
            environment: buildEnvironment(),
            execName: "shell"
        )

        // Create a reference Process object for tracking (not the actual process - SwiftTerm manages that)
        let processRef = Process()
        DispatchQueue.main.async {
            self.process = processRef
            self.isRunning = true
        }
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
        var parent: EmbeddedTerminalView
        var terminal: LocalProcessTerminalView?
        var hasStarted = false

        init(_ parent: EmbeddedTerminalView) {
            self.parent = parent
        }

        // MARK: - LocalProcessTerminalViewDelegate

        nonisolated func sizeChanged(source: LocalProcessTerminalView, newCols: Int, newRows: Int) {
            // Terminal resized - nothing to do
        }

        nonisolated func setTerminalTitle(source: LocalProcessTerminalView, title: String) {
            // Title changed - nothing to do
        }

        nonisolated func hostCurrentDirectoryUpdate(source: TerminalView, directory: String?) {
            // Directory changed - nothing to do
        }

        nonisolated func processTerminated(source: TerminalView, exitCode: Int32?) {
            Task { @MainActor [weak self] in
                self?.parent.isRunning = false
                self?.parent.process = nil
                self?.parent.onTerminate()
            }
        }
    }
}
