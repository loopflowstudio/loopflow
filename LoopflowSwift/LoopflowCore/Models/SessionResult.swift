// Data structures for session results panel.

import Foundation

public enum SessionResultStatus: String, Sendable {
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

public struct SessionBaseline: Sendable {
    public let sessionId: String
    public let worktree: String
    public let headSHA: String
    public let dirtyFiles: [String]
    public let timestamp: Date
}

public struct SessionResult: Sendable, Identifiable {
    public let id: String  // session ID
    public let task: String
    public let worktree: String
    public let status: SessionResultStatus
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

    public static func running(sessionId: String, task: String, worktree: String, startedAt: Date) -> SessionResult {
        SessionResult(
            id: sessionId,
            task: task,
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
