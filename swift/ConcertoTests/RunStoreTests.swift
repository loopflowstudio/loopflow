// Tests for RunStore caching behavior.

import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

@MainActor
@Suite("RunStore")
struct RunStoreTests {
    private func makeRun(
        id: String = "run-1",
        waveId: String = "wave-1",
        status: WaveRunStatus = .completed,
        iteration: Int = 1
    ) -> WaveRun {
        WaveRun(
            id: id,
            waveId: waveId,
            flow: "ship",
            area: "src/",
            repo: "/tmp/repo",
            status: status,
            iteration: iteration
        )
    }

    @Test("setRuns replaces all runs for a wave")
    func setRunsReplaces() {
        let store = RunStore()
        store.setRuns(for: "wave-1", [makeRun(id: "run-1"), makeRun(id: "run-2")])

        #expect(store.runs(for: "wave-1").count == 2)

        store.setRuns(for: "wave-1", [makeRun(id: "run-3")])

        #expect(store.runs(for: "wave-1").count == 1)
        #expect(store.runs(for: "wave-1").first?.id == "run-3")
    }

    @Test("setRuns caps at 50 runs")
    func setRunsCaps() {
        let store = RunStore()
        let runs = (0..<60).map { makeRun(id: "run-\($0)") }
        store.setRuns(for: "wave-1", runs)

        #expect(store.runs(for: "wave-1").count == 50)
    }

    @Test("runs(for:) returns empty array for unknown wave")
    func runsForUnknown() {
        let store = RunStore()

        #expect(store.runs(for: "nonexistent").isEmpty)
    }

    @Test("upsertRun inserts new run at front")
    func upsertRunInserts() {
        let store = RunStore()
        store.setRuns(for: "wave-1", [makeRun(id: "run-1", iteration: 1)])

        store.upsertRun(makeRun(id: "run-2", iteration: 2))

        let runs = store.runs(for: "wave-1")
        #expect(runs.count == 2)
        #expect(runs.first?.id == "run-2")
    }

    @Test("upsertRun updates existing run in place")
    func upsertRunUpdates() {
        let store = RunStore()
        store.setRuns(for: "wave-1", [
            makeRun(id: "run-1", status: .running),
        ])

        store.upsertRun(makeRun(id: "run-1", status: .completed))

        let runs = store.runs(for: "wave-1")
        #expect(runs.count == 1)
        #expect(runs.first?.status == .completed)
    }

    @Test("upsertRun with nil waveId is a no-op")
    func upsertRunNilWaveId() {
        let store = RunStore()
        let run = WaveRun(
            id: "run-1",
            waveId: nil,
            flow: "ship",
            area: "src/",
            repo: "/tmp/repo"
        )

        store.upsertRun(run)

        #expect(store.runs(for: "wave-1").isEmpty)
    }

    @Test("upsertRun into empty store creates entry")
    func upsertRunEmpty() {
        let store = RunStore()

        store.upsertRun(makeRun(id: "run-1"))

        #expect(store.runs(for: "wave-1").count == 1)
    }

    @Test("clear removes all runs for a wave")
    func clearRemoves() {
        let store = RunStore()
        store.setRuns(for: "wave-1", [makeRun(id: "run-1")])
        store.setRuns(for: "wave-2", [makeRun(id: "run-2", waveId: "wave-2")])

        store.clear(for: "wave-1")

        #expect(store.runs(for: "wave-1").isEmpty)
        #expect(store.runs(for: "wave-2").count == 1)
    }

    @Test("waves are independent — setting one doesn't affect another")
    func wavesIndependent() {
        let store = RunStore()
        store.setRuns(for: "wave-1", [makeRun(id: "run-1")])
        store.setRuns(for: "wave-2", [makeRun(id: "run-2", waveId: "wave-2")])

        store.setRuns(for: "wave-1", [])

        #expect(store.runs(for: "wave-1").isEmpty)
        #expect(store.runs(for: "wave-2").count == 1)
    }
}
