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

    public var status: WaveStatus { api.status }

    public var displayName: String { name }

    public var objectiveTagline: String? {
        guard let objective = plan?.objective else { return nil }
        return objective.components(separatedBy: .newlines)
            .first { !$0.trimmingCharacters(in: .whitespaces).isEmpty }?
            .trimmingCharacters(in: .whitespaces)
    }

    public var statusText: String {
        switch status {
        case .running: "Running"
        case .idle: "Idle"
        case .paused: "Paused"
        }
    }

    public var statusIndicator: (icon: String, color: Color) {
        (status.icon, status.color)
    }
}
