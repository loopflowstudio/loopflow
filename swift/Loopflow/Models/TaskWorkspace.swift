import Foundation

/// One path changed from a Task's immutable base commit.
public struct TaskChangedFile: Decodable, Sendable, Identifiable, Hashable {
    public var id: String { path }

    public let path: String
    public let committed: Bool
    public let staged: Bool
    public let unstaged: Bool
    public let untracked: Bool
}

public struct TaskChangesSnapshot: Decodable, Sendable, Hashable {
    public let issueIdentifier: String
    public let taskId: String
    public let baseCommit: String
    public let headCommit: String
    public let files: [TaskChangedFile]

    enum CodingKeys: String, CodingKey {
        case files
        case issueIdentifier = "issue_identifier"
        case taskId = "task_id"
        case baseCommit = "base_commit"
        case headCommit = "head_commit"
    }
}

public struct TaskDiffSnapshot: Decodable, Sendable, Hashable {
    public let issueIdentifier: String
    public let taskId: String
    public let path: String?
    public let patch: String
    public let binary: Bool
    public let truncated: Bool

    enum CodingKeys: String, CodingKey {
        case path, patch, binary, truncated
        case issueIdentifier = "issue_identifier"
        case taskId = "task_id"
    }
}

public struct TaskFileSnapshot: Decodable, Sendable, Hashable {
    public let issueIdentifier: String
    public let taskId: String
    public let path: String
    public let content: String?
    public let binary: Bool
    public let sizeBytes: UInt64
    public let truncated: Bool

    enum CodingKeys: String, CodingKey {
        case path, content, binary, truncated
        case issueIdentifier = "issue_identifier"
        case taskId = "task_id"
        case sizeBytes = "size_bytes"
    }
}
