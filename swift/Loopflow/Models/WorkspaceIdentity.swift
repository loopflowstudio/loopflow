import Foundation

public struct WorkspaceIdentity: Codable, Hashable, Sendable {
    enum CodingKeys: String, CodingKey {
        case machineId = "machine_id"
        case worktree
    }
    public let machineId: String
    public let worktree: String

    public init(machineId: String, worktree: String) {
        self.machineId = StructuredIdentity.prefixed(machineId, "home_")
        self.worktree = worktree
    }
}

/// Presentation identity. Execution locations belong to pane contents.
public struct WorkWorkspaceKey: Codable, Hashable, Sendable {
    public enum Kind: String, Codable, Sendable { case repository, wave, task }
    public let repository: String
    public let kind: Kind
    public let id: String

    public init(repository: String, kind: Kind, id: String) {
        self.repository = StructuredIdentity.prefixed(repository, "repo_")
        self.kind = kind
        self.id = switch kind {
        case .repository: StructuredIdentity.prefixed(id, "repo_")
        case .wave: StructuredIdentity.uuid(id)
        case .task: StructuredIdentity.prefixed(id, "task_")
        }
    }

    public var reference: WorkReference? {
        switch kind {
        case .repository: nil
        case .wave: .wave(id: id)
        case .task: .task(id: id)
        }
    }
}

// Match Rust's case-only domains. Paths and native provider identities never use this.
enum StructuredIdentity {
    private static let hyphenated = "[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"
    private static var uuidPattern: String { "^(?:[0-9a-f]{32}|\(hyphenated)|\\{\(hyphenated)\\}|urn:uuid:\(hyphenated))$" }

    static func uuid(_ value: String) -> String {
        let lower = value.lowercased()
        return lower.range(of: uuidPattern, options: .regularExpression) == nil ? value : lower
    }

    static func prefixed(_ value: String, _ prefix: String) -> String {
        let lower = value.lowercased()
        guard lower.hasPrefix(prefix),
              String(lower.dropFirst(prefix.count)).range(of: uuidPattern, options: .regularExpression) != nil else { return value }
        return lower
    }

    static func session(_ value: String) -> String {
        let lower = value.lowercased()
        return lower.range(of: "^(?:session_)?[0-9a-f]{32}$", options: .regularExpression) == nil ? value : lower
    }
}

extension WorkspaceIdentity {
    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        self.init(machineId: try values.decode(String.self, forKey: .machineId),
                  worktree: try values.decode(String.self, forKey: .worktree))
    }
}

extension WorkWorkspaceKey {
    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        self.init(repository: try values.decode(String.self, forKey: .repository),
                  kind: try values.decode(Kind.self, forKey: .kind),
                  id: try values.decode(String.self, forKey: .id))
    }
}
