// RunStore — cached wave runs keyed by wave ID.

import Foundation

@MainActor
@Observable
public final class RunStore {
    public private(set) var runs: [String: [Run]] = [:]

    private let maxRunsPerWave = 50

    public init() {}

    public func setRuns(for waveId: String, _ newRuns: [Run]) {
        runs[waveId] = Array(newRuns.prefix(maxRunsPerWave))
    }

    public func runs(for waveId: String) -> [Run] {
        runs[waveId] ?? []
    }

    public func clear(for waveId: String) {
        runs.removeValue(forKey: waveId)
    }
}
