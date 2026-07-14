import SwiftUI

public struct WaveViewModel: Sendable, Identifiable, Hashable {
    public var api: Wave
    public var plan: WavePlan?

    public init(api: Wave, plan: WavePlan? = nil) {
        self.api = api
        self.plan = plan
    }

    public var id: String { api.id }

    public var name: String {
        get { api.name }
        set { api.name = newValue }
    }

    public var repo: String {
        get { api.repo }
        set { api.repo = newValue }
    }

    public var status: WaveStatus {
        get { api.status }
        set { api.status = newValue }
    }

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
        case .waiting: "Waiting"
        case .idle: "Idle"
        case .failed: "Failed"
        case .paused: "Paused"
        }
    }

    public var statusIndicator: (icon: String, color: Color) {
        (status.icon, status.color)
    }
}
