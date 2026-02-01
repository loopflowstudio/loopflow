// Loopflow Concerto app entry point.
// macOS 15+ native app for managing worktrees and launching LLM coding sessions.

import SwiftUI
import LoopflowCore

@main
struct ConcertoApp: App {
    @State private var recentsService = RecentsService()
    @Environment(\.openWindow) private var openWindow
    @State private var snapshotError: String?
    @State private var showSnapshotError = false
    @AppStorage("appearanceMode") private var appearanceMode = AppearanceMode.system.rawValue

    init() {
        Task {
            try? await NotificationService.shared.requestAuthorization()
        }
    }

    var body: some Scene {
        let preferredScheme = AppearanceMode(rawValue: appearanceMode)?.colorScheme
        let uiTestMode = RepoState.uiTestMode()
        let screenshotMode = RepoState.ScreenshotMode.fromArgs()

        // Welcome/main window - shown on launch
        WindowGroup {
            if let screenshot = screenshotMode {
                ScreenshotWindow(mode: screenshot, recentsService: recentsService)
                    .tint(.loopflowBurgundy)
                    .preferredColorScheme(preferredScheme)
            } else if uiTestMode != nil {
                RepoWindow(
                    repoURL: URL(fileURLWithPath: "/tmp/loopflow-ui-tests"),
                    recentsService: recentsService
                )
                .tint(.loopflowBurgundy)
                .preferredColorScheme(preferredScheme)
            } else {
                WelcomeWindow(recentsService: recentsService)
                    .tint(.loopflowBurgundy)
                    .preferredColorScheme(preferredScheme)
            }
        }
        .windowStyle(.automatic)
        .defaultSize(width: 500, height: 400)

        // Repo windows - opened explicitly for each repository
        WindowGroup(id: "repo", for: URL.self) { $repoURL in
            RepoWindow(repoURL: repoURL, recentsService: recentsService)
                .tint(.loopflowBurgundy)
                .preferredColorScheme(preferredScheme)
        }
        .windowStyle(.automatic)
        .defaultSize(width: 900, height: 700)

        // Terminal test window - for testing embedded Ghostty
        Window("Terminal Test", id: "terminal-test") {
            TerminalTestWindow()
                .preferredColorScheme(preferredScheme)
        }
        .defaultSize(width: 800, height: 600)

        .commands {
            // Beta features menu
            CommandGroup(after: .appSettings) {
                Toggle("Beta Features", isOn: Binding(
                    get: { Flags.beta },
                    set: { Flags.setBeta($0) }
                ))
                Divider()
                Picker("Appearance", selection: Binding(
                    get: { appearanceMode },
                    set: { appearanceMode = $0 }
                )) {
                    ForEach(AppearanceMode.allCases, id: \.rawValue) { mode in
                        Text(mode.menuTitle).tag(mode.rawValue)
                    }
                }
                .pickerStyle(.radioGroup)
            }

            CommandGroup(after: .saveItem) {
                Button("Snapshot for Review") {
                    snapshotCurrentWindow()
                }
                .keyboardShortcut("4", modifiers: [.command])
            }

            // Edit menu shortcuts
            CommandGroup(after: .textEditing) {
                Button("Focus Prompt") {
                    NotificationCenter.default.post(name: .focusPromptInput, object: nil)
                }
                .keyboardShortcut("l", modifiers: .command)
            }

            // View menu - command palette and navigation
            CommandGroup(after: .sidebar) {
                Button("Command Palette") {
                    NotificationCenter.default.post(name: .toggleCommandPalette, object: nil)
                }
                .keyboardShortcut("k", modifiers: .command)

                Divider()

                Button("New Workspace") {
                    NotificationCenter.default.post(name: .showNewWorktreeSheet, object: nil)
                }
                .keyboardShortcut("n", modifiers: .command)

                Divider()

                Button("Terminal Test") {
                    openWindow(id: "terminal-test")
                }
                .keyboardShortcut("t", modifiers: [.command, .shift])
            }
        }
    }

    @MainActor
    private func snapshotCurrentWindow() {
        let snapshotService = SnapshotService()

        do {
            let outputURL = try snapshotService.snapshotKeyWindow()
            NSSound.beep()
            NSWorkspace.shared.activateFileViewerSelecting([outputURL])
        } catch {
            snapshotError = error.localizedDescription
            showSnapshotError = true
        }
    }
}
