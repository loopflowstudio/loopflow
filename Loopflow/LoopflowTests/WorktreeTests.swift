// Tests for Worktree model and service.

import Foundation
import Testing
@testable import Loopflow

@Suite("Worktree Model")
struct WorktreeModelTests {

    @Test("Worktree status text shows clean when not dirty")
    func statusTextClean() {
        let worktree = Worktree(
            path: "/test/path",
            branch: "main",
            baseBranch: nil,
            isDirty: false,
            aheadMain: 0,
            behindMain: 0,
            aheadRemote: 0,
            behindRemote: 0,
            prURL: nil,
            prNumber: nil,
            hasCodeWorkspace: false,
            isRebasing: false,
            isMerging: false
        )

        #expect(worktree.statusText == "clean")
    }

    @Test("Worktree status text shows modified when dirty")
    func statusTextDirty() {
        let worktree = Worktree(
            path: "/test/path",
            branch: "feature",
            baseBranch: "main",
            isDirty: true,
            aheadMain: 2,
            behindMain: 0,
            aheadRemote: 1,
            behindRemote: 0,
            prURL: nil,
            prNumber: nil,
            hasCodeWorkspace: false,
            isRebasing: false,
            isMerging: false
        )

        #expect(worktree.statusText == "modified")
    }

    @Test("Worktree commits text includes PR number when present")
    func commitsTextWithPR() {
        let worktree = Worktree(
            path: "/test/path",
            branch: "feature",
            baseBranch: "main",
            isDirty: false,
            aheadMain: 5,
            behindMain: 0,
            aheadRemote: 0,
            behindRemote: 0,
            prURL: URL(string: "https://github.com/org/repo/pull/42"),
            prNumber: 42,
            hasCodeWorkspace: false,
            isRebasing: false,
            isMerging: false
        )

        #expect(worktree.commitsText.contains("PR #42"))
        #expect(worktree.commitsText.contains("5 ahead"))
    }

    @Test("Worktree commits text shows no PR when absent")
    func commitsTextNoPR() {
        let worktree = Worktree(
            path: "/test/path",
            branch: "feature",
            baseBranch: "main",
            isDirty: false,
            aheadMain: 3,
            behindMain: 1,
            aheadRemote: 0,
            behindRemote: 0,
            prURL: nil,
            prNumber: nil,
            hasCodeWorkspace: false,
            isRebasing: false,
            isMerging: false
        )

        #expect(worktree.commitsText.contains("no PR"))
        #expect(worktree.commitsText.contains("3 ahead"))
        #expect(worktree.commitsText.contains("1 behind"))
    }

    @Test("Worktree ID is branch name")
    func idIsBranchName() {
        let worktree = Worktree(
            path: "/test/path",
            branch: "my-feature",
            baseBranch: nil,
            isDirty: false,
            aheadMain: 0,
            behindMain: 0,
            aheadRemote: 0,
            behindRemote: 0,
            prURL: nil,
            prNumber: nil,
            hasCodeWorkspace: false,
            isRebasing: false,
            isMerging: false
        )

        #expect(worktree.id == "my-feature")
    }
}

@Suite("Worktree JSON Parsing")
struct WorktreeJSONTests {

    @Test("Parses worktrunk JSON with all fields")
    func parseFullJSON() throws {
        let json = """
        {
            "branch": "feature",
            "path": "/repo/worktree",
            "kind": "worktree",
            "base_branch": "main",
            "working_tree": {
                "staged": true,
                "modified": false,
                "untracked": false,
                "diff_vs_main": {"added": 10, "deleted": 5}
            },
            "main": {"ahead": 3, "behind": 1},
            "remote": {"name": "origin", "branch": "feature", "ahead": 2, "behind": 0},
            "operation_state": "rebase",
            "ci": {"source": "pr", "url": "https://github.com/org/repo/pull/42"}
        }
        """

        let data = json.data(using: .utf8)!
        let decoded = try JSONDecoder().decode(WorktreeJSON.self, from: data)
        let worktree = Worktree(from: decoded)

        #expect(worktree.branch == "feature")
        #expect(worktree.path == "/repo/worktree")
        #expect(worktree.baseBranch == "main")
        #expect(worktree.isDirty == true)
        #expect(worktree.aheadMain == 3)
        #expect(worktree.behindMain == 1)
        #expect(worktree.aheadRemote == 2)
        #expect(worktree.isRebasing == true)
        #expect(worktree.prNumber == 42)
    }

    @Test("Parses minimal JSON with defaults")
    func parseMinimalJSON() throws {
        let json = """
        {
            "branch": "main",
            "path": "/repo"
        }
        """

        let data = json.data(using: .utf8)!
        let decoded = try JSONDecoder().decode(WorktreeJSON.self, from: data)
        let worktree = Worktree(from: decoded)

        #expect(worktree.branch == "main")
        #expect(worktree.isDirty == false)
        #expect(worktree.aheadMain == 0)
        #expect(worktree.prNumber == nil)
    }
}
