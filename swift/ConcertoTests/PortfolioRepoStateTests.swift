import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

@MainActor
@Suite("Portfolio Repo State")
struct PortfolioRepoStateTests {
    @Test("refresh registers repo before loading waves")
    func refreshRegistersRepo() async {
        let repoURL = URL(fileURLWithPath: "/tmp/portfolio-refresh")
        let repo = PortfolioRepo(path: repoURL.normalizedFilePath, lastOpened: Date())
        let service = MockWaveService()
        service.waves = [makeWave(id: "wave-1", repoPath: repo.path, status: .running, diffStat: nil)]
        let state = PortfolioRepoState(repo: repo, waveService: service)

        await state.refresh()

        #expect(service.addedRepoPaths == [repo.path])
        #expect(service.listWavesTargets.map(\.path) == [repo.path])
        #expect(state.isConnected)
        #expect(state.waves.count == 1)
    }

    // Guards #2 (portfolio blind to Asana) + the normalizedFilePath lesson:
    // a discovered repoPath with a trailing slash must still scope to this
    // repo and count toward the card.
    @Test("refresh counts unmanaged Asana waves with path normalization")
    func refreshCountsDiscoveredWaves() async {
        let repoURL = URL(fileURLWithPath: "/tmp/portfolio-discovered")
        let repo = PortfolioRepo(path: repoURL.normalizedFilePath, lastOpened: Date())
        let service = MockWaveService()
        service.discoveredWaves = [
            DiscoveredWaveSummary(
                repoPath: repo.path + "/",
                repoId: "owner/repo",
                waveName: "Desktop",
                provider: "asana",
                asanaProjectId: "p1",
                managedWaveId: nil
            )
        ]
        let state = PortfolioRepoState(repo: repo, waveService: service)

        await state.refresh()

        #expect(state.waves.isEmpty)
        #expect(!state.hasNoWaves)
        #expect(state.totalWaveCount == 1)
        #expect(state.unmanagedDiscoveredWaves.map(\.waveName) == ["Desktop"])
    }

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
                sessionId: nil,
                initialUserMessage: nil,
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
                sessionId: nil,
                initialUserMessage: nil,
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
                sessionId: nil,
                initialUserMessage: nil,
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
            flow: "build",
            direction: [],
            area: ["."],
            triggers: [],
            status: status,
            iteration: 0,
            diffStat: diffStat
        )
    }
}

private final class MockWaveService: WaveServiceProtocol, @unchecked Sendable {
    var addedRepoPaths: [String] = []
    var listWavesTargets: [RepoTarget] = []
    var waves: [Wave] = []
    var discoveredWaves: [DiscoveredWaveSummary] = []

    func listWaves(repo: RepoTarget) async throws -> [Wave] {
        listWavesTargets.append(repo)
        return waves
    }

    func addRepo(path: String) async throws -> RemoteRepo {
        addedRepoPaths.append(path)
        return RemoteRepo(path: path, name: URL(fileURLWithPath: path).lastPathComponent, waveCount: 0)
    }

    func getWave(_ id: String) async throws -> Wave { fatalError("unused") }
    func createWave(name: String, repo: RepoTarget) async throws -> Wave { fatalError("unused") }
    func createWave(
        name: String,
        repo: RepoTarget,
        flow: String,
        run: Bool,
        status: WaveStatus?
    ) async throws -> Wave { fatalError("unused") }
    func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave { fatalError("unused") }
    func deleteWave(_ id: String) async throws { fatalError("unused") }
    func cloneWave(_ id: String, name: String?) async throws -> Wave { fatalError("unused") }
    func run(_ id: String, overrides: RunOverrides?) async throws { fatalError("unused") }
    func addTrigger(_ waveId: String, signal: Trigger.Signal, flow: String?) async throws -> Trigger {
        fatalError("unused")
    }
    func removeTrigger(_ waveId: String, triggerId: String) async throws { fatalError("unused") }
    func createSession(
        harness: String,
        waveRunId: String?,
        config: AgentSessionConfig
    ) async throws -> AgentSession { fatalError("unused") }
    func getSession(_ id: String) async throws -> AgentSession { fatalError("unused") }
    func sendSessionInput(sessionId: String, content: String) async throws -> AgentSession {
        fatalError("unused")
    }
    func streamSessionEvents(
        sessionId: String,
        afterSeq: Int?
    ) -> AsyncThrowingStream<AgentSessionEventEnvelope, Error> { fatalError("unused") }
    func stopSession(_ id: String) async throws -> AgentSession { fatalError("unused") }
    func stop(_ id: String) async throws { fatalError("unused") }
    func restartStep(_ id: String) async throws { fatalError("unused") }
    func listAttention(repo: RepoTarget) async throws -> [AttentionItem] { fatalError("unused") }
    func getAttention(_ id: String) async throws -> AttentionItem { fatalError("unused") }
    func markAttentionViewed(_ id: String) async throws -> AttentionItem { fatalError("unused") }
    func listTerminalSessions(repo: RepoTarget, activeOnly: Bool) async throws -> [TerminalSession] {
        fatalError("unused")
    }
    func getTerminalSession(_ id: String) async throws -> TerminalSession { fatalError("unused") }
    func attachTerminalSession(_ id: String) async throws -> TerminalConnectionInfo {
        fatalError("unused")
    }
    func startTerminalSession(_ id: String) async throws -> TerminalSession { fatalError("unused") }
    func cancelTerminalSession(_ id: String) async throws -> TerminalSession { fatalError("unused") }
    func landWave(_ id: String) async throws { fatalError("unused") }
    func nextWave(_ id: String) async throws -> String { fatalError("unused") }
    func listFlowsAndDirections(repo: RepoTarget) async throws -> WaveFlowsResult { fatalError("unused") }
    func listWorktrees(repo: RepoTarget) async throws -> [WorktreeInfo] { fatalError("unused") }
    func listRepos() async throws -> [RemoteRepo] { fatalError("unused") }
    func removeRepo(path: String) async throws { fatalError("unused") }
    func checkConnection() async throws { fatalError("unused") }
    func fileDiff(waveId: String, path: String) async throws -> String { fatalError("unused") }
    func deleteWaveItem(waveId: String, filename: String) async throws { fatalError("unused") }
    func fetchRoadmap(repo: String, wave: String) async throws -> RoadmapResponse? { fatalError("unused") }
    func listDiscoveredWaves() async throws -> [DiscoveredWaveSummary] { discoveredWaves }
    func usageSummary(
        filters: UsageAnalyticsFilters,
        groupBy: UsageGroupBy
    ) async throws -> UsageSummary { fatalError("unused") }
    func usageTimeseries(
        filters: UsageAnalyticsFilters,
        bucket: UsageTimeBucket,
        groupBy: UsageGroupBy
    ) async throws -> UsageTimeseries { fatalError("unused") }
}
