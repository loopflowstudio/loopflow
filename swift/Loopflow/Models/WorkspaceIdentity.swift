import Foundation

public struct WorkspaceIdentity: Codable, Hashable, Sendable {
    enum CodingKeys: String, CodingKey {
        case machineId = "machine_id"
        case worktree
    }
    public let machineId: String
    public let worktree: String

    public init(machineId: String, worktree: String) {
        self.machineId = machineId
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
        self.repository = repository
        self.kind = kind
        self.id = id
    }

    public var reference: WorkReference? {
        switch kind {
        case .repository: nil
        case .wave: .wave(id: id)
        case .task: .task(id: id)
        }
    }
}
