// Lightweight per-repo state for the portfolio dashboard.

import Loopflow
import Foundation

@MainActor
@Observable
final class PortfolioRepoState {
    let repo: PortfolioRepo

    private let repoPath: String
    /// Local discovery via `lf wave list` (see `RegistryQuery`). `nil` means this
    /// platform cannot read the registry yet; there is no HTTP fallback.
    private let registryQuery: RegistryQuery?

    private(set) var waves: [WaveViewModel] = []
    private(set) var isConnected = false
    private(set) var isLoading = true
    private var plansByWaveId: [String: WavePlan] = [:]

    init(
        repo: PortfolioRepo,
        registryQuery: RegistryQuery? = nil
    ) {
        self.repo = repo
        self.repoPath = repo.path.normalizedFilePath
        self.registryQuery = registryQuery
    }

    /// Re-read this repo's waves. Discovery is a query, not a stream: `lf wave list`
    /// via `RegistryQuery`. The dashboard re-runs this on a cadence; a wave's
    /// live motion rides its own per-wave SSE in the detail pane.
    func refresh() async {
        isLoading = true
        defer { isLoading = false }

        guard let registryQuery else {
            waves = []
            isConnected = false
            return
        }

        do {
            // Scope only after resolving worktrees to their origin: an exact
            // path filter would discard the canonical registry rows a
            // development worktree needs to see.
            let loaded = try await registryQuery.allWaves()
            applyConnectedWaves(loaded, plans: plansByWaveId)
            isConnected = true
        } catch {
            isConnected = false
        }
    }

    func applyConnectedWaves(_ connectedWaves: [Wave], plans: [String: WavePlan] = [:]) {
        plansByWaveId = plans
        let origin = WaveOrigin.resolve(repoPath).normalizedFilePath
        let filtered = connectedWaves
            .filter { WaveOrigin.resolve($0.repo).normalizedFilePath == origin }
            .map { wave in
                WaveViewModel(
                    api: wave,
                    plan: plans[wave.id]
                )
            }
        waves = Self.sortWaves(filtered)
        isConnected = true
        isLoading = false
    }

    func markRefreshFailed() {
        isConnected = false
        isLoading = false
    }

    /// One stable alphabetical list. The row's lens carries state, so rows never
    /// reorder as processes start and stop.
    private static func sortWaves(_ waves: [WaveViewModel]) -> [WaveViewModel] {
        waves.sorted {
            $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending
        }
    }

}
