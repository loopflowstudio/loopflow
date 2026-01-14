// Loopflow Maestro app entry point.
// macOS 15+ native app for managing worktrees and launching LLM coding sessions.

import SwiftUI

@main
struct MaestroApp: App {
    @State private var recentsService = RecentsService()

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
    }
}
