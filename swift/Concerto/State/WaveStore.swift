// WaveStore — dictionary-keyed wave state with derived groups and status tracking.

import Foundation
import LoopflowCore

struct WaveGroups {
    let blocked: [WaveViewModel]
    let pr: [WaveViewModel]
    let recentActivity: [WaveViewModel]
    let active: [WaveViewModel]
    let idle: [WaveViewModel]

    var attentionCount: Int { blocked.count + pr.count }
    var allInOrder: [WaveViewModel] { blocked + pr + recentActivity + active + idle }

    static let empty = WaveGroups(blocked: [], pr: [], recentActivity: [], active: [], idle: [])
}

@MainActor
@Observable
final class WaveStore {
    // Primary storage — dictionary keyed by wave ID
    private(set) var waves: [String: WaveViewModel] = [:] {
        didSet { recompute() }
    }

    // Derived state — recomputed on any change to waves
    private(set) var ordered: [WaveViewModel] = []
    private(set) var groups: WaveGroups = .empty

    // Status tracking for notifications
    private var previousStatuses: [String: WaveStatus] = [:]
    var onStatusChange: ((WaveViewModel, WaveStatus?, WaveStatus) -> Void)?

    // MARK: - Mutations

    func set(_ wave: WaveViewModel) {
        detectStatusChange(wave)
        waves[wave.id] = wave
    }

    func setAll(_ newWaves: [WaveViewModel]) {
        for wave in newWaves { detectStatusChange(wave) }
        waves = Dictionary(uniqueKeysWithValues: newWaves.map { ($0.id, $0) })
        previousStatuses = Dictionary(uniqueKeysWithValues: newWaves.map { ($0.id, $0.status) })
    }

    @discardableResult
    func remove(_ id: String) -> WaveViewModel? {
        let removed = waves.removeValue(forKey: id)
        previousStatuses.removeValue(forKey: id)
        return removed
    }

    func removeAll() {
        waves = [:]
        previousStatuses = [:]
    }

    // MARK: - Queries

    func wave(for id: String) -> WaveViewModel? { waves[id] }

    var isEmpty: Bool { waves.isEmpty }
    var count: Int { waves.count }

    // MARK: - Private

    private func detectStatusChange(_ wave: WaveViewModel) {
        let old = previousStatuses[wave.id]
        if old != wave.status {
            onStatusChange?(wave, old, wave.status)
        }
        previousStatuses[wave.id] = wave.status
    }

    private func recompute() {
        let allWaves = Array(waves.values)

        let blocked = allWaves.filter { $0.status == .failed }
        let pr = allWaves.filter { wave in
            wave.status != .failed && pendingPR(for: wave) != nil
        }

        let hourAgo = Date().addingTimeInterval(-3600)
        let recentActivity = Array(allWaves
            .filter { wave in
                guard let lastActivity = wave.lastActivityAt else { return false }
                return lastActivity > hourAgo && wave.status != .failed && pendingPR(for: wave) == nil
            }
            .sorted { ($0.lastActivityAt ?? .distantPast) > ($1.lastActivityAt ?? .distantPast) }
            .prefix(5))

        let recentIds = Set(recentActivity.map(\.id))

        let active = allWaves.filter { wave in
            (wave.status == .running || wave.status == .waiting)
                && pendingPR(for: wave) == nil
                && !recentIds.contains(wave.id)
        }

        let idle = allWaves.filter { wave in
            wave.status == .idle
                && pendingPR(for: wave) == nil
                && !recentIds.contains(wave.id)
        }

        groups = WaveGroups(
            blocked: blocked,
            pr: pr,
            recentActivity: recentActivity,
            active: active,
            idle: idle
        )
        ordered = groups.allInOrder
    }

    private func pendingPR(for wave: WaveViewModel) -> (number: Int, url: URL?)? {
        guard let prNumber = wave.prNumber, wave.prState == .open else {
            return nil
        }
        return (number: prNumber, url: wave.prURL)
    }
}
