// Lightweight per-repo state for the portfolio dashboard.

import Loopflow
import Foundation

enum PortfolioRepoError: LocalizedError {
    case emptyWaveName
    case duplicateWave(String)
    case unknownRepo(String)

    var errorDescription: String? {
        switch self {
        case .emptyWaveName:
            "Wave name is required"
        case .duplicateWave(let name):
            "Wave '\(name)' already exists"
        case .unknownRepo(let path):
            "Unknown repo: \(path)"
        }
    }
}

@MainActor
@Observable
final class PortfolioRepoState {
    let repo: PortfolioRepo

    private let repoPath: String
    /// Local discovery via `lf ls` (see `RegistryQuery`). `nil` means this
    /// platform cannot read the registry yet; there is no HTTP fallback.
    private let registryQuery: RegistryQuery?

    private(set) var waves: [WaveViewModel] = []
    private(set) var isConnected = false
    private(set) var isLoading = true
    private var plansByKey: [String: WavePlan] = [:]

    init(
        repo: PortfolioRepo,
        registryQuery: RegistryQuery? = nil
    ) {
        self.repo = repo
        self.repoPath = repo.path.normalizedFilePath
        self.registryQuery = registryQuery
    }

    /// Author a Wave before it is served. `lf start` later registers the
    /// coordination row; GOAL.md and MEMORY.md remain the authored objective
    /// and durable learning.
    ///
    /// Wave files live at the ORIGIN repo: every reader (endpoint discovery,
    /// launcher, session probe) resolves a worktree to its main checkout, so
    /// the write must land there too — a GOAL.md written into a worktree is a
    /// wave no reader ever finds.
    func createWave(name: String) async throws {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw PortfolioRepoError.emptyWaveName
        }

        let repoURL = repo.url
        try await Task.detached {
            let waveDir = URL(fileURLWithPath: WaveOrigin.resolve(repoURL.path), isDirectory: true)
                .appendingPathComponent("wave", isDirectory: true)
                .appendingPathComponent(trimmed, isDirectory: true)
            let goalURL = waveDir.appendingPathComponent("GOAL.md", isDirectory: false)
            guard !FileManager.default.fileExists(atPath: goalURL.path) else {
                throw PortfolioRepoError.duplicateWave(trimmed)
            }
            try FileManager.default.createDirectory(at: waveDir, withIntermediateDirectories: true)
            let goal = "Drive the '\(trimmed)' wave's goal forward.\n"
            try goal.write(to: goalURL, atomically: true, encoding: .utf8)
            let memoryURL = waveDir.appendingPathComponent("MEMORY.md", isDirectory: false)
            try "".write(to: memoryURL, atomically: true, encoding: .utf8)
        }.value

        await refresh()
    }

    /// Re-read this repo's waves. Discovery is a query, not a stream: `lf ls`
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
            // Scope only after resolving worktrees to their origin. Asking
            // RegistryQuery.waves(repoPath:) to exact-filter first discards the
            // canonical registry rows a development worktree needs to see.
            let loaded = try await registryQuery.allWaves()
            applyConnectedWaves(loaded, plans: plansByKey)
            isConnected = true
        } catch {
            isConnected = false
        }
    }

    func applyConnectedWaves(_ connectedWaves: [Wave], plans: [String: WavePlan] = [:]) {
        plansByKey = plans
        let origin = WaveOrigin.resolve(repoPath).normalizedFilePath
        let filtered = connectedWaves
            .filter { WaveOrigin.resolve($0.repo).normalizedFilePath == origin }
            .map { wave in
                WaveViewModel(
                    api: wave,
                    plan: plans[Self.wavePlanKey(repoPath: wave.repo, waveName: wave.name)]
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

    nonisolated static func wavePlanKey(repoPath: String, waveName: String) -> String {
        "\(repoPath.normalizedFilePath)#\(waveName)"
    }

    /// One stable alphabetical list. The row's lens carries state, so rows never
    /// reorder as processes start and stop.
    private static func sortWaves(_ waves: [WaveViewModel]) -> [WaveViewModel] {
        waves.sorted {
            $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending
        }
    }

}
