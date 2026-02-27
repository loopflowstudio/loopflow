import Testing
@testable import Concerto
@testable import LoopflowCore

@Suite("Wave Detail Live Update Decisions")
struct WaveDetailLiveUpdatesTests {
    @Test("initial commit snapshot does not animate")
    func initialSnapshotSkipsAnimation() {
        let decision = evaluateCommitFeedUpdate(
            previousCommitSHAs: [],
            commits: [CommitEntry(sha: "abc123", message: "initial")],
            isWaveRunning: true
        )

        #expect(decision.newCommitSHAs == Set(["abc123"]))
        #expect(decision.shouldAnimateInsertion == false)
        #expect(decision.shouldInvalidateDiffCache == true)
    }

    @Test("new commits animate only during running waves")
    func runningWaveAnimatesNewCommits() {
        let decision = evaluateCommitFeedUpdate(
            previousCommitSHAs: ["abc123"],
            commits: [
                CommitEntry(sha: "def456", message: "new"),
                CommitEntry(sha: "abc123", message: "old")
            ],
            isWaveRunning: true
        )

        #expect(decision.newCommitSHAs == Set(["def456"]))
        #expect(decision.shouldAnimateInsertion)
        #expect(decision.shouldInvalidateDiffCache)
    }

    @Test("new commits skip animation when wave is not running")
    func idleWaveSkipsAnimation() {
        let decision = evaluateCommitFeedUpdate(
            previousCommitSHAs: ["abc123"],
            commits: [
                CommitEntry(sha: "def456", message: "new"),
                CommitEntry(sha: "abc123", message: "old")
            ],
            isWaveRunning: false
        )

        #expect(decision.newCommitSHAs == Set(["def456"]))
        #expect(decision.shouldAnimateInsertion == false)
        #expect(decision.shouldInvalidateDiffCache == false)
    }

    @Test("unchanged commits do not invalidate diff cache")
    func unchangedCommitsNoInvalidation() {
        let decision = evaluateCommitFeedUpdate(
            previousCommitSHAs: ["abc123", "def456"],
            commits: [
                CommitEntry(sha: "def456", message: "new"),
                CommitEntry(sha: "abc123", message: "old")
            ],
            isWaveRunning: true
        )

        #expect(decision.newCommitSHAs.isEmpty)
        #expect(decision.shouldAnimateInsertion == false)
        #expect(decision.shouldInvalidateDiffCache == false)
    }
}
