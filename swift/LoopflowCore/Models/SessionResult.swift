// Data structures for step run results panel.

import Foundation

public enum StepRunResultStatus: String, Sendable {
    case running
    case completed
    case error
}

public enum FileChangeKind: String, Sendable {
    case added
    case modified
    case deleted
    case renamed
}

public struct FileChange: Sendable, Identifiable {
    public let id = UUID()
    public let path: String
    public let kind: FileChangeKind
    public let linesAdded: Int
    public let linesRemoved: Int
    public var diffPreview: String?  // First ~20 lines of diff, loaded lazily
}

public struct StepRunBaseline: Sendable {
    public let stepRunId: String
    public let worktree: String
    public let headSHA: String
    public let dirtyFiles: [String]
    public let timestamp: Date
}

public struct StepRunResult: Sendable, Identifiable {
    public let id: String  // step run ID
    public let step: String
    public let worktree: String
    public let status: StepRunResultStatus
    public let startedAt: Date
    public let endedAt: Date
    public var duration: TimeInterval { endedAt.timeIntervalSince(startedAt) }
    public var filesChanged: [FileChange]
    public let newCommits: [String]  // commit messages
    public let baselineSHA: String
    public let currentSHA: String

    public var hasChanges: Bool {
        !filesChanged.isEmpty || !newCommits.isEmpty
    }

    public var totalLinesAdded: Int {
        filesChanged.reduce(0) { $0 + $1.linesAdded }
    }

    public var totalLinesRemoved: Int {
        filesChanged.reduce(0) { $0 + $1.linesRemoved }
    }

    public static func running(stepRunId: String, step: String, worktree: String, startedAt: Date) -> StepRunResult {
        StepRunResult(
            id: stepRunId,
            step: step,
            worktree: worktree,
            status: .running,
            startedAt: startedAt,
            endedAt: startedAt,  // Will be updated when complete
            filesChanged: [],
            newCommits: [],
            baselineSHA: "",
            currentSHA: ""
        )
    }
}
