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

        // Configure terminal appearance
        let colors = terminal.getTerminal().installColors(
            Pty15Colors
        )
        terminal.nativeBackgroundColor = NSColor.textBackgroundColor
        terminal.nativeForegroundColor = NSColor.textColor

        // Set terminal font
        if let font = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular) as NSFont? {
            terminal.font = font
        }

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

        // Monitor for termination
        context.coordinator.monitorTermination(terminal)
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
    class Coordinator: NSObject {
        var parent: EmbeddedTerminalView
        var terminal: LocalProcessTerminalView?
        var hasStarted = false
        private var terminationTask: Task<Void, Never>?

        init(_ parent: EmbeddedTerminalView) {
            self.parent = parent
        }

        func monitorTermination(_ terminal: LocalProcessTerminalView) {
            terminationTask = Task { @MainActor in
                // Poll for process completion (SwiftTerm doesn't expose a delegate for this)
                while !Task.isCancelled {
                    try? await Task.sleep(for: .milliseconds(500))

                    // Check if terminal process has exited
                    if !terminal.running {
                        parent.isRunning = false
                        parent.process = nil
                        parent.onTerminate()
                        break
                    }
                }
            }
        }

        deinit {
            terminationTask?.cancel()
        }
    }
}

// Standard 16-color palette for terminal
private let Pty15Colors: [Color] = [
    Color(red: 0, green: 0, blue: 0),           // black
    Color(red: 0.8, green: 0, blue: 0),         // red
    Color(red: 0, green: 0.8, blue: 0),         // green
    Color(red: 0.8, green: 0.8, blue: 0),       // yellow
    Color(red: 0, green: 0, blue: 0.8),         // blue
    Color(red: 0.8, green: 0, blue: 0.8),       // magenta
    Color(red: 0, green: 0.8, blue: 0.8),       // cyan
    Color(red: 0.8, green: 0.8, blue: 0.8),     // white
    Color(red: 0.4, green: 0.4, blue: 0.4),     // bright black
    Color(red: 1, green: 0.4, blue: 0.4),       // bright red
    Color(red: 0.4, green: 1, blue: 0.4),       // bright green
    Color(red: 1, green: 1, blue: 0.4),         // bright yellow
    Color(red: 0.4, green: 0.4, blue: 1),       // bright blue
    Color(red: 1, green: 0.4, blue: 1),         // bright magenta
    Color(red: 0.4, green: 1, blue: 1),         // bright cyan
    Color(red: 1, green: 1, blue: 1),           // bright white
]
