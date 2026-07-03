import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

@Suite("Repo wave list")
struct ContentViewTests {
    @Test("filters waves to the current repo")
    func filtersWavesToCurrentRepo() {
        let repoA = URL(fileURLWithPath: "/tmp/repo-a").normalizedFilePath
        let repoB = URL(fileURLWithPath: "/tmp/repo-b").normalizedFilePath

        let waves = [
            makeWave(id: "mine", repo: repoA),
            makeWave(id: "other", repo: repoB),
        ]

        let filtered = repoFilteredWaves(waves, currentRepoPath: repoA)

        #expect(filtered.map(\.id) == ["mine"])
    }

    @Test("returns no waves before a repo is open")
    func returnsEmptyWithoutCurrentRepo() {
        let waves = [makeWave(id: "mine", repo: "/tmp/repo-a")]

        #expect(repoFilteredWaves(waves, currentRepoPath: nil).isEmpty)
    }

    @Test("normalizes repo paths before filtering")
    func normalizesRepoPathsBeforeFiltering() {
        let waves = [
            makeWave(id: "mine", repo: "/tmp/repo-a/../repo-a"),
        ]

        let filtered = repoFilteredWaves(waves, currentRepoPath: "/tmp/repo-a")

        #expect(filtered.map(\.id) == ["mine"])
    }

    @Test("repo chip uses the repository directory name")
    func repoChipUsesRepositoryDirectoryName() {
        #expect(repoChipText(for: "/tmp/repo-a/../repo-a") == "repo-a")
    }

    private func makeWave(id: String, repo: String, status: WaveStatus = .running) -> WaveViewModel {
        WaveViewModel(
            api: Wave(
                id: id,
                name: id,
                repo: repo,
                flow: "build",
                status: status
            )
        )
    }
}
