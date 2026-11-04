import SwiftUI

public struct WaveViewModel: Sendable, Identifiable, Hashable {
    public let api: Wave
    public let plan: WavePlan?

    public init(api: Wave, plan: WavePlan? = nil) {
        self.api = api
        self.plan = plan
    }

    public var id: String { api.id }

    public var name: String { api.name }

    public var repo: String { api.repo }

    public var displayName: String { api.displayName }

    /// The stable id of this Wave's parent, when one exists. Drives future
    /// ancestry indentation in the navigation list.
    public var parentWaveId: String? { api.parentWaveId }

    /// Open Tasks the registry counts as active for this Wave.
    public var openTaskCount: Int { api.activeTasks }

    /// The operational lens for this row, in the shared green/red/blue/black grammar,
    /// projected from the runtime `lf wave list` carries.
    public var lens: WaveLens {
        WaveLens.forWave(activeTasks: api.activeTasks)
    }

    public var objectiveTagline: String? {
        guard let objective = plan?.objective else { return nil }
        return objective.components(separatedBy: .newlines)
            .first { !$0.trimmingCharacters(in: .whitespaces).isEmpty }?
            .trimmingCharacters(in: .whitespaces)
    }
}
