import Foundation

/// Live ownership is independent of a command's durable outcome and Session state.
public struct ActiveSessionsSnapshot: Codable, Sendable, Hashable {
    public let discovery: ActiveSessionDiscovery
    public let home: String
    public let observedAt: Int64
    public let task: WorkReference?
    public let sessions: [ActiveSession]
    public let gaps: [String]

    enum CodingKeys: String, CodingKey {
        case discovery, home, task, sessions, gaps
        case observedAt = "observed_at"
    }
}

public struct ActiveSession: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let work: WorkReference?
    public let title: String
    public let processes: [LiveProviderProcess]
}

public struct LiveProviderProcess: Codable, Sendable, Hashable {
    public let pid: UInt32
    public let provider: String
    public let state: ActivityState
}

public enum ActivityState: String, Codable, Sendable, Hashable {
    case working
    case waiting
    case stalled
}

public enum ActiveSessionDiscovery: String, Codable, Sendable, Hashable {
    case scanning, ready, unavailable
}
