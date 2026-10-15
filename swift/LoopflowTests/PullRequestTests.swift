#if os(macOS)
import Foundation
import Testing
@testable import LoopflowMac

@Suite("Compact GitHub pull request")
struct PullRequestTests {
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

    @Test func publishedDiffDisclosesMissingAndPartialPatchesAndRenames() throws {
        let data = Data(#"""
        [[{"filename":"new.txt","previous_filename":"old.txt","status":"renamed","additions":1,"deletions":1,
           "patch":"@@ -1 +1 @@\n-old\n+new","blob_url":"https://github.com/example/repo/blob/head/new.txt"}],
         [{"filename":"image.png","status":"added","additions":0,"deletions":0},
          {"filename":"large.txt","status":"modified","additions":40,"deletions":0,"patch":"@@ -0 +1 @@\n+one"}]]
        """#.utf8)
        let files = try JSONDecoder().decode([[GitHubPullRequestFile]].self, from: data).flatMap { $0 }
        #expect(files.count == 3)
        #expect(files[0].previousFilename == "old.txt")
        #expect(!files[0].incompletePatch)
        #expect(files[1].incompletePatch)
        #expect(files[2].incompletePatch)
    }
}
#endif
