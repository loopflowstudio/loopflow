// Portfolio repository entry for persistence.

import Loopflow
import Foundation

struct PortfolioRepo: Codable, Identifiable, Hashable {
    let path: String
    var lastOpened: Date

    /// Overrides the rail label for a launch-provided worktree that stands in for
    /// its main repo: the stored `path` stays the real checkout (for reads + lfd),
    /// while the rail shows the collapsed main-repo name. Absent for scanned repos.
    var displayNameOverride: String? = nil

    var id: String { path }
    var url: URL { URL(fileURLWithPath: path) }
    var displayName: String { displayNameOverride ?? url.lastPathComponent }
}
