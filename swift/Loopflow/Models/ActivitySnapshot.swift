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
    public let startedAt: Date
    public let state: ActivityState

    enum CodingKeys: String, CodingKey {
        case id, kind, label, repo, worktree, wave, pid, state
        case parentId = "parent_id"
        case startedAt = "started_at"
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(String.self, forKey: .id)
        parentId = try c.decode(String?.self, forKey: .parentId)
        kind = try c.decode(ActivityNodeKind.self, forKey: .kind)
        label = try c.decode(String.self, forKey: .label)
        repo = try c.decode(String?.self, forKey: .repo)
        worktree = try c.decode(String?.self, forKey: .worktree)
        wave = try c.decode(String?.self, forKey: .wave)
        pid = try c.decode(UInt32?.self, forKey: .pid)
        startedAt = try c.decodeRFC3339(forKey: .startedAt)
        state = try c.decode(ActivityState.self, forKey: .state)
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(id, forKey: .id)
        try c.encode(parentId, forKey: .parentId)
        try c.encode(kind, forKey: .kind)
        try c.encode(label, forKey: .label)
        try c.encode(repo, forKey: .repo)
        try c.encode(worktree, forKey: .worktree)
        try c.encode(wave, forKey: .wave)
        try c.encode(pid, forKey: .pid)
        try c.encodeRFC3339(startedAt, forKey: .startedAt)
        try c.encode(state, forKey: .state)
    }
}

public struct ActivitySnapshot: Codable, Sendable, Hashable {
    public let nodes: [ActivityNode]
}
