// RunStore — cached wave runs keyed by wave ID.

import Foundation
import LoopflowCore

@MainActor
@Observable
final class RunStore {
    private(set) var runs: [String: [WaveRun]] = [:]

    private let maxRunsPerWave = 50

    func setRuns(for waveId: String, _ newRuns: [WaveRun]) {
        runs[waveId] = Array(newRuns.prefix(maxRunsPerWave))
    }

    func runs(for waveId: String) -> [WaveRun] {
        runs[waveId] ?? []
    }

    func clear(for waveId: String) {
        runs.removeValue(forKey: waveId)
    }
}
