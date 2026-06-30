import Foundation
import Testing
@testable import LoopflowCore

@MainActor
@Suite("RepoState snapshot scope")
struct RepoStateSnapshotScopeTests {
    @Test("connected snapshot keeps only the window's repo waves")
    func snapshotScopedToRepo() {
        let repoURL = URL(fileURLWithPath: "/tmp/repo-a")
        let state = RepoState()
        state.repoTarget = .local(repoURL)

        let mine = makeWave(id: "mine", repoPath: repoURL.normalizedFilePath)
        let other = makeWave(
            id: "other",
            repoPath: URL(fileURLWithPath: "/tmp/repo-b").normalizedFilePath
        )

        // The bundled daemon hands back both repos' waves; only this repo's survive.
        state.applyConnectedSnapshot([mine, other])

        #expect(state.waves.count == 1)
        #expect(state.waves.first?.id == "mine")
    }

    private func makeWave(id: String, repoPath: String) -> Wave {
        Wave(
            id: id,
            name: id,
            repo: repoPath,
            flow: "build",
            direction: [],
            area: ["."],
            triggers: [],
            status: .running,
            iteration: 0,
            diffStat: nil
        )
    }
}
