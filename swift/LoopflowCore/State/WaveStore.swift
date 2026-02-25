// WaveStore — dictionary-keyed wave state with derived groups and status tracking.

import Foundation

public struct WaveGroups: Sendable {
    public let active: [WaveViewModel]
    public let idle: [WaveViewModel]

    public var allInOrder: [WaveViewModel] { active + idle }

    public static let empty = WaveGroups(active: [], idle: [])

    public init(active: [WaveViewModel], idle: [WaveViewModel]) {
        self.active = active
        self.idle = idle
    }
}

@MainActor
@Observable
public final class WaveStore {
    // Primary storage — dictionary keyed by wave ID
    public private(set) var waves: [String: WaveViewModel] = [:] {
        didSet { recompute() }
    }

    // Derived state — recomputed on any change to waves
    public private(set) var ordered: [WaveViewModel] = []
    public private(set) var groups: WaveGroups = .empty

    // Status tracking for notifications
    private var previousStatuses: [String: WaveStatus] = [:]
    public var onStatusChange: ((WaveViewModel, WaveStatus?, WaveStatus) -> Void)?

    // Pending optimistic mutations — events skip these waves until committed
    private var pendingMutations: Set<String> = []

    // MARK: - Mutations

    public init() {}

    public func set(_ wave: WaveViewModel) {
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

    public func setAll(_ newWaves: [WaveViewModel]) {
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
    public func remove(_ id: String) -> WaveViewModel? {
        let removed = waves.removeValue(forKey: id)
        previousStatuses.removeValue(forKey: id)
        return removed
    }

    public func removeAll() {
        waves = [:]
        previousStatuses = [:]
    }

    // MARK: - Optimistic mutations

    public func applyOptimistic(_ id: String, _ mutation: (inout WaveViewModel) -> Void) -> WaveViewModel? {
        guard var wave = waves[id] else { return nil }
        let snapshot = wave
        mutation(&wave)
        pendingMutations.insert(id)
        _set(wave)
        return snapshot
    }

    public func commitMutation(_ id: String) {
        pendingMutations.remove(id)
    }

    public func rollback(_ snapshot: WaveViewModel) {
        pendingMutations.remove(snapshot.id)
        _set(snapshot)
    }

    // MARK: - Pending create/delete

    public func insertPending(_ wave: WaveViewModel) {
        pendingMutations.insert(wave.id)
        _set(wave)
    }

    public func replacePending(_ pendingId: String, with wave: WaveViewModel) {
        pendingMutations.remove(pendingId)
        remove(pendingId)
        _set(wave)
    }

    public func removePending(_ id: String) {
        pendingMutations.remove(id)
        remove(id)
    }

    public func applyDelete(_ id: String) {
        pendingMutations.insert(id)
        remove(id)
    }

    // MARK: - Queries

    public func wave(for id: String) -> WaveViewModel? { waves[id] }

    // MARK: - Private

    private func recompute() {
        let allWaves = Array(waves.values)

        let active = allWaves.filter { $0.status != .idle }
        let idle = allWaves.filter { $0.status == .idle }

        groups = WaveGroups(active: active, idle: idle)
        ordered = groups.allInOrder
    }
}
