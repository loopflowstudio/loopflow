import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

@MainActor
@Suite("Portfolio Repo State")
struct PortfolioRepoStateTests {
    @Test("summary metrics count blocked and diff totals")
    func summaryMetrics() {
        let repoURL = URL(fileURLWithPath: "/tmp/portfolio-state")
        let repo = PortfolioRepo(path: repoURL.normalizedFilePath, lastOpened: Date())
        let state = PortfolioRepoState(repo: repo, connection: .local, token: nil)

        state.applyConnectedWaves([
            makeWave(id: "running", repoPath: repo.path, status: .running, diffStat: " 1 files changed, 8 insertions(+), 2 deletions(-)"),
            makeWave(id: "waiting", repoPath: repo.path, status: .waiting, diffStat: " 2 files changed, 3 insertions(+), 7 deletions(-)"),
            makeWave(id: "failed", repoPath: repo.path, status: .failed, diffStat: " 1 files changed, 4 insertions(+), 0 deletions(-)"),
        ])

        #expect(state.blockedCount == 1)
        #expect(state.totalDiff.insertions == 15)
        #expect(state.totalDiff.deletions == 9)
        #expect(state.needsAttention)
    }

    @Test("wave events update and delete local waves")
    func waveEventsUpdateState() {
        let repoURL = URL(fileURLWithPath: "/tmp/portfolio-events")
        let repo = PortfolioRepo(path: repoURL.normalizedFilePath, lastOpened: Date())
        let state = PortfolioRepoState(repo: repo, connection: .local, token: nil)

        let createdWave = makeWave(id: "wave-1", repoPath: repo.path, status: .running, diffStat: nil)
        state.applyWaveEvent(
            WaveEvent(
                type: .created,
                waveId: createdWave.id,
                waveRunId: nil,
                step: nil,
                name: nil,
                wave: createdWave,
                timestamp: Date()
            )
        )

        #expect(state.waves.count == 1)

        state.applyWaveEvent(
            WaveEvent(
                type: .deleted,
                waveId: createdWave.id,
                waveRunId: nil,
                step: nil,
                name: nil,
                wave: nil,
                timestamp: Date()
            )
        )

        #expect(state.waves.isEmpty)
    }

    @Test("delete events with another repo do not remove local waves")
    func deleteEventFromDifferentRepoIsIgnored() {
        let repoURL = URL(fileURLWithPath: "/tmp/portfolio-events-local")
        let repo = PortfolioRepo(path: repoURL.normalizedFilePath, lastOpened: Date())
        let state = PortfolioRepoState(repo: repo, connection: .local, token: nil)

        let localWave = makeWave(id: "wave-1", repoPath: repo.path, status: .running, diffStat: nil)
        state.applyConnectedWaves([localWave])

        let otherRepoPath = URL(fileURLWithPath: "/tmp/portfolio-events-other").normalizedFilePath
        let remoteWave = makeWave(id: localWave.id, repoPath: otherRepoPath, status: .running, diffStat: nil)

        state.applyWaveEvent(
            WaveEvent(
                type: .deleted,
                waveId: localWave.id,
                waveRunId: nil,
                step: nil,
                name: nil,
                wave: remoteWave,
                timestamp: Date()
            )
        )

        #expect(state.waves.count == 1)
        #expect(state.waves[0].id == localWave.id)
    }

    private func makeWave(id: String, repoPath: String, status: WaveStatus, diffStat: String?) -> Wave {
        Wave(
            id: id,
            name: id,
            repo: repoPath,
            flow: "ship",
            direction: [],
            area: ["."],
            stimuli: [],
            status: status,
            iteration: 0,
            diffStat: diffStat
        )
    }
}
