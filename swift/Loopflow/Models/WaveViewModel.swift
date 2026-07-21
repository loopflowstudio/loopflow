import SwiftUI

public struct WaveViewModel: Sendable, Identifiable, Hashable {
    public let api: Wave
    public let plan: WavePlan?
    public let isRegistered: Bool

    public init(api: Wave, plan: WavePlan? = nil, isRegistered: Bool = true) {
        self.api = api
        self.plan = plan
        self.isRegistered = isRegistered
    }

    public var id: String { api.id }

    public var name: String { api.name }

    public var repo: String { api.repo }

    public var displayName: String { name }

    /// The stable id of this Wave's parent, when one exists. Drives future
    /// ancestry indentation in the navigation list.
    public var parentWaveId: String? { api.parentWaveId }

    /// Open Tasks the registry counts as active for this Wave.
    public var openTaskCount: Int { api.activeTasks }

    /// The operational lens for this row, in the shared green/red/blue/black grammar.
    /// A registered Wave projects from the runtime `lf ls` carries; an unregistered
    /// Wave (authored on disk, never served) has no runtime reading, so it stays
    /// unknown-with-reason rather than a silent black or a local-session guess.
    public var lens: WaveLens {
        guard isRegistered else {
            return WaveLens(
                color: .unknown,
                reason: "Not served yet · run the Wave to read its state"
            )
        }
        return WaveLens.forWave(
            live: api.live,
            paused: api.paused,
            status: api.status,
            activeTasks: api.activeTasks,
            activeProjects: api.activeProjects
        )
    }

    public var objectiveTagline: String? {
        guard let objective = plan?.objective else { return nil }
        return objective.components(separatedBy: .newlines)
            .first { !$0.trimmingCharacters(in: .whitespaces).isEmpty }?
            .trimmingCharacters(in: .whitespaces)
    }
}
