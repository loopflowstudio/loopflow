// Tests for WaveStore optimistic mutations and rollback.

import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

@MainActor
@Suite("WaveStore Optimistic Mutations")
struct WaveStoreOptimisticTests {
    private func makeWave(
        id: String = "wave-1",
        name: String = "original",
        status: WaveStatus = .idle,
        flow: String = "build",
        area: [String] = ["src/"],
        direction: [String] = [],
        openPRCount: Int = 0,
        prNumber: Int? = nil,
        prState: PRState? = nil
    ) -> WaveViewModel {
        WaveViewModel(
            api: Wave(
                id: id,
                name: name,
                repo: "/tmp/repo",
                flow: flow,
                direction: direction,
                area: area,
                triggers: [],
                status: status,
                iteration: 0,
                openPRCount: openPRCount
            ),
            prNumber: prNumber,
            prState: prState
        )
    }

    @Test("applyOptimistic updates wave and returns snapshot with old values")
    func applyOptimisticUpdatesAndReturnsSnapshot() {
        let store = WaveStore()
        store.set(makeWave(name: "original"))

        let snapshot = store.applyOptimistic("wave-1") { $0.name = "renamed" }

        #expect(snapshot?.name == "original")
        #expect(store.wave(for: "wave-1")?.name == "renamed")
    }

    @Test("rollback restores snapshot exactly")
    func rollbackRestoresSnapshot() {
        let store = WaveStore()
        store.set(makeWave(name: "original"))

        let snapshot = store.applyOptimistic("wave-1") { $0.name = "renamed" }
        #expect(store.wave(for: "wave-1")?.name == "renamed")

        store.rollback(snapshot!)
        #expect(store.wave(for: "wave-1")?.name == "original")
    }

    @Test("applyOptimistic returns nil for missing wave ID")
    func applyOptimisticReturnsNilForMissing() {
        let store = WaveStore()

        let snapshot = store.applyOptimistic("nonexistent") { $0.name = "renamed" }

        #expect(snapshot == nil)
    }

    @Test("groups recompute after optimistic status change")
    func groupsRecomputeAfterStatusChange() {
        let store = WaveStore()
        store.set(makeWave(status: .idle))

        #expect(store.groups.idle.count == 1)
        #expect(store.groups.active.count == 0)

        _ = store.applyOptimistic("wave-1") { $0.status = .running }

        #expect(store.groups.idle.count == 0)
        #expect(store.groups.active.count == 1)
    }

    @Test("applyOptimistic with multiple fields updates all")
    func applyOptimisticMultipleFields() {
        let store = WaveStore()
        store.set(makeWave(flow: "build", area: ["src/"], direction: []))

        let snapshot = store.applyOptimistic("wave-1") { w in
            w.flow = "debug"
            w.area = ["lib/"]
            w.direction = ["ux"]
        }

        let wave = store.wave(for: "wave-1")!
        #expect(wave.flow == "debug")
        #expect(wave.area == ["lib/"])
        #expect(wave.direction == ["ux"])

        #expect(snapshot?.flow == "build")
        #expect(snapshot?.area == ["src/"])
        #expect(snapshot?.direction == [])
    }

    @Test("rollback after multiple field mutation restores all fields")
    func rollbackRestoresAllFields() {
        let store = WaveStore()
        store.set(makeWave(name: "original", flow: "build", area: ["src/"]))

        let snapshot = store.applyOptimistic("wave-1") { w in
            w.name = "renamed"
            w.flow = "debug"
            w.area = ["lib/"]
        }

        store.rollback(snapshot!)

        let wave = store.wave(for: "wave-1")!
        #expect(wave.name == "original")
        #expect(wave.flow == "build")
        #expect(wave.area == ["src/"])
    }

    @Test("status change notification fires on optimistic mutation")
    func statusChangeNotificationFires() {
        let store = WaveStore()
        store.set(makeWave(status: .idle))

        var notifiedNew: WaveStatus?
        var notifiedOld: WaveStatus?
        store.onStatusChange = { _, old, new in
            notifiedOld = old
            notifiedNew = new
        }

        _ = store.applyOptimistic("wave-1") { $0.status = .running }

        #expect(notifiedOld == .idle)
        #expect(notifiedNew == .running)
    }

    @Test("rollback fires status change notification back to original")
    func rollbackFiresStatusChange() {
        let store = WaveStore()
        store.set(makeWave(status: .idle))

        let snapshot = store.applyOptimistic("wave-1") { $0.status = .running }

        var notifiedNew: WaveStatus?
        store.onStatusChange = { _, _, new in
            notifiedNew = new
        }

        store.rollback(snapshot!)

        #expect(notifiedNew == .idle)
    }

    // MARK: - Pending mutations guard

    @Test("set skips waves with pending optimistic mutations")
    func setSkipsPendingMutations() {
        let store = WaveStore()
        store.set(makeWave(name: "original"))

        _ = store.applyOptimistic("wave-1") { $0.name = "optimistic" }

        // External set (simulating event arrival) should be blocked
        store.set(makeWave(name: "from-event"))

        #expect(store.wave(for: "wave-1")?.name == "optimistic")
    }

    @Test("commitMutation allows subsequent set calls through")
    func commitMutationUnblocks() {
        let store = WaveStore()
        store.set(makeWave(name: "original"))

        _ = store.applyOptimistic("wave-1") { $0.name = "optimistic" }
        store.commitMutation("wave-1")

        store.set(makeWave(name: "from-event"))

        #expect(store.wave(for: "wave-1")?.name == "from-event")
    }

    @Test("rollback clears pending mutation state")
    func rollbackClearsPendingState() {
        let store = WaveStore()
        store.set(makeWave(name: "original"))

        let snapshot = store.applyOptimistic("wave-1") { $0.name = "optimistic" }

        // Set blocked while pending
        store.set(makeWave(name: "blocked"))
        #expect(store.wave(for: "wave-1")?.name == "optimistic")

        store.rollback(snapshot!)

        // After rollback, set should work again
        store.set(makeWave(name: "after-rollback"))
        #expect(store.wave(for: "wave-1")?.name == "after-rollback")
    }

    @Test("setAll preserves optimistic state for pending waves")
    func setAllPreservesPendingState() {
        let store = WaveStore()
        store.set(makeWave(id: "wave-1", name: "original"))
        store.set(makeWave(id: "wave-2", name: "other"))

        _ = store.applyOptimistic("wave-1") { $0.name = "optimistic" }

        // Simulate full refresh from connected event
        store.setAll([
            makeWave(id: "wave-1", name: "server-name"),
            makeWave(id: "wave-2", name: "other-updated"),
        ])

        #expect(store.wave(for: "wave-1")?.name == "optimistic")
        #expect(store.wave(for: "wave-2")?.name == "other-updated")
    }

    @Test("waves with open PRs appear in idle group when idle")
    func wavesWithOpenPRsGroupedByStatus() {
        let store = WaveStore()
        store.setAll([
            makeWave(id: "wave-1", openPRCount: 2),
            makeWave(id: "wave-2", prNumber: 12, prState: .open),
            makeWave(id: "wave-3"),
        ])

        // All three are idle status, so all land in idle group
        #expect(store.groups.idle.count == 3)
        #expect(store.groups.active.count == 0)
    }

    // MARK: - insertPending / replacePending / removePending

    @Test("insertPending adds wave and blocks set for that ID")
    func insertPendingBlocksSet() {
        let store = WaveStore()
        let pending = makeWave(id: "pending-1", name: "new wave")
        store.insertPending(pending)

        #expect(store.wave(for: "pending-1")?.name == "new wave")

        // External set should be blocked
        store.set(makeWave(id: "pending-1", name: "from-event"))
        #expect(store.wave(for: "pending-1")?.name == "new wave")
    }

    @Test("replacePending swaps pending for real wave and unblocks set")
    func replacePendingSwapsAndUnblocks() {
        let store = WaveStore()
        store.insertPending(makeWave(id: "pending-1", name: "pending"))

        let real = makeWave(id: "real-1", name: "server wave")
        store.replacePending("pending-1", with: real)

        #expect(store.wave(for: "pending-1") == nil)
        #expect(store.wave(for: "real-1")?.name == "server wave")

        // set should work for the real ID (not pending-guarded)
        store.set(makeWave(id: "real-1", name: "updated"))
        #expect(store.wave(for: "real-1")?.name == "updated")
    }

    @Test("removePending removes wave and clears pending state")
    func removePendingClearsAll() {
        let store = WaveStore()
        store.insertPending(makeWave(id: "pending-1", name: "pending"))

        store.removePending("pending-1")

        #expect(store.wave(for: "pending-1") == nil)

        // ID should no longer be pending-guarded
        store.set(makeWave(id: "pending-1", name: "reused"))
        #expect(store.wave(for: "pending-1")?.name == "reused")
    }

    // MARK: - applyDelete

    @Test("applyDelete removes wave but blocks set for that ID")
    func applyDeleteBlocksReInsertion() {
        let store = WaveStore()
        store.set(makeWave(id: "wave-1", name: "original"))

        store.applyDelete("wave-1")

        #expect(store.wave(for: "wave-1") == nil)

        // WebSocket event should not re-insert
        store.set(makeWave(id: "wave-1", name: "from-event"))
        #expect(store.wave(for: "wave-1") == nil)
    }

    @Test("applyDelete + rollback restores the wave")
    func applyDeleteRollbackRestores() {
        let store = WaveStore()
        let wave = makeWave(id: "wave-1", name: "original")
        store.set(wave)

        store.applyDelete("wave-1")
        #expect(store.wave(for: "wave-1") == nil)

        store.rollback(wave)
        #expect(store.wave(for: "wave-1")?.name == "original")

        // set should work again after rollback
        store.set(makeWave(id: "wave-1", name: "updated"))
        #expect(store.wave(for: "wave-1")?.name == "updated")
    }

    @Test("setAll during pending delete does not re-insert the wave")
    func setAllDuringPendingDelete() {
        let store = WaveStore()
        store.set(makeWave(id: "wave-1", name: "original"))
        store.set(makeWave(id: "wave-2", name: "other"))

        store.applyDelete("wave-1")

        store.setAll([
            makeWave(id: "wave-1", name: "server-name"),
            makeWave(id: "wave-2", name: "other-updated"),
        ])

        // wave-1 should stay deleted (pending guard blocks it)
        #expect(store.wave(for: "wave-1") == nil)
        #expect(store.wave(for: "wave-2")?.name == "other-updated")
    }

    @Test("applyDelete + commitMutation allows future set")
    func applyDeleteCommitAllowsFutureSet() {
        let store = WaveStore()
        store.set(makeWave(id: "wave-1", name: "original"))

        store.applyDelete("wave-1")
        store.commitMutation("wave-1")

        // After commit, a new wave with the same ID can be set
        store.set(makeWave(id: "wave-1", name: "new"))
        #expect(store.wave(for: "wave-1")?.name == "new")
    }

    @Test("setAll preserves pending-inserted waves not in server list")
    func setAllPreservesPendingInsert() {
        let store = WaveStore()
        store.set(makeWave(id: "wave-1", name: "existing"))
        store.insertPending(makeWave(id: "pending-1", name: "pending wave"))

        // Server refresh doesn't include the pending wave
        store.setAll([
            makeWave(id: "wave-1", name: "refreshed"),
        ])

        #expect(store.wave(for: "wave-1")?.name == "refreshed")
        #expect(store.wave(for: "pending-1")?.name == "pending wave")
    }

    // MARK: - Responsive actions (optimistic status)

    @Test("optimistic run sets status to running, blocks events")
    func optimisticRunSetsRunning() {
        let store = WaveStore()
        store.set(makeWave(status: .idle))

        let snapshot = store.applyOptimistic("wave-1") { $0.status = .running }

        #expect(store.wave(for: "wave-1")?.status == .running)
        #expect(snapshot?.status == .idle)

        // Event with old status should be blocked
        store.set(makeWave(status: .idle))
        #expect(store.wave(for: "wave-1")?.status == .running)

        // After commit, events flow through
        store.commitMutation("wave-1")
        store.set(makeWave(status: .running))
        #expect(store.wave(for: "wave-1")?.status == .running)
    }

    @Test("optimistic stop sets status to idle, rollback restores running")
    func optimisticStopSetsIdle() {
        let store = WaveStore()
        store.set(makeWave(status: .running))

        let snapshot = store.applyOptimistic("wave-1") { $0.status = .idle }

        #expect(store.wave(for: "wave-1")?.status == .idle)
        #expect(store.groups.idle.count == 1)
        #expect(store.groups.active.count == 0)

        // Rollback restores running
        store.rollback(snapshot!)
        #expect(store.wave(for: "wave-1")?.status == .running)
        #expect(store.groups.active.count == 1)
        #expect(store.groups.idle.count == 0)
    }

    @Test("optimistic next sets status to idle")
    func optimisticNextSetsIdle() {
        let store = WaveStore()
        store.set(makeWave(status: .waiting))

        _ = store.applyOptimistic("wave-1") { $0.status = .idle }

        #expect(store.wave(for: "wave-1")?.status == .idle)
    }

    @Test("event overwrites optimistic status after commit")
    func eventOverwritesAfterCommit() {
        let store = WaveStore()
        store.set(makeWave(status: .idle))

        _ = store.applyOptimistic("wave-1") { $0.status = .running }
        store.commitMutation("wave-1")

        // WebSocket event arrives with actual status
        store.set(makeWave(status: .waiting))
        #expect(store.wave(for: "wave-1")?.status == .waiting)
    }
}
