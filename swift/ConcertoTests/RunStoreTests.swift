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
            flow: "build",
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
