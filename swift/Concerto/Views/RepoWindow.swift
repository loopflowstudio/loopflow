// Window wrapper for a single repository.
// Each repo window has its own state instances.

import SwiftUI
import LoopflowCore

struct RepoWindow: View {
    let repoURL: URL?
    let recentsService: RecentsService

    @State private var repoState = RepoState()
    @State private var sessionState = SessionState()
    @State private var launcherState = LauncherState()

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
                ContentView()
                    .environment(repoState)
                    .environment(sessionState)
                    .environment(launcherState)
            }
        }
        .background(WindowAccessor(repoURL: repoURL))
        .task {
            if let mode = RepoState.uiTestMode() {
                setupComplete = true
                hasCheckedSetup = true
                hasOpenedRepo = true
                let url = repoURL ?? URL(fileURLWithPath: "/tmp/loopflow-ui-tests")
                repoState.configureForUITest(mode, repoURL: url)
                return
            }

            // Check setup first
            let status = setupService.checkDependencies()
            setupComplete = status.lfInstalled
            hasCheckedSetup = true

            // Then open repo if setup is complete
            if setupComplete, let url = repoURL, !hasOpenedRepo {
                hasOpenedRepo = true
                await repoState.openRepo(url, launcherState: launcherState, sessionState: sessionState)
                recentsService.addRecent(url)
            }
        }
        .onChange(of: setupComplete) { _, complete in
            if complete, let url = repoURL, !hasOpenedRepo {
                hasOpenedRepo = true
                Task {
                    await repoState.openRepo(url, launcherState: launcherState, sessionState: sessionState)
                    recentsService.addRecent(url)
                }
            }
        }
    }
}

/// Helper to set representedURL on the hosting window for snapshot service.
private struct WindowAccessor: NSViewRepresentable {
    let repoURL: URL?

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        DispatchQueue.main.async {
            view.window?.representedURL = repoURL
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        DispatchQueue.main.async {
            nsView.window?.representedURL = repoURL
        }
    }
}
