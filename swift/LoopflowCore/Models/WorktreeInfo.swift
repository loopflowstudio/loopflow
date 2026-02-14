// A git worktree on disk, possibly associated with a wave.

import Foundation

public struct WorktreeInfo: Sendable, Identifiable, Hashable {
    public let branch: String?
    public let path: String
    public let merged: Bool
    public let prunable: Bool
    public let waveId: String?

    public var id: String { path }
    public var hasWave: Bool { waveId != nil }

    /// Directory name only (e.g. "loopflow.feature-auth").
    public var directoryName: String {
        URL(fileURLWithPath: path).lastPathComponent
    }

    /// Short name extracted from the directory name.
    /// Given "loopflow.feature-auth", returns "feature-auth".
    public var shortName: String? {
        let parts = directoryName.split(
            separator: ".",
            maxSplits: 1,
            omittingEmptySubsequences: false
        )
        guard parts.count == 2, !parts[1].isEmpty else { return nil }
        return String(parts[1])
    }

    public init(
        branch: String? = nil,
        path: String,
        merged: Bool = false,
        prunable: Bool = false,
        waveId: String? = nil
    ) {
        self.branch = branch
        self.path = path
        self.merged = merged
        self.prunable = prunable
        self.waveId = waveId
    }
}
