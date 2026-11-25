#if os(macOS)
import Foundation
import Loopflow
import Testing
@testable import LoopflowMac

@Suite("Compact GitHub pull request")
struct PullRequestTests {
    @Test @MainActor func removingPublicationClearsTheReading() async {
        let store = PullRequestStore()
        await store.refresh(nil)
        #expect(store.pullRequest == nil && store.error == nil && store.readAt == nil)
        #expect(!store.refreshing)
    }

    @Test func githubReadingDistinguishesPendingFailedAndUnavailableChecks() throws {
        let json = #"""
        {"number":12,"url":"https://github.com/example/repo/pull/12","title":"A change","body":"Details",
         "state":"OPEN","isDraft":true,"additions":10,"deletions":3,"changedFiles":2,
         "headRefOid":"head","baseRefOid":"base","headRefName":"feature","baseRefName":"main",
         "reviewDecision":"REVIEW_REQUIRED","reviewRequests":[{"login":"reviewer"},{"slug":"team"}],
         "statusCheckRollup":[
           {"name":"Build","status":"IN_PROGRESS","conclusion":"","detailsUrl":"https://github.com/example/repo/actions/runs/1"},
           {"name":"Tests","status":"COMPLETED","conclusion":"FAILURE"},
           {"context":"legacy-ci","state":"SUCCESS","targetUrl":"https://example.com/ci"},
           {"name":"Future check","status":"UNKNOWN"}]}
        """#
        let pr = try JSONDecoder().decode(GitHubPullRequest.self, from: Data(json.utf8))
        #expect(pr.statusLabel == "Draft")
        #expect(pr.reviewLabel == "Review required")
        #expect(pr.reviewRequests.map(\.label) == ["reviewer", "team"])
        let checks = try #require(pr.statusCheckRollup)
        #expect(checks[0].result == "IN_PROGRESS")
        #expect(!checks[0].passed && !checks[0].failed)
        #expect(checks[1].failed)
        #expect(checks[2].passed)
        #expect(checks[2].label == "legacy-ci")
        #expect(checks[2].link?.host == "example.com")
        #expect(!checks[3].passed)
        #expect(pr.checksSummary == "1 passed · 1 failed · 1 pending · 1 unknown")
        let unavailable = json.replacingOccurrences(of: "\"statusCheckRollup\":[", with: "\"statusCheckRollup\":null,\"ignored\":[")
        #expect(try JSONDecoder().decode(GitHubPullRequest.self, from: Data(unavailable.utf8)).statusCheckRollup == nil)
    }

    // MARK: Delivery on the Changes page

    private static let reading = #"""
    {"number":12,"url":"https://github.com/example/repo/pull/12","title":"A change","body":"",
     "state":"OPEN","isDraft":false,"additions":1,"deletions":1,"changedFiles":1,
     "headRefOid":"head","baseRefOid":"base","headRefName":"feature","baseRefName":"main",
     "reviewDecision":"REVIEW_REQUIRED","reviewRequests":[],
     "statusCheckRollup":[{"name":"Build","status":"COMPLETED","conclusion":"SUCCESS"}]}
    """#

    private func record(_ phase: String, github: Bool = true, merge: String? = nil) throws -> PrSnapshot {
        let publication = github
            ? #"{"requested_at":"2026-11-14T10:00:00Z","presentation":null,"github":{"number":12,"url":"https://github.com/example/repo/pull/12"},"merge":null}"#
            : #"{"requested_at":"2026-11-14T10:00:00Z","presentation":null,"github":null,"merge":null}"#
        let json = """
        {"id":"pr","sequence":1,"slug":"s","branch":"b","base_commit":"base","phase":"\(phase)","empty":false,
         "publication":\(phase == "working" ? "null" : publication),
         "merge_commit":\(merge.map { "\"\($0)\"" } ?? "null"),"abandoned_at":null}
        """
        return try JSONDecoder().decode(PrSnapshot.self, from: Data(json.utf8))
    }

    @Test func deliveryDistinguishesEveryStateWithDistinctText() throws {
        let now = Date()
        let pr = try JSONDecoder().decode(GitHubPullRequest.self, from: Data(Self.reading.utf8))
        func describe(_ record: PrSnapshot?, reading: GitHubPullRequest? = nil, error: String? = nil,
                      readAt: Date? = nil, head: String? = "head", runsOn: String? = nil) -> TaskDelivery {
            TaskDelivery.describe(record: record, taskOpen: true, reading: reading, error: error,
                                  readAt: readAt, localHead: head, runsOn: runsOn, now: now)
        }
        let none = describe(nil)
        let working = describe(try record("working"))
        let publishing = describe(try record("publishing", github: false))
        let open = describe(try record("open"), reading: pr, readAt: now.addingTimeInterval(-20))
        let stale = describe(try record("open"), reading: pr, readAt: now.addingTimeInterval(-600))
        let ahead = describe(try record("open"), reading: pr, readAt: now.addingTimeInterval(-20), head: "newer")
        let unavailable = describe(try record("open"), reading: pr, error: "gh: offline", readAt: now.addingTimeInterval(-840))
        let merged = describe(try record("merged", merge: "abcdef0123456789"))
        let abandoned = describe(try record("abandoned"))

        #expect(none.headline == "No pull request yet. Changes below are local.")
        // A Task another Machine runs never calls its changes local.
        #expect(describe(nil, runsOn: "mini").headline == "No pull request yet. Changes below are on mini.")
        #expect(describe(try record("open"), reading: pr, readAt: now.addingTimeInterval(-20), head: "newer",
                         runsOn: "mini").detail == "The checkout on mini is ahead of the PR")
        #expect(working == none)
        #expect(publishing.headline == "Publishing… · requested 2026-11-14T10:00:00Z")
        #expect(open.headline == "Open #12 · 1 passed · read 20 s ago")
        #expect(open.detail == "Review required" && open.readingIsCurrent)
        #expect(stale.headline == "Open #12 · stale · read 10 min ago" && !stale.readingIsCurrent)
        #expect(ahead.detail == "Local checkout is ahead of the PR" && !ahead.readingIsCurrent)
        #expect(unavailable.headline == "GitHub unavailable: gh: offline")
        #expect(unavailable.detail == "Last read 14 min ago" && !unavailable.readingIsCurrent)
        // A merged PR never reads as a finished Task.
        #expect(merged.headline == "Merged abcdef0123" && merged.detail == "Task still open")
        #expect(abandoned.headline == "Abandoned" && abandoned.detail == "Task still open")
        let headlines = [none, publishing, open, stale, unavailable, merged, abandoned].map(\.headline)
        #expect(Set(headlines).count == headlines.count)
    }
}
#endif
