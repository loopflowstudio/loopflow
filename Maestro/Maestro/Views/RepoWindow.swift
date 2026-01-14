// Window wrapper for a single repository.
// Each repo window has its own AppState instance.

import SwiftUI

struct RepoWindow: View {
    let repoURL: URL?
    let recentsService: RecentsService
    @State private var appState = AppState()
    @State private var setupComplete = false
    @State private var hasCheckedSetup = false
    @State private var hasOpenedRepo = false

    private let setupService = SetupService()

    var body: some View {
        Group {
            if !hasCheckedSetup {
                ProgressView("Loading...")
            } else if !setupComplete {
                SetupView(isComplete: $setupComplete)
            } else {
                ContentView(appState: appState)
            }
        }
        .task {
            // Check setup first
            let status = setupService.checkDependencies()
            setupComplete = status.allInstalled
            hasCheckedSetup = true

            // Then open repo if setup is complete
            if setupComplete, let url = repoURL, !hasOpenedRepo {
                hasOpenedRepo = true
                await appState.openRepo(url)
                recentsService.addRecent(url)
            }
        }
        .onChange(of: setupComplete) { _, complete in
            // Handle case where setup completes after initial check
            if complete, let url = repoURL, !hasOpenedRepo {
                hasOpenedRepo = true
                Task {
                    await appState.openRepo(url)
                    recentsService.addRecent(url)
                }
            }
        }
    }
}
