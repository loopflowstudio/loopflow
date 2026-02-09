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
}
