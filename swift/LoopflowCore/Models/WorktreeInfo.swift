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
        let dir = directoryName
        guard let dotIndex = dir.firstIndex(of: ".") else { return nil }
        let after = dir[dir.index(after: dotIndex)...]
        return after.isEmpty ? nil : String(after)
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
