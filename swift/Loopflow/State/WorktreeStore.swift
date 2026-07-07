// WorktreeStore — cached worktrees for the current repo.

import Foundation

@MainActor
@Observable
public final class WorktreeStore {
    public private(set) var worktrees: [WorktreeInfo] = []

    /// Worktrees not tracked by any wave and not prunable (merged/empty).
    public var orphans: [WorktreeInfo] {
        worktrees.filter { !$0.hasWave && !$0.prunable }
    }

    public init() {}

    public func setAll(_ items: [WorktreeInfo]) {
        worktrees = items
    }

    public func removeAll() {
        worktrees = []
    }
}
