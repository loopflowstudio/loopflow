import Foundation

public enum ActivityNodeKind: String, Codable, Sendable, Hashable {
    case process
    case agentProcess = "agent_process"
}

public enum ActivityState: String, Codable, Sendable, Hashable {
    case working
    case waiting
    case stalled
    case unknown
}

public struct ActivityNode: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let parentId: String?
    public let kind: ActivityNodeKind
    public let label: String
    public let repo: String?
    public let worktree: String?
    public let wave: String?
    public let pid: UInt32?
    public let startedAt: Int64
    public let state: ActivityState

    enum CodingKeys: String, CodingKey {
        case id, kind, label, repo, worktree, wave, pid, state
        case parentId = "parent_id"
        case startedAt = "started_at"
    }
}

public struct ActivitySnapshot: Codable, Sendable, Hashable {
    public let schemaVersion: UInt32
    public let observedAt: Int64
    public let nodes: [ActivityNode]

    enum CodingKeys: String, CodingKey {
        case nodes
        case schemaVersion = "schema_version"
        case observedAt = "observed_at"
    }
}
