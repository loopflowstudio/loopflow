// Tests for WorktreeStore and WorktreeInfo.

import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

@MainActor
@Suite("WorktreeStore")
struct WorktreeStoreTests {
    private func makeWorktree(
        branch: String? = "feature-auth",
        path: String = "/Users/jack/src/loopflow.feature-auth",
        merged: Bool = false,
        prunable: Bool = false,
        waveId: String? = nil
    ) -> WorktreeInfo {
        WorktreeInfo(
            branch: branch,
            path: path,
            merged: merged,
            prunable: prunable,
            waveId: waveId
        )
    }

    @Test("setAll replaces all worktrees")
    func setAllReplaces() {
        let store = WorktreeStore()
        store.setAll([makeWorktree(path: "/a"), makeWorktree(path: "/b")])

        #expect(store.worktrees.count == 2)

        store.setAll([makeWorktree(path: "/c")])

        #expect(store.worktrees.count == 1)
    }

    @Test("orphans filters out wave-tracked worktrees")
    func orphansFiltersTracked() {
        let store = WorktreeStore()
        store.setAll([
            makeWorktree(path: "/a", waveId: nil),
            makeWorktree(path: "/b", waveId: "wave-1"),
            makeWorktree(path: "/c", waveId: nil),
        ])

        #expect(store.orphans.count == 2)
        #expect(store.orphans.allSatisfy { !$0.hasWave })
    }

    @Test("orphans returns empty when all tracked")
    func orphansEmptyWhenAllTracked() {
        let store = WorktreeStore()
        store.setAll([
            makeWorktree(path: "/a", waveId: "wave-1"),
            makeWorktree(path: "/b", waveId: "wave-2"),
        ])

        #expect(store.orphans.isEmpty)
    }

    @Test("removeAll clears store")
    func removeAllClears() {
        let store = WorktreeStore()
        store.setAll([makeWorktree()])
        store.removeAll()

        #expect(store.worktrees.isEmpty)
        #expect(store.orphans.isEmpty)
    }
}

@Suite("WorktreeInfo")
struct WorktreeInfoTests {
    @Test("id is the path")
    func idIsPath() {
        let wt = WorktreeInfo(path: "/Users/jack/src/loopflow.feature-auth")
        #expect(wt.id == "/Users/jack/src/loopflow.feature-auth")
    }

    @Test("hasWave is true when waveId is present")
    func hasWaveWhenPresent() {
        let wt = WorktreeInfo(path: "/a", waveId: "wave-1")
        #expect(wt.hasWave)
    }

    @Test("hasWave is false when waveId is nil")
    func hasWaveWhenNil() {
        let wt = WorktreeInfo(path: "/a")
        #expect(!wt.hasWave)
    }

    @Test("directoryName extracts last path component")
    func directoryName() {
        let wt = WorktreeInfo(path: "/Users/jack/src/loopflow.feature-auth")
        #expect(wt.directoryName == "loopflow.feature-auth")
    }

    @Test("shortName extracts name after first dot")
    func shortName() {
        let wt = WorktreeInfo(path: "/Users/jack/src/loopflow.feature-auth")
        #expect(wt.shortName == "feature-auth")
    }

    @Test("shortName returns nil for path without dot")
    func shortNameNoDot() {
        let wt = WorktreeInfo(path: "/Users/jack/src/loopflow")
        #expect(wt.shortName == nil)
    }

    @Test("shortName handles dots in the short name")
    func shortNameWithDots() {
        let wt = WorktreeInfo(path: "/Users/jack/src/loopflow.jack.wave.20260213")
        #expect(wt.shortName == "jack.wave.20260213")
    }
}
