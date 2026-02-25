// Window wrapper for a single repository.
// Each repo window has its own state instances.

import SwiftUI
import LoopflowCore

struct RepoWindow: View {
    let repoURL: URL?
    let portfolioService: PortfolioService

    @State private var repoState = RepoState()
    @State private var outputBuffer = OutputBuffer()

    @State private var setupComplete = false
    @State private var hasCheckedSetup = false
    @State private var hasOpenedRepo = false
    @State private var pendingWaveSelection: String?

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
                    .environment(outputBuffer)
            }
        }
        .background(WindowAccessor { window in
            window?.representedURL = repoURL
        })
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
                await repoState.openRepo(url, outputBuffer: outputBuffer)
                portfolioService.addRepo(url)
                applyPendingWaveSelectionIfNeeded()
            }
        }
        .onChange(of: setupComplete) { _, complete in
            if complete, let url = repoURL, !hasOpenedRepo {
                hasOpenedRepo = true
                Task {
                    await repoState.openRepo(url, outputBuffer: outputBuffer)
                    portfolioService.addRepo(url)
                    applyPendingWaveSelectionIfNeeded()
                }
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .selectPortfolioWave)) { notification in
            guard let targetRepoPath = notification.userInfo?["repoPath"] as? String,
                  let waveId = notification.userInfo?["waveId"] as? String else {
                return
            }

            let repoPath = repoURL?.standardizedFileURL.path(percentEncoded: false)
            guard repoPath == targetRepoPath else { return }
            pendingWaveSelection = waveId
            applyPendingWaveSelectionIfNeeded()
        }
    }

    private func applyPendingWaveSelectionIfNeeded() {
        guard hasOpenedRepo, let waveId = pendingWaveSelection else { return }
        NotificationCenter.default.post(
            name: .selectWave,
            object: nil,
            userInfo: ["waveId": waveId]
        )
        pendingWaveSelection = nil
    }
}
