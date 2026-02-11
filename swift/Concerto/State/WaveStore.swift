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
    var openPRCount: Int {
        pr.reduce(0) { total, wave in
            total + wave.effectiveOpenPRCount
        }
    }
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

    // Pending optimistic mutations — events skip these waves until committed
    private var pendingMutations: Set<String> = []

    // MARK: - Mutations

    func set(_ wave: WaveViewModel) {
        guard !pendingMutations.contains(wave.id) else { return }
        _set(wave)
    }

    private func _set(_ wave: WaveViewModel) {
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

        // Preserve all waves with pending mutations (optimistic inserts/deletes/edits)
        for id in pendingMutations {
            if let existing = waves[id] {
                updatedWaves[id] = existing
                updatedStatuses[id] = previousStatuses[id] ?? existing.status
            }
            // If not in waves (e.g. pending delete), stay absent
        }

        for wave in newWaves {
            if pendingMutations.contains(wave.id) { continue }
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

    // MARK: - Optimistic mutations

    func applyOptimistic(_ id: String, _ mutation: (inout WaveViewModel) -> Void) -> WaveViewModel? {
        guard var wave = waves[id] else { return nil }
        let snapshot = wave
        mutation(&wave)
        pendingMutations.insert(id)
        _set(wave)
        return snapshot
    }

    func commitMutation(_ id: String) {
        pendingMutations.remove(id)
    }

    func rollback(_ snapshot: WaveViewModel) {
        pendingMutations.remove(snapshot.id)
        _set(snapshot)
    }

    // MARK: - Pending create/delete

    func insertPending(_ wave: WaveViewModel) {
        pendingMutations.insert(wave.id)
        _set(wave)
    }

    func replacePending(_ pendingId: String, with wave: WaveViewModel) {
        pendingMutations.remove(pendingId)
        remove(pendingId)
        _set(wave)
    }

    func removePending(_ id: String) {
        pendingMutations.remove(id)
        remove(id)
    }

    func applyDelete(_ id: String) {
        pendingMutations.insert(id)
        remove(id)
    }

    // MARK: - Queries

    func wave(for id: String) -> WaveViewModel? { waves[id] }

    // MARK: - Private

    private func recompute() {
        let allWaves = Array(waves.values)

        let blocked = allWaves.filter { $0.status == .failed }
        let nonFailed = allWaves.filter { $0.status != .failed }
        let pr = nonFailed.filter(\.hasOpenPRs)
        let nonFailedWithoutPR = nonFailed.filter { !$0.hasOpenPRs }

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
