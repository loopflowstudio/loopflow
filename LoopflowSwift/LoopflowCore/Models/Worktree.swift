// Worktree model representing a git worktree with status information.
// Maps to JSON from `wt list --format json --full`.

import Foundation
import SwiftUI

public enum PRState: String, Sendable, Codable {
    case open
    case merged
    case closed
    case draft

    public var displayText: String {
        switch self {
        case .open: return "Open"
        case .merged: return "Merged"
        case .closed: return "Closed"
        case .draft: return "Draft"
        }
    }
}

public enum CIStatus: String, Sendable, Codable {
    case passing
    case failing
    case pending
    case unknown

    public var icon: String {
        switch self {
        case .passing: return "checkmark.circle.fill"
        case .failing: return "xmark.circle.fill"
        case .pending: return "clock.fill"
        case .unknown: return "questionmark.circle"
        }
    }

    public var color: Color {
        switch self {
        case .passing: return .green
        case .failing: return .red
        case .pending: return .orange
        case .unknown: return .gray
        }
    }
}

public enum Staleness: Sendable, Codable, Equatable, Hashable {
    case active
    case merged
    case remoteDeleted
    case inactive(days: Int)

    public var displayText: String {
        switch self {
        case .active: return "Active"
        case .merged: return "Merged"
        case .remoteDeleted: return "Remote deleted"
        case .inactive(let days): return "Inactive \(days)d"
        }
    }

    public var isStale: Bool {
        switch self {
        case .active: return false
        case .merged, .remoteDeleted, .inactive: return true
        }
    }
}

public struct Worktree: Sendable, Identifiable, Hashable, Codable {
    public var id: String { branch }

    public let path: String
    public let branch: String
    public let baseBranch: String?
    public let isDirty: Bool
    public let aheadMain: Int
    public let behindMain: Int
    public let aheadRemote: Int
    public let behindRemote: Int
    public let prURL: URL?
    public let prNumber: Int?
    public let prState: PRState?
    public let hasCodeWorkspace: Bool
    public let isRebasing: Bool
    public let isMerging: Bool
    public let hasDiff: Bool
    public var recentTasks: [TaskSession] = []
    public var staleness: Staleness = .active
    public var ciStatus: CIStatus?

    enum CodingKeys: String, CodingKey {
        case path, branch, baseBranch, isDirty, aheadMain, behindMain
        case aheadRemote, behindRemote, prURL, prNumber, prState
        case hasCodeWorkspace, isRebasing, isMerging, hasDiff, recentTasks, staleness, ciStatus
    }

    public init(
        path: String,
        branch: String,
        baseBranch: String? = nil,
        isDirty: Bool = false,
        aheadMain: Int = 0,
        behindMain: Int = 0,
        aheadRemote: Int = 0,
        behindRemote: Int = 0,
        prURL: URL? = nil,
        prNumber: Int? = nil,
        prState: PRState? = nil,
        hasCodeWorkspace: Bool = false,
        isRebasing: Bool = false,
        isMerging: Bool = false,
        hasDiff: Bool = false,
        recentTasks: [TaskSession] = [],
        staleness: Staleness = .active,
        ciStatus: CIStatus? = nil
    ) {
        self.path = path
        self.branch = branch
        self.baseBranch = baseBranch
        self.isDirty = isDirty
        self.aheadMain = aheadMain
        self.behindMain = behindMain
        self.aheadRemote = aheadRemote
        self.behindRemote = behindRemote
        self.prURL = prURL
        self.prNumber = prNumber
        self.prState = prState
        self.hasCodeWorkspace = hasCodeWorkspace
        self.isRebasing = isRebasing
        self.isMerging = isMerging
        self.hasDiff = hasDiff
        self.recentTasks = recentTasks
        self.staleness = staleness
        self.ciStatus = ciStatus
    }

    public var commitsText: String {
        var parts: [String] = []
        if let pr = prNumber {
            var prText = "PR #\(pr)"
            if let state = prState {
                prText += " (\(state.displayText))"
            }
            parts.append(prText)
        } else {
            parts.append("no PR")
        }
        if aheadMain > 0 {
            parts.append("\(aheadMain) ahead")
        }
        if behindMain > 0 {
            parts.append("\(behindMain) behind")
        }
        return parts.joined(separator: " · ")
    }

    public var lastTask: String? {
        recentTasks.first?.task
    }

    public var lastCompletedTask: String? {
        recentTasks.first(where: { $0.isCompleted })?.task
    }

    /// Short name extracted from worktree path (e.g., "../repo.my-feature" → "my-feature").
    /// When branch names use a schema (e.g., "jack.my-feature.20260120_1234"),
    /// the short name from the path is more user-friendly for display.
    public var shortName: String {
        // Extract from path: ../repo.short-name → short-name
        let pathURL = URL(fileURLWithPath: path)
        let dirname = pathURL.lastPathComponent  // "repo.short-name"
        if let dotIndex = dirname.firstIndex(of: ".") {
            return String(dirname[dirname.index(after: dotIndex)...])
        }
        // Fallback to branch if path doesn't follow expected pattern
        return branch
    }

    /// Display name for the worktree in the sidebar.
    /// Uses shortName if it differs from branch (schema-based naming).
    public var displayName: String {
        shortName
    }
}

// JSON structure from `wt list --format json --full`
public struct WorktreeJSON: Sendable, Codable {
    public let branch: String
    public let path: String
    public let kind: String?
    public let baseBranch: String?
    public let workingTree: WorkingTreeJSON?
    public let main: MainStatusJSON?
    public let mainState: String?
    public let remote: RemoteStatusJSON?
    public let operationState: String?
    public let ci: CIJSON?
    public let prunable: Bool?

    enum CodingKeys: String, CodingKey {
        case branch, path, kind
        case baseBranch = "base_branch"
        case workingTree = "working_tree"
        case main
        case mainState = "main_state"
        case remote
        case operationState = "operation_state"
        case ci, prunable
    }
}

public struct WorkingTreeJSON: Sendable, Codable {
    public let staged: Bool?
    public let modified: Bool?
    public let untracked: Bool?
    public let diffVsMain: DiffJSON?

    enum CodingKeys: String, CodingKey {
        case staged, modified, untracked
        case diffVsMain = "diff_vs_main"
    }
}

public struct DiffJSON: Sendable, Codable {
    public let added: Int?
    public let deleted: Int?
}

public struct MainStatusJSON: Sendable, Codable {
    public let ahead: Int?
    public let behind: Int?
}

public struct RemoteStatusJSON: Sendable, Codable {
    public let name: String?
    public let branch: String?
    public let ahead: Int?
    public let behind: Int?
}

public struct CIJSON: Sendable, Codable {
    public let source: String?
    public let url: String?
    public let state: String?
}

public extension Worktree {
    init(from json: WorktreeJSON, hasCodeWorkspace: Bool = false, recentTasks: [TaskSession] = []) {
        self.path = json.path
        self.branch = json.branch
        self.baseBranch = json.baseBranch

        let wt = json.workingTree
        self.isDirty = (wt?.staged ?? false) || (wt?.modified ?? false) || (wt?.untracked ?? false)

        self.aheadMain = json.main?.ahead ?? 0
        self.behindMain = json.main?.behind ?? 0
        self.aheadRemote = json.remote?.ahead ?? 0
        self.behindRemote = json.remote?.behind ?? 0

        if json.ci?.source == "pr", let urlString = json.ci?.url {
            self.prURL = URL(string: urlString)
            // Extract PR number from URL like https://github.com/org/repo/pull/12
            if let match = urlString.range(of: #"/pull/(\d+)"#, options: .regularExpression) {
                let numberPart = urlString[match].dropFirst(6) // drop "/pull/"
                self.prNumber = Int(numberPart)
            } else {
                self.prNumber = nil
            }
            // Parse PR state
            if let stateString = json.ci?.state?.lowercased() {
                self.prState = PRState(rawValue: stateString)
            } else {
                self.prState = nil
            }
        } else {
            self.prURL = nil
            self.prNumber = nil
            self.prState = nil
        }

        self.hasCodeWorkspace = hasCodeWorkspace
        self.isRebasing = json.operationState == "rebase"
        self.isMerging = json.operationState == "merge"

        let diffStats = json.workingTree?.diffVsMain
        self.hasDiff = (diffStats?.added ?? 0) + (diffStats?.deleted ?? 0) > 0

        self.recentTasks = recentTasks
        if json.prunable == true {
            self.staleness = .merged
        }
    }
}
