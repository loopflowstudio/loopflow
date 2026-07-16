// Lightweight per-repo state for the portfolio dashboard.

import Loopflow
import Foundation

enum PortfolioRepoError: LocalizedError {
    case unknownRepo(String)

    var errorDescription: String? {
        switch self {
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
            let loaded = try await registryQuery.waves(repoPath: repo.url.path)
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

    /// The tmux session name for a wave. Named after the wave's ORIGIN repo
    /// (`WaveOrigin.resolve`, memoized): the launcher launches at the origin,
    /// so the rail's status probe and the attach hint must derive the same
    /// name from a worktree path — one resolution feeds probe, launcher,
    /// guard, and hint.
    nonisolated static func waveAgentSessionName(repoPath: String, waveName: String) -> String {
        let repoName = URL(fileURLWithPath: WaveOrigin.resolve(repoPath)).lastPathComponent
        return "lf-\(repoName)-\(sanitizeWavePathComponent(waveName))"
            .replacingOccurrences(of: ".", with: "-")
            .replacingOccurrences(of: ":", with: "-")
    }

    nonisolated static func waveAgentSessionExists(repoPath: String, waveName: String) -> Bool {
        LocalWaveAgentLauncher.sessionExists(repoPath: repoPath, waveName: waveName)
    }

    private nonisolated static func sanitizeWavePathComponent(_ value: String) -> String {
        var sanitized = ""
        var pendingDash = false
        for char in value {
            if char.isASCII && (char.isLetter || char.isNumber || char == "_" || char == "-") {
                if pendingDash && !sanitized.isEmpty {
                    sanitized.append("-")
                }
                pendingDash = false
                sanitized.append(char)
            } else {
                pendingDash = true
            }
        }
        return sanitized.isEmpty ? "wave" : sanitized
    }

}
