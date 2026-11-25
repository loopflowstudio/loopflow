#if os(macOS)
import Foundation
import Loopflow
import Observation

/// GitHub's published PR, independent of local uncommitted changes.
struct GitHubPullRequest: Decodable, Sendable {
    let number: Int
    let url: URL
    let title: String
    let body: String
    let state: String
    let isDraft: Bool
    let additions: Int
    let deletions: Int
    let changedFiles: Int
    let headRefOid: String
    let baseRefOid: String
    let headRefName: String
    let baseRefName: String
    let reviewDecision: String
    let reviewRequests: [Reviewer]
    let statusCheckRollup: [Check]?

    struct Reviewer: Decodable, Sendable {
        let login: String?
        let name: String?
        let slug: String?
        var label: String { login ?? slug ?? name ?? "Unknown reviewer" }
    }

    struct Check: Decodable, Sendable {
        let name: String?
        let context: String?
        let status: String?
        let conclusion: String?
        let state: String?
        let detailsUrl: URL?
        let targetUrl: URL?

        var label: String { name ?? context ?? "Check" }
        var result: String {
            if let conclusion, !conclusion.isEmpty { return conclusion }
            return state ?? status ?? "UNKNOWN"
        }
        var link: URL? { detailsUrl ?? targetUrl }
        var passed: Bool { result == "SUCCESS" }
        var skipped: Bool { ["NEUTRAL", "SKIPPED"].contains(result) }
        var pending: Bool { ["QUEUED", "IN_PROGRESS", "PENDING", "WAITING", "REQUESTED"].contains(result) }
        var failed: Bool { ["FAILURE", "ERROR", "CANCELLED", "TIMED_OUT", "ACTION_REQUIRED", "STARTUP_FAILURE"].contains(result) }
    }

    var checksSummary: String {
        guard let checks = statusCheckRollup else { return "Check status unavailable" }
        if checks.isEmpty { return "No checks reported" }
        let groups = [
            (checks.filter(\.passed).count, "passed"),
            (checks.filter(\.failed).count, "failed"),
            (checks.filter(\.pending).count, "pending"),
            (checks.filter(\.skipped).count, "skipped"),
            (checks.filter { !$0.passed && !$0.failed && !$0.pending && !$0.skipped }.count, "unknown")
        ]
        return groups.filter { $0.0 > 0 }.map { "\($0.0) \($0.1)" }.joined(separator: " · ")
    }

    var statusLabel: String { isDraft && state == "OPEN" ? "Draft" : state.capitalized }
    var reviewLabel: String {
        switch reviewDecision {
        case "APPROVED": "Approved"
        case "CHANGES_REQUESTED": "Changes requested"
        case "REVIEW_REQUIRED": "Review required"
        default: "No review decision"
        }
    }
}

@MainActor
@Observable
final class PullRequestStore {
    private(set) var pullRequest: GitHubPullRequest?
    private(set) var error: String?
    private(set) var refreshing = false
    /// When GitHub last answered; a retained reading keeps its own time.
    private(set) var readAt: Date?
    private var requestedURL: URL?
    private var generation = 0

    func refresh(_ url: URL?) async {
        generation += 1
        let generation = generation
        if requestedURL != url || url == nil {
            pullRequest = nil
            error = nil
            readAt = nil
        }
        requestedURL = url
        guard let url else {
            refreshing = false
            return
        }
        refreshing = true
        defer { if self.generation == generation { refreshing = false } }
        do {
            let text = try await LocalWaveAgentLauncher.queryGitHub([
                "pr", "view", url.absoluteString, "--json",
                "number,url,title,body,state,isDraft,additions,deletions,changedFiles,headRefOid,baseRefOid,headRefName,baseRefName,reviewDecision,reviewRequests,statusCheckRollup"
            ])
            let next = try JSONDecoder().decode(GitHubPullRequest.self, from: Data(text.utf8))
            guard !Task.isCancelled, generation == self.generation else { return }
            pullRequest = next
            error = nil
            readAt = Date()
        } catch {
            guard !Task.isCancelled, generation == self.generation else { return }
            self.error = error.localizedDescription
        }
    }
}

/// What the Changes page says about delivery: Loopflow's record decides whether
/// a PR exists; GitHub's reading adds checks and review only while it is fresh.
/// Nothing here reads as Task completion.
struct TaskDelivery: Equatable {
    let headline: String
    let detail: String?
    let warning: Bool
    /// Whether check and review results describe the PR as it is now.
    let readingIsCurrent: Bool

    /// A reading this old no longer stands for the PR.
    static let staleAfter: TimeInterval = 120

    /// `runsOn` names the Machine whose checkout the changes come from when it
    /// is not this Mac.
    static func describe(record: PrSnapshot?, taskOpen: Bool, reading: GitHubPullRequest?, error: String?,
                         readAt: Date?, localHead: String?, runsOn: String?, now: Date = .now) -> TaskDelivery {
        let stillOpen = taskOpen ? "Task still open" : nil
        let place = runsOn.map { "on \($0)" } ?? "local"
        let none = TaskDelivery(headline: "No pull request yet. Changes below are \(place).", detail: nil,
                                warning: false, readingIsCurrent: false)
        guard let record else { return none }
        switch record.phase {
        case .working:
            return none
        case .merged:
            let sha = record.mergeCommit.map { " \($0.prefix(10))" } ?? ""
            return TaskDelivery(headline: "Merged\(sha)", detail: stillOpen, warning: false, readingIsCurrent: false)
        case .abandoned:
            return TaskDelivery(headline: "Abandoned", detail: stillOpen, warning: false, readingIsCurrent: false)
        case .publishing, .open:
            guard let github = record.publication?.github else {
                let requested = record.publication.map { " · requested \($0.requestedAt)" } ?? ""
                return TaskDelivery(headline: "Publishing…\(requested)", detail: nil, warning: false, readingIsCurrent: false)
            }
            let age = readAt.map { Self.age(now.timeIntervalSince($0)) }
            if let error {
                return TaskDelivery(headline: "GitHub unavailable: \(error)",
                                    detail: age.map { "Last read \($0) ago" }, warning: true, readingIsCurrent: false)
            }
            guard let reading, let readAt, let age else {
                return TaskDelivery(headline: "Open #\(github.number) · reading GitHub…", detail: nil,
                                    warning: false, readingIsCurrent: false)
            }
            let state = "\(reading.statusLabel) #\(reading.number)"
            if now.timeIntervalSince(readAt) > staleAfter {
                return TaskDelivery(headline: "\(state) · stale · read \(age) ago", detail: nil,
                                    warning: true, readingIsCurrent: false)
            }
            if let localHead, localHead != reading.headRefOid {
                let checkout = runsOn.map { "The checkout on \($0)" } ?? "Local checkout"
                return TaskDelivery(headline: "\(state) · read \(age) ago", detail: "\(checkout) is ahead of the PR",
                                    warning: true, readingIsCurrent: false)
            }
            let detail = reading.state == "MERGED" ? stillOpen : reading.reviewLabel
            return TaskDelivery(headline: "\(state) · \(reading.checksSummary) · read \(age) ago", detail: detail,
                                warning: false, readingIsCurrent: true)
        }
    }

    static func age(_ seconds: TimeInterval) -> String {
        let seconds = max(0, Int(seconds))
        if seconds < 60 { return "\(seconds) s" }
        if seconds < 3600 { return "\(seconds / 60) min" }
        if seconds < 86_400 { return "\(seconds / 3600) h" }
        return "\(seconds / 86_400) d"
    }
}
#endif
