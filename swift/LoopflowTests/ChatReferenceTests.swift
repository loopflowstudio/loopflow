import Testing
import Foundation
@testable import Loopflow

@Suite("Chat reference detection")
struct ChatReferenceTests {
    private func kinds(_ text: String) -> [ChatReferenceKind] {
        parseChatReferences(in: text).map(\.kind)
    }

    private func identifiers(_ text: String) -> [String] {
        parseChatReferences(in: text).map(\.identifier)
    }

    @Test("Detects a Linear issue key as a Task reference")
    func detectsTaskKey() {
        let matches = parseChatReferences(in: "W2-174 incorporated the landing hold")
        #expect(matches.count == 1)
        #expect(matches.first?.kind == .task)
        #expect(matches.first?.identifier == "W2-174")
        #expect(matches.first?.displayText == "W2-174")
    }

    @Test("Detects a PR reference in each authored form")
    func detectsPullRequestForms() {
        #expect(identifiers("landed in PR #889") == ["889"])
        #expect(identifiers("landed in PR#889") == ["889"])
        #expect(identifiers("closed by #889") == ["889"])
        #expect(kinds("closed by #889") == [.pullRequest])
        // Hash-less "PR 889" is ambiguous prose and intentionally not linked.
        #expect(parseChatReferences(in: "PR 889 files changed").isEmpty)
    }

    @Test("Keeps the authored display text for a PR reference")
    func keepsPullRequestDisplay() {
        let matches = parseChatReferences(in: "see PR #889 for detail")
        #expect(matches.first?.displayText == "PR #889")
        #expect(matches.first?.identifier == "889")
    }

    @Test("A mixed message keeps references ordered and non-overlapping")
    func mixedMessageOrdered() {
        let matches = parseChatReferences(in: "W2-174 landed as PR #889, follow-up W2-180")
        #expect(matches.map(\.kind) == [.task, .pullRequest, .task])
        #expect(matches.map(\.identifier) == ["W2-174", "889", "W2-180"])
        // Ordered by position and disjoint.
        for pair in zip(matches, matches.dropFirst()) {
            #expect(pair.0.range.upperBound <= pair.1.range.lowerBound)
        }
    }

    @Test("PR #889 is one reference, not a PR plus a bare hash")
    func prPrefixDoesNotDoubleMatch() {
        let matches = parseChatReferences(in: "PR #889")
        #expect(matches.count == 1)
        #expect(matches.first?.kind == .pullRequest)
        #expect(matches.first?.displayText == "PR #889")
    }

    @Test("Common technical tokens are not mistaken for Task references")
    func denylistedTokensIgnored() {
        #expect(parseChatReferences(in: "encoded as UTF-8").isEmpty)
        #expect(parseChatReferences(in: "SHA-256 digest").isEmpty)
        #expect(parseChatReferences(in: "an ISO-8601 timestamp").isEmpty)
        #expect(parseChatReferences(in: "using GPT-4 here").isEmpty)
    }

    @Test("Keys glued to other identifier characters are not references")
    func boundedMatchingOnly() {
        #expect(parseChatReferences(in: "xW2-174").isEmpty)
        #expect(parseChatReferences(in: "W2-174x").isEmpty)
        #expect(parseChatReferences(in: "path/to/#889abc").isEmpty)
    }

    @Test("Plain prose with no references returns nothing quickly")
    func noReferences() {
        #expect(parseChatReferences(in: "The restarted Project Session is healthy.").isEmpty)
        #expect(parseChatReferences(in: "").isEmpty)
    }

    @Test("Reference range slices back to its display text")
    func rangeSlicesToDisplay() {
        let text = "closing W2-174 now"
        let match = parseChatReferences(in: text).first
        #expect(match != nil)
        if let match {
            #expect(String(text[match.range]) == "W2-174")
        }
    }
}

@Suite("Reference external targets")
struct ReferenceTargetTests {
    @Test("Normalizes GitHub remotes to a repo base URL")
    func normalizesRemotes() {
        let expected = URL(string: "https://github.com/loopflowstudio/loopflow")
        #expect(githubRepoBase(fromRemote: "git@github.com:loopflowstudio/loopflow.git") == expected)
        #expect(githubRepoBase(fromRemote: "https://github.com/loopflowstudio/loopflow.git") == expected)
        #expect(githubRepoBase(fromRemote: "https://github.com/loopflowstudio/loopflow") == expected)
        #expect(githubRepoBase(fromRemote: "ssh://git@github.com/loopflowstudio/loopflow.git") == expected)
    }

    @Test("Non-GitHub or unparseable remotes yield no base")
    func rejectsNonGitHub() {
        #expect(githubRepoBase(fromRemote: "git@gitlab.com:acme/repo.git") == nil)
        #expect(githubRepoBase(fromRemote: "") == nil)
        #expect(githubRepoBase(fromRemote: "https://github.com/") == nil)
    }

    @Test("A lookalike host is not mistaken for GitHub")
    func rejectsLookalikeHost() {
        #expect(githubRepoBase(fromRemote: "git@mygithub.com:acme/repo.git") == nil)
        #expect(githubRepoBase(fromRemote: "https://notgithub.com/acme/repo.git") == nil)
    }

    @Test("Builds a PR URL from a base and number")
    func buildsPullRequestURL() {
        let base = URL(string: "https://github.com/loopflowstudio/loopflow")
        #expect(
            githubPullRequestURL(base: base, number: "889")
                == URL(string: "https://github.com/loopflowstudio/loopflow/pull/889")
        )
        #expect(githubPullRequestURL(base: nil, number: "889") == nil)
        #expect(githubPullRequestURL(base: base, number: "abc") == nil)
    }
}
