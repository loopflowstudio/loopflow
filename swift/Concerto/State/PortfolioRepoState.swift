// Lightweight per-repo state for the portfolio dashboard.

import Foundation
import LoopflowCore

@MainActor
@Observable
final class PortfolioRepoState {
    let repo: PortfolioRepo

    private let repoPath: String
    private let waveService: WaveService
    /// Local discovery via `lf ls` (see `RegistryQuery`). `nil` on a platform
    /// that can't shell `lf`, or for a remote repo — those fall back to the REST
    /// wave list.
    private let registryQuery: RegistryQuery?

    private(set) var waves: [WaveViewModel] = []
    private(set) var isConnected = false
    private(set) var isLoading = true

    init(
        repo: PortfolioRepo,
        connection: ServerConnection,
        token: String?,
        registryQuery: RegistryQuery? = nil
    ) {
        self.repo = repo
        self.repoPath = repo.path.normalizedFilePath
        self.waveService = WaveService(connection: connection, tokenProvider: { token })
        self.registryQuery = registryQuery
    }

    /// Create a wave file-first: a wave IS its markdown. Write
    /// `wave/<name>/GOAL.md` (+ an empty MEMORY.md) into the repo — the same
    /// shape the registry overlays when the wave is started (`lf wave <name>`,
    /// the Start button). No POST; the row is not the wave.
    ///
    /// Wave state lives at the ORIGIN repo: every reader (endpoint discovery,
    /// launcher, session probe) resolves a worktree to its main checkout, so
    /// the write must land there too — a GOAL.md written into a worktree is a
    /// wave no reader ever finds.
    func createWave(name: String) async throws {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw WaveServiceError.commandFailed("Wave name is required")
        }

        let repoURL = repo.url
        try await Task.detached {
            let waveDir = URL(fileURLWithPath: WaveOrigin.resolve(repoURL.path), isDirectory: true)
                .appendingPathComponent("wave", isDirectory: true)
                .appendingPathComponent(trimmed, isDirectory: true)
            let goalURL = waveDir.appendingPathComponent("GOAL.md", isDirectory: false)
            guard !FileManager.default.fileExists(atPath: goalURL.path) else {
                throw WaveServiceError.commandFailed("Wave '\(trimmed)' already exists")
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
    /// via `RegistryQuery` when a local runner is wired, else the surviving REST
    /// wave list. The dashboard re-runs this on a cadence; a wave's live motion
    /// rides its own per-wave SSE in the detail pane.
    func refresh() async {
        isLoading = true
        defer { isLoading = false }

        do {
            let loaded: [Wave]
            if let registryQuery {
                loaded = try await registryQuery.waves(repoPath: repo.url.path)
            } else {
                loaded = try await waveService.listWaves(repo: .local(repo.url))
            }
            applyConnectedWaves(loaded)
            isConnected = true
        } catch {
            isConnected = false
        }
    }

    func applyConnectedWaves(_ connectedWaves: [Wave]) {
        let filtered = connectedWaves
            .filter { wave in wave.repos.contains { $0.repo.normalizedFilePath == repoPath } }
            .map { WaveViewModel(api: $0) }
        waves = Self.sortWaves(filtered)
    }

    var blockedCount: Int {
        waves.filter { $0.status == .waiting }.count
    }

    var totalDiff: (insertions: Int, deletions: Int) {
        waves.reduce(into: (insertions: 0, deletions: 0)) { partial, wave in
            let stat = Self.parseDiffStat(wave.diffStat)
            partial.insertions += stat.insertions
            partial.deletions += stat.deletions
        }
    }

    func diffSummary(for wave: WaveViewModel) -> String? {
        let summary = Self.parseDiffStat(wave.diffStat)
        guard summary.insertions > 0 || summary.deletions > 0 else {
            return nil
        }
        return "+\(summary.insertions) -\(summary.deletions)"
    }

    private static func sortWaves(_ waves: [WaveViewModel]) -> [WaveViewModel] {
        waves.sorted { lhs, rhs in
            let lhsPriority = statusPriority(lhs.status)
            let rhsPriority = statusPriority(rhs.status)
            if lhsPriority != rhsPriority {
                return lhsPriority < rhsPriority
            }
            return lhs.displayName.localizedCaseInsensitiveCompare(rhs.displayName) == .orderedAscending
        }
    }

    private static func statusPriority(_ status: WaveStatus) -> Int {
        switch status {
        case .running: 0
        case .waiting: 1
        case .failed: 2
        case .paused: 3
        case .idle: 4
        }
    }

    private static func parseDiffStat(_ diffStat: String?) -> (insertions: Int, deletions: Int) {
        guard let diffStat else { return (0, 0) }
        return (
            extractCount(from: diffStat, pattern: #"(\d+)\s+insertions?\(\+\)"#),
            extractCount(from: diffStat, pattern: #"(\d+)\s+deletions?\(-\)"#)
        )
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

    private static func extractCount(from text: String, pattern: String) -> Int {
        guard let regex = try? NSRegularExpression(pattern: pattern) else { return 0 }
        let range = NSRange(text.startIndex..., in: text)
        guard let match = regex.firstMatch(in: text, range: range),
              let valueRange = Range(match.range(at: 1), in: text) else {
            return 0
        }
        return Int(text[valueRange]) ?? 0
    }
}
