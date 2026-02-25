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

            await openRepoIfNeeded()
        }
        .onChange(of: setupComplete) { _, complete in
            if complete {
                Task {
                    await openRepoIfNeeded()
                }
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .selectPortfolioWave)) { notification in
            guard let targetRepoPath = notification.userInfo?["repoPath"] as? String,
                  let waveId = notification.userInfo?["waveId"] as? String else {
                return
            }

            let repoPath = repoURL?.normalizedFilePath
            guard repoPath == targetRepoPath else { return }
            pendingWaveSelection = waveId
            applyPendingWaveSelectionIfNeeded()
        }
    }

    private func openRepoIfNeeded() async {
        guard setupComplete, let url = repoURL, !hasOpenedRepo else { return }
        hasOpenedRepo = true
        await repoState.openRepo(url, outputBuffer: outputBuffer)
        portfolioService.addRepo(url)
        applyPendingWaveSelectionIfNeeded()
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
