// Loopflow Maestro app entry point.
// macOS 15+ native app for managing worktrees and launching LLM coding sessions.

import SwiftUI

@main
struct MaestroApp: App {
    @State private var recentsService = RecentsService()
    @Environment(\.openWindow) private var openWindow
    @State private var captureError: String?
    @State private var showCaptureError = false
    @AppStorage("appearanceMode") private var appearanceMode = AppearanceMode.system.rawValue

    var body: some Scene {
        let preferredScheme = AppearanceMode(rawValue: appearanceMode)?.colorScheme
        let uiTestMode = AppState.uiTestMode()

        // Welcome/main window - shown on launch
        WindowGroup {
            if uiTestMode != nil {
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
                Button("Capture for Review") {
                    captureCurrentWindow()
                }
                .keyboardShortcut("4", modifiers: [.command])
            }

            // Edit menu shortcut for prompt focus (handled by PromptLauncher)
            CommandGroup(after: .textEditing) {
                Button("Focus Prompt") {
                    // Handled by hidden button in PromptLauncher
                }
                .keyboardShortcut("l", modifiers: .command)
            }
        }
    }

    @MainActor
    private func captureCurrentWindow() {
        let captureService = CaptureService()

        do {
            let outputURL = try captureService.captureKeyWindow()
            NSSound.beep()
            NSWorkspace.shared.activateFileViewerSelecting([outputURL])
        } catch {
            captureError = error.localizedDescription
            showCaptureError = true
        }
    }
}
