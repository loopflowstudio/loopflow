import Foundation
import LoopflowCore

struct CommitFeedUpdateDecision: Equatable {
    let currentCommitSHAs: Set<String>
    let newCommitSHAs: Set<String>
    let shouldAnimateInsertion: Bool
    let shouldInvalidateDiffCache: Bool
}

func evaluateCommitFeedUpdate(
    previousCommitSHAs: Set<String>,
    commits: [CommitEntry],
    isWaveRunning: Bool
) -> CommitFeedUpdateDecision {
    let currentCommitSHAs = Set(commits.map(\.sha))
    let newCommitSHAs = currentCommitSHAs.subtracting(previousCommitSHAs)
    let isInitialSnapshot = previousCommitSHAs.isEmpty
    let shouldAnimateInsertion = isWaveRunning && !isInitialSnapshot && !newCommitSHAs.isEmpty

    return CommitFeedUpdateDecision(
        currentCommitSHAs: currentCommitSHAs,
        newCommitSHAs: newCommitSHAs,
        shouldAnimateInsertion: shouldAnimateInsertion,
        shouldInvalidateDiffCache: isWaveRunning && !newCommitSHAs.isEmpty
    )
}
