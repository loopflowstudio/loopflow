// Worktree model representing a git worktree with status information.
// Maps to JSON from `wt list --format json --full`.

import Foundation

enum PRState: String, Codable {
    case open
    case merged
    case closed
    case draft

    var displayText: String {
        switch self {
        case .open: return "Open"
        case .merged: return "Merged"
        case .closed: return "Closed"
        case .draft: return "Draft"
        }
    }
}

enum Staleness: Codable, Equatable, Hashable {
    case active
    case merged
    case remoteDeleted
    case inactive(days: Int)

    var displayText: String {
        switch self {
        case .active: return "Active"
        case .merged: return "Merged"
        case .remoteDeleted: return "Remote deleted"
        case .inactive(let days): return "Inactive \(days)d"
        }
    }

    var isStale: Bool {
        switch self {
        case .active: return false
        case .merged, .remoteDeleted, .inactive: return true
        }
    }
}

struct Worktree: Identifiable, Hashable, Codable {
    var id: String { branch }

    let path: String
    let branch: String
    let baseBranch: String?
    let isDirty: Bool
    let aheadMain: Int
    let behindMain: Int
    let aheadRemote: Int
    let behindRemote: Int
    let prURL: URL?
    let prNumber: Int?
    let prState: PRState?
    let hasCodeWorkspace: Bool
    let isRebasing: Bool
    let isMerging: Bool
    var recentTasks: [TaskSession] = []
    var staleness: Staleness = .active

    enum CodingKeys: String, CodingKey {
        case path, branch, baseBranch, isDirty, aheadMain, behindMain
        case aheadRemote, behindRemote, prURL, prNumber, prState
        case hasCodeWorkspace, isRebasing, isMerging, recentTasks, staleness
    }

    var statusText: String {
        if isDirty {
            return "modified"
        }
        return "clean"
    }

    var commitsText: String {
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

    var lastTask: String? {
        recentTasks.first?.task
    }

    var lastCompletedTask: String? {
        recentTasks.first(where: { $0.isCompleted })?.task
    }
}

// JSON structure from `wt list --format json --full`
struct WorktreeJSON: Codable {
    let branch: String
    let path: String
    let kind: String?
    let baseBranch: String?
    let workingTree: WorkingTreeJSON?
    let main: MainStatusJSON?
    let remote: RemoteStatusJSON?
    let operationState: String?
    let ci: CIJSON?

    enum CodingKeys: String, CodingKey {
        case branch, path, kind
        case baseBranch = "base_branch"
        case workingTree = "working_tree"
        case main, remote
        case operationState = "operation_state"
        case ci
    }
}

struct WorkingTreeJSON: Codable {
    let staged: Bool?
    let modified: Bool?
    let untracked: Bool?
    let diffVsMain: DiffJSON?

    enum CodingKeys: String, CodingKey {
        case staged, modified, untracked
        case diffVsMain = "diff_vs_main"
    }
}

struct DiffJSON: Codable {
    let added: Int?
    let deleted: Int?
}

struct MainStatusJSON: Codable {
    let ahead: Int?
    let behind: Int?
}

struct RemoteStatusJSON: Codable {
    let name: String?
    let branch: String?
    let ahead: Int?
    let behind: Int?
}

struct CIJSON: Codable {
    let source: String?
    let url: String?
    let state: String?
}

extension Worktree {
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
        self.recentTasks = recentTasks
    }
}
