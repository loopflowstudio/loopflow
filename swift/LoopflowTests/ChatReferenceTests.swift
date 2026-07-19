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

    @Test("Detects an authored project:<slug> reference")
    func detectsProjectReference() {
        let matches = parseChatReferences(in: "advancing project:wave-chat this cycle")
        #expect(matches.count == 1)
        #expect(matches.first?.kind == .project)
        #expect(matches.first?.identifier == "wave-chat")
        #expect(matches.first?.displayText == "project:wave-chat")
    }

    @Test("Detects an authored evidence:<token> reference")
    func detectsEvidenceReference() {
        let matches = parseChatReferences(in: "see evidence:run-abc123 for the p95")
        #expect(matches.count == 1)
        #expect(matches.first?.kind == .evidence)
        #expect(matches.first?.identifier == "run-abc123")
        #expect(matches.first?.displayText == "evidence:run-abc123")
    }

    @Test("Project and evidence names without the authored prefix are left alone")
    func unprefixedNamesNotLinked() {
        // Real narration names these in prose with no identifier; only the
        // authored form links, so bare words never become false references.
        #expect(parseChatReferences(in: "the Loopflow API Project is alive").isEmpty)
        #expect(parseChatReferences(in: "generation 2 is the evidence here").isEmpty)
    }

    @Test("All four kinds coexist in one message, ordered")
    func allKindsOrdered() {
        let matches = parseChatReferences(
            in: "W2-174 lands as PR #889 under project:wave-chat, evidence:sha-9f2"
        )
        #expect(matches.map(\.kind) == [.task, .pullRequest, .project, .evidence])
        #expect(matches.map(\.identifier) == ["W2-174", "889", "wave-chat", "sha-9f2"])
    }

    @Test("An authored token wins over a Task match nested inside it")
    func authoredWinsOverNestedTask() {
        // `evidence:W2-174` is one evidence reference, not evidence wrapping a
        // stray W2-174 Task match; ranges never overlap.
        let matches = parseChatReferences(in: "logged evidence:W2-174 for the run")
        #expect(matches.count == 1)
        #expect(matches.first?.kind == .evidence)
        #expect(matches.first?.identifier == "W2-174")
    }

    @Test("project:evidence:x is one project reference, not an overlapping evidence match")
    func projectEvidenceOverlapResolvesToOneProject() {
        // `project:evidence:x` shares the `evidence` span: a project whose slug
        // is `evidence` and an evidence whose token is `x`. The outer keyword
        // (`project`, earliest start) claims the range; the evidence match is
        // dropped, so the two never overlap.
        let matches = parseChatReferences(in: "see project:evidence:x for detail")
        #expect(matches.count == 1)
        #expect(matches.first?.kind == .project)
        #expect(matches.first?.identifier == "evidence")
        #expect(matches.first?.displayText == "project:evidence")
    }

    @Test("evidence:project:foo is one evidence reference, not an overlapping project match")
    func evidenceProjectOverlapResolvesToOneEvidence() {
        // The symmetric case: the outer keyword is `evidence`, so it wins by
        // earliest start. The nested `project:foo` match is dropped.
        let matches = parseChatReferences(in: "see evidence:project:foo for detail")
        #expect(matches.count == 1)
        #expect(matches.first?.kind == .evidence)
        #expect(matches.first?.identifier == "project")
        #expect(matches.first?.displayText == "evidence:project")
    }

    @Test("Repeated authored keywords resolve without overlap")
    func repeatedAuthoredKeywordsDoNotOverlap() {
        // `project:project:foo` — the first `project:` claims `project:project`;
        // the nested second `project:foo` overlaps and is dropped.
        let matches = parseChatReferences(in: "see project:project:foo for detail")
        #expect(matches.count == 1)
        #expect(matches.first?.kind == .project)
        #expect(matches.first?.identifier == "project")
    }

    @Test("All authored references across a message stay non-overlapping")
    func authoredReferencesNonOverlapping() {
        let matches = parseChatReferences(
            in: "project:wave-chat added evidence:run-1 and evidence:run-2"
        )
        // project + two evidence, no overlap, ordered by position.
        #expect(matches.map(\.kind) == [.project, .evidence, .evidence])
        #expect(matches.map(\.identifier) == ["wave-chat", "run-1", "run-2"])
        for pair in zip(matches, matches.dropFirst()) {
            #expect(pair.0.range.upperBound <= pair.1.range.lowerBound)
        }
    }

    @Test("Plain prose with no references returns nothing quickly")
    func noReferences() {
        #expect(parseChatReferences(in: "The restarted Project Work is healthy.").isEmpty)
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
