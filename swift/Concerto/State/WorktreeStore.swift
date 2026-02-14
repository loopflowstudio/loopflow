// WorktreeStore — cached worktrees for the current repo.

import Foundation
import LoopflowCore

@MainActor
@Observable
final class WorktreeStore {
    private(set) var worktrees: [WorktreeInfo] = []

    /// Worktrees not tracked by any wave.
    var orphans: [WorktreeInfo] {
        worktrees.filter { !$0.hasWave }
    }

    func setAll(_ items: [WorktreeInfo]) {
        worktrees = items
    }

    func removeAll() {
        worktrees = []
    }
}
