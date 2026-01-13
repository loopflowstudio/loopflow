// Loopflow Maestro app entry point.
// macOS 15+ native app for managing worktrees and launching LLM coding sessions.

import SwiftUI

@main
struct MaestroApp: App {
    @State private var appState = AppState()
    @State private var setupComplete = false
    @State private var hasCheckedSetup = false

    private let setupService = SetupService()

    var body: some Scene {
        WindowGroup {
            if !hasCheckedSetup {
                ProgressView("Loading...")
                    .onAppear {
                        checkSetup()
                    }
            } else if !setupComplete {
                SetupView(isComplete: $setupComplete)
            } else {
                ContentView(appState: appState)
            }
        }
        .windowStyle(.automatic)
        .defaultSize(width: 900, height: 700)
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("Open Folder...") {
                    openFolder()
                }
                .keyboardShortcut("o", modifiers: .command)
            }

            CommandGroup(after: .sidebar) {
                Button("Refresh Worktrees") {
                    Task { await appState.refreshWorktrees() }
                }
                .keyboardShortcut("r", modifiers: .command)
            }
        }
    }

    private func checkSetup() {
        let status = setupService.checkDependencies()
        setupComplete = status.allInstalled
        hasCheckedSetup = true
    }

    private func openFolder() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.message = "Choose a repository folder"

        if panel.runModal() == .OK, let url = panel.url {
            Task {
                await appState.openRepo(url)
            }
        }
    }
}
