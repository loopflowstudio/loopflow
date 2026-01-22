// Work item model for the work queue.

import Foundation

public enum WorkStatus: String, Sendable, Codable, CaseIterable {
    case proposed
    case approved
    case active
    case done
}

public struct WorkItem: Sendable, Identifiable, Codable {
    public let id: String
    public var title: String
    public var description: String
    public var status: WorkStatus
    public var claimedBy: String?
    public var blockedOn: String?
    public var worktree: String?
    public var notes: String

    enum CodingKeys: String, CodingKey {
        case id, title, description, status
        case claimedBy = "claimed_by"
        case blockedOn = "blocked_on"
        case worktree, notes
    }

    public init(
        id: String,
        title: String,
        description: String = "",
        status: WorkStatus = .proposed,
        claimedBy: String? = nil,
        blockedOn: String? = nil,
        worktree: String? = nil,
        notes: String = ""
    ) {
        self.id = id
        self.title = title
        self.description = description
        self.status = status
        self.claimedBy = claimedBy
        self.blockedOn = blockedOn
        self.worktree = worktree
        self.notes = notes
    }

    public var isBlocked: Bool {
        blockedOn != nil
    }

    public var isHumanClaimed: Bool {
        claimedBy == "human"
    }
}
