// Lightweight per-repo state for the portfolio dashboard.

import Foundation
import LoopflowCore

@MainActor
@Observable
final class PortfolioRepoState {
    let repo: PortfolioRepo

    private let repoPath: String
    private let waveService: WaveService

    private(set) var waves: [WaveViewModel] = []
    private(set) var isConnected = false
    private(set) var isLoading = true

    init(repo: PortfolioRepo, connection: ServerConnection, token: String?) {
        self.repo = repo
        self.repoPath = repo.path.normalizedFilePath
        self.waveService = WaveService(connection: connection, tokenProvider: { token })
    }

    func refresh() async {
        isLoading = true
        defer { isLoading = false }

        do {
            let loaded = try await waveService.listWaves(repo: .local(repo.url))
            applyConnectedWaves(loaded)
            isConnected = true
        } catch {
            isConnected = false
        }
    }

    func applyConnectedWaves(_ connectedWaves: [Wave]) {
        let filtered = connectedWaves
            .filter { $0.repo.normalizedFilePath == repoPath }
            .map { WaveViewModel(api: $0) }
        waves = Self.sortWaves(filtered)
    }

    func applyWaveEvent(_ event: WaveEvent) {
        switch event.type {
        case .deleted:
            if let wave = event.wave,
               wave.repo.normalizedFilePath != repoPath {
                return
            }
            waves.removeAll { $0.id == event.waveId }
        case .created, .updated, .started, .stopped, .waiting:
            guard let wave = event.wave,
                  wave.repo.normalizedFilePath == repoPath else {
                return
            }
            upsertWave(WaveViewModel(api: wave))
        }
    }

    func updateConnectionState(_ state: ConnectionState) {
        isConnected = if case .connected = state { true } else { false }
        if !isConnected && waves.isEmpty {
            isLoading = false
        }
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

    var needsAttention: Bool {
        waves.contains { $0.status == .failed || $0.status == .waiting }
    }

    func diffSummary(for wave: WaveViewModel) -> String? {
        let summary = Self.parseDiffStat(wave.diffStat)
        guard summary.insertions > 0 || summary.deletions > 0 else {
            return nil
        }
        return "+\(summary.insertions) -\(summary.deletions)"
    }

    private func upsertWave(_ wave: WaveViewModel) {
        if let existingIndex = waves.firstIndex(where: { $0.id == wave.id }) {
            waves[existingIndex] = wave
        } else {
            waves.append(wave)
        }
        waves = Self.sortWaves(waves)
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
