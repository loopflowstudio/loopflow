import Foundation

public enum TaskFileKind: String, Decodable, Sendable {
    case file, directory, symlink
}

public struct TaskFileEntry: Decodable, Sendable, Hashable, Identifiable {
    public var id: String { path }
    public let path: String
    public let kind: TaskFileKind

    public init(path: String, kind: TaskFileKind) {
        self.path = path
        self.kind = kind
    }
}

public struct TaskSymbol: Decodable, Hashable, Sendable {
    public let name: String
    public let kind: String
    public let path: String
    public let line: Int
    public let container: String?

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        name = try values.decode(String.self, forKey: .name)
        kind = try values.decode(String.self, forKey: .kind)
        path = try values.decode(String.self, forKey: .path)
        line = try values.decode(Int.self, forKey: .line)
        container = try values.decode(String?.self, forKey: .container)
    }

    public init(name: String, kind: String, path: String, line: Int, container: String?) {
        self.name = name
        self.kind = kind
        self.path = path
        self.line = line
        self.container = container
    }

    private enum CodingKeys: CodingKey { case name, kind, path, line, container }
}

public struct TaskSymbols: Decodable, Sendable {
    public let hits: [TaskSymbol]
    public let truncated: Bool
}

public struct TaskGrepHit: Decodable, Hashable, Sendable {
    public let path: String
    public let line: Int
    public let start: Int
    public let end: Int
    public let revision: String
    public let excerptStart: Int
    public let excerpt: String

    private enum CodingKeys: String, CodingKey {
        case path, line, start, end, revision, excerpt
        case excerptStart = "excerpt_start"
    }
}

public struct TaskGrepResults: Decodable, Sendable {
    public let hits: [TaskGrepHit]
    public let truncated: Bool
}

public struct TaskFileInventory: Decodable, Sendable {
    public let paths: [String]
    public let scratch: [String]
    public let latestScratchPage: String?
    public let truncated: Bool

    enum CodingKeys: String, CodingKey {
        case paths, scratch, truncated
        case latestScratchPage = "latest_scratch_page"
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        paths = try values.decode([String].self, forKey: .paths)
        scratch = try values.decode([String].self, forKey: .scratch)
        latestScratchPage = try values.decode(String?.self, forKey: .latestScratchPage)
        truncated = try values.decode(Bool.self, forKey: .truncated)
    }
}

public struct TaskDirectory: Decodable, Sendable, Hashable {
    public let path: String
    public let entries: [TaskFileEntry]
    public let nextCursor: String?

    public init(path: String, entries: [TaskFileEntry], nextCursor: String?) {
        self.path = path
        self.entries = entries
        self.nextCursor = nextCursor
    }

    enum CodingKeys: String, CodingKey {
        case path, entries
        case nextCursor = "next_cursor"
    }
}

/// One path changed from a Task's immutable base commit.
public struct TaskChangedFile: Decodable, Sendable, Identifiable, Hashable {
    public var id: String { path }

    public let path: String
    public let oldPath: String?
    public let committed: Bool
    public let staged: Bool
    public let unstaged: Bool
    public let untracked: Bool
    /// Runs of changed lines in the file's patch: the Diff's stops, known
    /// before its lines are read.
    public let changes: Int
    public struct ChangedLines: Decodable, Hashable, Sendable {
        public let added: Int
        public let removed: Int
    }
    public let lines: ChangedLines?

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        path = try values.decode(String.self, forKey: .path)
        oldPath = try values.decodeIfPresent(String.self, forKey: .oldPath)
        committed = try values.decode(Bool.self, forKey: .committed)
        staged = try values.decode(Bool.self, forKey: .staged)
        unstaged = try values.decode(Bool.self, forKey: .unstaged)
        untracked = try values.decode(Bool.self, forKey: .untracked)
        changes = try values.decode(Int.self, forKey: .changes)
        lines = try values.decode(ChangedLines?.self, forKey: .lines)
    }

    enum CodingKeys: String, CodingKey {
        case path, committed, staged, unstaged, untracked, changes, lines
        case oldPath = "old_path"
    }
}

/// One commit in a Task's comparison. A Graph step's commits share the
/// subject prefix `lf <step>:`.
public struct TaskCommit: Decodable, Sendable, Hashable {
    public let sha: String
    /// The first parent: a step starting here compares against it.
    public let parent: String
    public let subject: String
}

public struct TaskChangesSnapshot: Decodable, Sendable, Hashable {
    public let recoveryDirectory: String?
    public let issueIdentifier: String
    public let taskId: String
    public let baseCommit: String
    public let headCommit: String
    /// Head's first-parent line since base, oldest first, without merges.
    public let commits: [TaskCommit]
    public let files: [TaskChangedFile]
    public let scratch: [String]
    public let scratchTruncated: Bool

    enum CodingKeys: String, CodingKey {
        case recoveryDirectory = "recovery_directory"
        case commits, files, scratch
        case scratchTruncated = "scratch_truncated"
        case issueIdentifier = "issue_identifier"
        case taskId = "task_id"
        case baseCommit = "base_commit"
        case headCommit = "head_commit"
    }
}

public struct TaskDiffSnapshot: Decodable, Sendable, Hashable {
    public let baseCommit: String
    public let headCommit: String?
    public let issueIdentifier: String
    public let taskId: String
    public let path: String?
    public let patch: String
    public let binary: Bool
    public let truncated: Bool

    enum CodingKeys: String, CodingKey {
        case path, patch, binary, truncated
        case baseCommit = "base_commit"
        case headCommit = "head_commit"
        case issueIdentifier = "issue_identifier"
        case taskId = "task_id"
    }
}

public enum TaskFileState: String, Decodable, Sendable {
    case text, binary, truncated, missing
    case unsupportedEncoding = "unsupported_encoding"

    /// What a person sees in place of a file that cannot be shown as text.
    public var title: String {
        switch self {
        case .text: "File"
        case .binary: "Binary file"
        case .missing: "File no longer exists"
        case .truncated: "File exceeds 1 MB · preview unavailable"
        case .unsupportedEncoding: "Unsupported text encoding"
        }
    }
}

/// How one line of a unified patch reads.
public enum PatchLineKind: Equatable, Sendable {
    case header, hunk, added, removed, context

    /// The kind of `line`, the next line of a patch. `inHunk` carries across
    /// lines: only lines inside a hunk are added or removed, so a file header
    /// such as `+++ b/path` stays a header and a removed `-- comment` is removed.
    public static func next(_ line: some StringProtocol, inHunk: inout Bool) -> PatchLineKind {
        if line.hasPrefix("@@") { inHunk = true; return .hunk }
        if line.hasPrefix("diff ") { inHunk = false; return .header }
        guard inHunk else { return .header }
        if line.hasPrefix("+") { return .added }
        if line.hasPrefix("-") { return .removed }
        return .context
    }
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
    /// A `binary` file's bytes (base64 on the wire), up to 16 MiB; nil over that and for other states.
    public let data: Data?
    public let state: TaskFileState
    public let revision: String?
    public let readOnlyReason: String?
    public let sizeBytes: UInt64

    public init(recoveries: [TaskFileRecovery], issueIdentifier: String, taskId: String, path: String,
                content: String?, data: Data?, state: TaskFileState, revision: String?, readOnlyReason: String?,
                sizeBytes: UInt64) {
        self.recoveries = recoveries; self.issueIdentifier = issueIdentifier; self.taskId = taskId
        self.path = path; self.content = content; self.data = data; self.state = state; self.revision = revision
        self.readOnlyReason = readOnlyReason; self.sizeBytes = sizeBytes
    }

    enum CodingKeys: String, CodingKey {
        case path, content, data, state, revision, recoveries
        case issueIdentifier = "issue_identifier"
        case taskId = "task_id"
        case sizeBytes = "size_bytes"
        case readOnlyReason = "read_only_reason"
    }
}

/// One `task.files.watch` reading: the Machine running the Task, its checkout
/// there and an opaque revision of its whole tree. `checkout` and `revision`
/// are nil until the Task has a checkout there. A changed Machine or checkout
/// is a move.
public struct TaskFilesRevision: Decodable, Sendable, Equatable {
    public let taskID: String
    public let machineID: String
    /// The serving Machine's name for the owner.
    public let machineLabel: String?
    public let checkout: String?
    public let revision: String?

    public var notStarted: Bool { checkout == nil }

    enum CodingKeys: String, CodingKey {
        case checkout, revision
        case taskID = "task_id", machineID = "machine_id", machineLabel = "machine_label"
    }

    public init(from decoder: any Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        taskID = try values.decode(String.self, forKey: .taskID)
        machineID = StructuredIdentity.prefixed(try values.decode(String.self, forKey: .machineID), "home_")
        machineLabel = try values.decode(String?.self, forKey: .machineLabel)
        checkout = try values.decode(String?.self, forKey: .checkout)
        revision = try values.decode(String?.self, forKey: .revision)
    }
}
