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
        flow: String = "ship",
        area: [String] = ["src/"],
        direction: [String] = []
    ) -> WaveViewModel {
        WaveViewModel(
            api: Wave(
                id: id,
                name: name,
                repo: "/tmp/repo",
                flow: flow,
                direction: direction,
                area: area,
                stimulus: Stimulus(kind: .once),
                status: status,
                iteration: 0
            )
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
        store.set(makeWave(flow: "ship", area: ["src/"], direction: []))

        let snapshot = store.applyOptimistic("wave-1") { w in
            w.flow = "debug"
            w.area = ["lib/"]
            w.direction = ["designer"]
        }

        let wave = store.wave(for: "wave-1")!
        #expect(wave.flow == "debug")
        #expect(wave.area == ["lib/"])
        #expect(wave.direction == ["designer"])

        #expect(snapshot?.flow == "ship")
        #expect(snapshot?.area == ["src/"])
        #expect(snapshot?.direction == [])
    }

    @Test("rollback after multiple field mutation restores all fields")
    func rollbackRestoresAllFields() {
        let store = WaveStore()
        store.set(makeWave(name: "original", flow: "ship", area: ["src/"]))

        let snapshot = store.applyOptimistic("wave-1") { w in
            w.name = "renamed"
            w.flow = "debug"
            w.area = ["lib/"]
        }

        store.rollback(snapshot!)

        let wave = store.wave(for: "wave-1")!
        #expect(wave.name == "original")
        #expect(wave.flow == "ship")
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
}
