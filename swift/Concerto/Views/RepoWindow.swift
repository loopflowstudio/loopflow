// Window wrapper for a single repository.
// Each repo window has its own state instances.

import SwiftUI
import LoopflowCore

struct RepoWindow: View {
    let repoURL: URL?
    let portfolioService: PortfolioService

    @State private var repoState = RepoState()
    @State private var outputBuffer = OutputBuffer()
    @State private var openedRepoPath: String?
    @State private var pendingWaveSelection: String?

    var body: some View {
        ContentView()
            .environment(repoState)
            .environment(outputBuffer)
        .background(WindowAccessor { window in
            window?.representedURL = repoURL
        })
        .task {
            if let mode = RepoState.uiTestMode() {
                let url = repoURL ?? URL(fileURLWithPath: "/tmp/loopflow-ui-tests")
                openedRepoPath = canonicalPath(url)
                repoState.configureForUITest(mode, repoURL: url)
                return
            }

            await openRepoIfNeeded()
        }
        .onChange(of: repoURL) { _, _ in
            Task {
                await openRepoIfNeeded()
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
        .onDisappear {
            repoState.closeRepo()
            openedRepoPath = nil
        }
    }

    private func openRepoIfNeeded() async {
        guard let url = repoURL else { return }
        let canonical = canonicalPath(url)
        guard openedRepoPath != canonical else { return }
        openedRepoPath = canonical
        await repoState.openRepo(url, outputBuffer: outputBuffer)
        portfolioService.addRepo(url)
        applyPendingWaveSelectionIfNeeded()
    }

    private func applyPendingWaveSelectionIfNeeded() {
        guard openedRepoPath != nil, let waveId = pendingWaveSelection else { return }
        NotificationCenter.default.post(
            name: .selectWave,
            object: nil,
            userInfo: ["waveId": waveId]
        )
        pendingWaveSelection = nil
    }

    private func canonicalPath(_ url: URL) -> String {
        url.resolvingSymlinksInPath().standardizedFileURL.path
    }
}
