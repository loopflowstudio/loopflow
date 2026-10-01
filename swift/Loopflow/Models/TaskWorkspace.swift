import Foundation

/// One path changed from a Task's immutable base commit.
public struct TaskChangedFile: Decodable, Sendable, Identifiable, Hashable {
    public var id: String { path }

    public let path: String
    public let oldPath: String?
    public let committed: Bool
    public let staged: Bool
    public let unstaged: Bool
    public let untracked: Bool

    enum CodingKeys: String, CodingKey {
        case path, committed, staged, unstaged, untracked
        case oldPath = "old_path"
    }
}

public struct TaskChangesSnapshot: Decodable, Sendable, Hashable {
    public let recoveryDirectory: String?
    public let issueIdentifier: String
    public let taskId: String
    public let baseCommit: String
    public let headCommit: String
    public let files: [TaskChangedFile]
    public let scratch: [String]
    public let scratchTruncated: Bool

    enum CodingKeys: String, CodingKey {
        case recoveryDirectory = "recovery_directory"
        case files, scratch
        case scratchTruncated = "scratch_truncated"
        case issueIdentifier = "issue_identifier"
        case taskId = "task_id"
        case baseCommit = "base_commit"
        case headCommit = "head_commit"
    }
}

public struct TaskDiffSnapshot: Decodable, Sendable, Hashable {
    public let baseCommit: String
    public let issueIdentifier: String
    public let taskId: String
    public let path: String?
    public let patch: String
    public let binary: Bool
    public let truncated: Bool

    enum CodingKeys: String, CodingKey {
        case path, patch, binary, truncated
        case baseCommit = "base_commit"
        case issueIdentifier = "issue_identifier"
        case taskId = "task_id"
    }
}

public enum TaskFileState: String, Decodable, Sendable {
    case text, binary, truncated, missing
    case unsupportedEncoding = "unsupported_encoding"
}

public struct TaskFileRecovery: Decodable, Sendable, Hashable, Identifiable {
    public var id: String { directory }
    public let directory: String
    public let changed: Bool
    public let message: String
}

public struct TaskFileSave: Decodable, Sendable {
    public let file: TaskFileSnapshot
    public let published: Bool
    public let message: String
}

public struct TaskFileSnapshot: Decodable, Sendable, Hashable {
    public let recoveries: [TaskFileRecovery]
    public let issueIdentifier: String
    public let taskId: String
    public let path: String
    public let content: String?
    public let state: TaskFileState
    public let revision: String?
    public let sizeBytes: UInt64

    enum CodingKeys: String, CodingKey {
        case path, content, state, revision, recoveries
        case issueIdentifier = "issue_identifier"
        case taskId = "task_id"
        case sizeBytes = "size_bytes"
    }
}
