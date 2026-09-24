import Foundation

/// Live ownership is independent of a Run's durable outcome and Session state.
public struct ActiveRunsSnapshot: Codable, Sendable, Hashable {
    public let home: String
    public let observedAt: Int64
    public let task: WorkReference?
    public let runs: [ActiveRun]
    public let gaps: [String]

    enum CodingKeys: String, CodingKey {
        case home, task, runs, gaps
        case observedAt = "observed_at"
    }
}

public struct ActiveRun: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let work: WorkReference?
    public let subjects: [RunSubjectAttribution]
    public let label: String
    public let harness: String
    public let model: String?
    public let repo: String?
    public let processes: [LiveProviderProcess]
}

public struct LiveProviderProcess: Codable, Sendable, Hashable {
    public let pid: UInt32
    public let provider: String
    public let state: ActivityState
}
