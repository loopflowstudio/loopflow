// Loopflow Maestro app entry point.
// macOS 15+ native app for managing worktrees and launching LLM coding sessions.

import SwiftUI

@main
struct MaestroApp: App {
    @State private var recentsService = RecentsService()
    @Environment(\.openWindow) private var openWindow
    @State private var captureError: String?
    @State private var showCaptureError = false

    var body: some Scene {
        // Welcome/main window - shown on launch
        WindowGroup {
            WelcomeWindow(recentsService: recentsService)
        }
        .windowStyle(.automatic)
        .defaultSize(width: 500, height: 400)

        // Repo windows - opened explicitly for each repository
        WindowGroup(id: "repo", for: URL.self) { $repoURL in
            RepoWindow(repoURL: repoURL, recentsService: recentsService)
        }
        .windowStyle(.automatic)
        .defaultSize(width: 900, height: 700)

        // Agents window - global agent management (beta only)
        WindowGroup(id: "agents") {
            AgentWindow()
        }
        .windowStyle(.automatic)
        .defaultSize(width: 800, height: 600)
        .commands {
            // Beta features menu
            CommandGroup(after: .appSettings) {
                Toggle("Beta Features", isOn: Binding(
                    get: { Flags.beta },
                    set: { Flags.setBeta($0) }
                ))
            }

            // Agents menu item (beta only)
            if Flags.beta {
                CommandGroup(after: .windowArrangement) {
                    Button("Agents") {
                        openWindow(id: "agents")
                    }
                    .keyboardShortcut("a", modifiers: [.command, .shift])
                }
            }

            CommandGroup(after: .saveItem) {
                Button("Capture for Review") {
                    captureCurrentWindow()
                }
                .keyboardShortcut("4", modifiers: [.command])
            }
        }
    }

    @MainActor
    private func captureCurrentWindow() {
        let captureService = CaptureService()

        // Try to find the repo root from the current window's represented URL
        let repoRoot = NSApp.keyWindow?.representedURL

        do {
            let outputURL = try captureService.captureKeyWindow(repoRoot: repoRoot)
            // Play sound feedback
            NSSound.beep()
            // Show in Finder
            NSWorkspace.shared.activateFileViewerSelecting([outputURL])
        } catch {
            captureError = error.localizedDescription
            showCaptureError = true
        }
    }
}
