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
        let oldStatus = previousStatuses[wave.id]
        if oldStatus != wave.status {
            onStatusChange?(wave, oldStatus, wave.status)
        }
        previousStatuses[wave.id] = wave.status
        waves[wave.id] = wave
    }

    func setAll(_ newWaves: [WaveViewModel]) {
        var updatedWaves: [String: WaveViewModel] = [:]
        var updatedStatuses: [String: WaveStatus] = [:]

        for wave in newWaves {
            let oldStatus = previousStatuses[wave.id]
            if oldStatus != wave.status {
                onStatusChange?(wave, oldStatus, wave.status)
            }
            updatedWaves[wave.id] = wave
            updatedStatuses[wave.id] = wave.status
        }

        waves = updatedWaves
        previousStatuses = updatedStatuses
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

    // MARK: - Private

    private func recompute() {
        let allWaves = Array(waves.values)

        let blocked = allWaves.filter { $0.status == .failed }
        let nonFailedWithoutPR = allWaves.filter { $0.status != .failed && $0.pendingPR == nil }
        let pr = allWaves.filter { $0.status != .failed && $0.pendingPR != nil }

        let hourAgo = Date().addingTimeInterval(-3600)
        let recentActivity = Array(nonFailedWithoutPR
            .filter { wave in
                guard let lastActivity = wave.lastActivityAt else { return false }
                return lastActivity > hourAgo
            }
            .sorted { ($0.lastActivityAt ?? .distantPast) > ($1.lastActivityAt ?? .distantPast) }
            .prefix(5))

        let recentIds = Set(recentActivity.map(\.id))

        let active = nonFailedWithoutPR.filter { wave in
            (wave.status == .running || wave.status == .waiting)
                && !recentIds.contains(wave.id)
        }

        let idle = nonFailedWithoutPR.filter { wave in
            wave.status == .idle
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
}
