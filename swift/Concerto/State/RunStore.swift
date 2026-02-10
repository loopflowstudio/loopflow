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

    func upsertRun(_ run: WaveRun) {
        guard let waveId = run.waveId else { return }
        var existing = runs[waveId] ?? []
        if let index = existing.firstIndex(where: { $0.id == run.id }) {
            existing[index] = run
        } else {
            existing.insert(run, at: 0)
            if existing.count > maxRunsPerWave {
                existing.removeLast()
            }
        }
        runs[waveId] = existing
    }

    func runs(for waveId: String) -> [WaveRun] {
        runs[waveId] ?? []
    }

    func clear(for waveId: String) {
        runs.removeValue(forKey: waveId)
    }
}
