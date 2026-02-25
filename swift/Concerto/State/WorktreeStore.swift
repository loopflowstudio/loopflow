// WorktreeStore — cached worktrees for the current repo.

import Foundation
import LoopflowCore

@MainActor
@Observable
final class WorktreeStore {
    private(set) var worktrees: [WorktreeInfo] = []

    /// Worktrees not tracked by any wave and not prunable (merged/empty).
    var orphans: [WorktreeInfo] {
        worktrees.filter { !$0.hasWave && !$0.prunable }
    }

    func setAll(_ items: [WorktreeInfo]) {
        worktrees = items
    }

    func removeAll() {
        worktrees = []
    }
}
