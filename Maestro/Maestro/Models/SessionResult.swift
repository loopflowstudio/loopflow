// Data structures for session results panel.

import Foundation

enum SessionResultStatus: String {
    case running
    case completed
    case error
}

enum FileChangeKind: String {
    case added
    case modified
    case deleted
    case renamed
}

struct FileChange: Identifiable {
    let id = UUID()
    let path: String
    let kind: FileChangeKind
    let linesAdded: Int
    let linesRemoved: Int
    var diffPreview: String?  // First ~20 lines of diff, loaded lazily
}

struct SessionBaseline {
    let sessionId: String
    let worktree: String
    let headSHA: String
    let dirtyFiles: [String]
    let timestamp: Date
}

struct SessionResult: Identifiable {
    let id: String  // session ID
    let task: String
    let worktree: String
    let status: SessionResultStatus
    let startedAt: Date
    let endedAt: Date
    var duration: TimeInterval { endedAt.timeIntervalSince(startedAt) }
    var filesChanged: [FileChange]
    let newCommits: [String]  // commit messages
    let baselineSHA: String
    let currentSHA: String

    var hasChanges: Bool {
        !filesChanged.isEmpty || !newCommits.isEmpty
    }

    var totalLinesAdded: Int {
        filesChanged.reduce(0) { $0 + $1.linesAdded }
    }

    var totalLinesRemoved: Int {
        filesChanged.reduce(0) { $0 + $1.linesRemoved }
    }

    static func running(sessionId: String, task: String, worktree: String, startedAt: Date) -> SessionResult {
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
