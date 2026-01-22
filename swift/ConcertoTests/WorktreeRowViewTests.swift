// UI tests for WorktreeRow view.

import SwiftUI
import Testing
import ViewInspector
@testable import Concerto

@MainActor
@Suite("Worktree Row View")
struct WorktreeRowViewTests {
    private func makeWorktree(
        branch: String = "feature",
        aheadMain: Int = 0,
        behindMain: Int = 0,
        prNumber: Int? = nil,
        prState: PRState? = nil,
        staleness: Staleness = .active,
        hasDiff: Bool = true
    ) -> Worktree {
        var worktree = Worktree(
            path: "/tmp/repo.feature",
            branch: branch,
            baseBranch: "main",
            isDirty: false,
            aheadMain: aheadMain,
            behindMain: behindMain,
            aheadRemote: 0,
            behindRemote: 0,
            prURL: prNumber == nil ? nil : URL(string: "https://github.com/org/repo/pull/\(prNumber!)"),
            prNumber: prNumber,
            prState: prState,
            hasCodeWorkspace: false,
            isRebasing: false,
            isMerging: false,
            hasDiff: hasDiff
        )
        worktree.staleness = staleness
        return worktree
    }

    private func makeRow(worktree: Worktree) -> WorktreeRow {
        WorktreeRow(
            worktree: worktree,
            isSelected: false,
            terminalName: "Warp",
            ideName: "Cursor",
            otherWorktrees: [],
            onSelect: {},
            onDoubleClick: {},
            onOpenTerminal: {},
            onOpenIDE: {},
            onOpenFinder: {},
            onViewDiff: {},
            onCompareWith: { _ in },
            onCreatePR: {},
            onViewPR: {},
            onLandPR: {},
            onDelete: {}
        )
    }

    @Test("Worktree row shows branch and commits text")
    func showsBranchAndCommits() throws {
        let worktree = makeWorktree(aheadMain: 2, behindMain: 1)
        let row = makeRow(worktree: worktree)

        let branchText = try row.inspect().find(viewWithAccessibilityIdentifier: "worktree-branch").text()
        #expect(try branchText.string() == "feature")

        let commitsText = try row.inspect().find(viewWithAccessibilityIdentifier: "worktree-commits").text()
        #expect(try commitsText.string().contains("2 ahead"))
        #expect(try commitsText.string().contains("1 behind"))
    }

    @Test("Worktree row shows ahead badge when ahead of main")
    func showsAheadBadge() throws {
        let worktree = makeWorktree(aheadMain: 3)
        let row = makeRow(worktree: worktree)

        let badge = try row.inspect().find(viewWithAccessibilityIdentifier: "worktree-ahead-badge").text()
        #expect(try badge.string() == "3")
    }

    @Test("Worktree row shows staleness badge when merged")
    func showsStalenessBadge() throws {
        let worktree = makeWorktree(staleness: .merged)
        let row = makeRow(worktree: worktree)

        let badge = try row.inspect().find(viewWithAccessibilityIdentifier: "worktree-staleness")
        #expect(try badge.find(text: "Merged").string() == "Merged")
    }
}
