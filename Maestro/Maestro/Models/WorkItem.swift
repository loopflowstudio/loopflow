// Work item model for the work queue.

import Foundation

enum WorkStatus: String, Codable, CaseIterable {
    case proposed
    case approved
    case active
    case done
}

enum WorkConfidence: String, Codable, CaseIterable {
    case high
    case medium
    case low
}

struct WorkItem: Identifiable, Codable {
    let id: String
    var title: String
    var description: String
    var status: WorkStatus
    var claimedBy: String?
    var blockedOn: String?
    var confidence: WorkConfidence
    var worktree: String?
    var notes: String

    enum CodingKeys: String, CodingKey {
        case id, title, description, status
        case claimedBy = "claimed_by"
        case blockedOn = "blocked_on"
        case confidence, worktree, notes
    }

    init(
        id: String,
        title: String,
        description: String = "",
        status: WorkStatus = .proposed,
        claimedBy: String? = nil,
        blockedOn: String? = nil,
        confidence: WorkConfidence = .medium,
        worktree: String? = nil,
        notes: String = ""
    ) {
        self.id = id
        self.title = title
        self.description = description
        self.status = status
        self.claimedBy = claimedBy
        self.blockedOn = blockedOn
        self.confidence = confidence
        self.worktree = worktree
        self.notes = notes
    }

    var isBlocked: Bool {
        blockedOn != nil
    }

    var isHumanClaimed: Bool {
        claimedBy == "human"
    }
}
