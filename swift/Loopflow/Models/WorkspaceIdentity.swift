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
