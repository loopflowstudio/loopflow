#if os(macOS)
import Foundation
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

struct GitHubPullRequestFile: Decodable, Sendable, Identifiable {
    let filename: String
    let previousFilename: String?
    let status: String
    let additions: Int
    let deletions: Int
    let patch: String?
    let blobURL: URL?
    var id: String { filename }

    enum CodingKeys: String, CodingKey {
        case filename, status, additions, deletions, patch
        case previousFilename = "previous_filename"
        case blobURL = "blob_url"
    }

    var incompletePatch: Bool {
        guard let patch else { return true }
        let lines = patch.split(separator: "\n", omittingEmptySubsequences: false)
        return lines.filter { $0.hasPrefix("+") }.count < additions
            || lines.filter { $0.hasPrefix("-") }.count < deletions
    }
}

@MainActor
@Observable
final class PullRequestStore {
    private(set) var pullRequest: GitHubPullRequest?
    private(set) var files: [GitHubPullRequestFile]?
    private(set) var error: String?
    private(set) var filesError: String?
    private(set) var refreshing = false
    private(set) var loadingFiles = false
    private(set) var updatedAt: Date?
    var showsFiles = false
    var expandedFiles: Set<String> = []
    private var requestedURL: URL?
    private var generation = 0
    private var fileGeneration = 0

    func refresh(_ url: URL?) async {
        generation += 1
        let generation = generation
        if requestedURL != url || url == nil {
            pullRequest = nil
            files = nil
            error = nil
            filesError = nil
            showsFiles = false
            expandedFiles.removeAll()
            updatedAt = nil
        }
        requestedURL = url
        fileGeneration += 1
        loadingFiles = false
        guard let url else {
            refreshing = false
            return
        }
        refreshing = true
        defer { if self.generation == generation { refreshing = false } }
        do {
            let next = try await Task.detached(priority: .userInitiated) {
                let text = try LocalWaveAgentLauncher.queryGitHub([
                    "pr", "view", url.absoluteString, "--json",
                    "number,url,title,body,state,isDraft,additions,deletions,changedFiles,headRefOid,baseRefOid,headRefName,baseRefName,reviewDecision,reviewRequests,statusCheckRollup"
                ])
                return try JSONDecoder().decode(GitHubPullRequest.self, from: Data(text.utf8))
            }.value
            guard !Task.isCancelled, generation == self.generation else { return }
            files = nil
            filesError = nil
            pullRequest = next
            error = nil
            updatedAt = Date()
        } catch {
            guard !Task.isCancelled, generation == self.generation else { return }
            self.error = error.localizedDescription
        }
    }

    func loadFiles() async {
        guard let pr = pullRequest, files == nil else { return }
        fileGeneration += 1
        let generation = fileGeneration
        loadingFiles = true
        defer { if fileGeneration == generation { loadingFiles = false } }
        do {
            let next = try await Task.detached(priority: .userInitiated) {
                let parts = pr.url.pathComponents.filter { $0 != "/" }
                guard parts.count >= 4, parts[2] == "pull", let host = pr.url.host else {
                    throw LocalLfError(errorDescription: "GitHub PR URL is unavailable.")
                }
                let text = try LocalWaveAgentLauncher.queryGitHub([
                    "api", "--hostname", host, "--paginate", "--slurp",
                    "repos/\(parts[0])/\(parts[1])/pulls/\(pr.number)/files?per_page=100"
                ])
                let pages = try JSONDecoder().decode([[GitHubPullRequestFile]].self, from: Data(text.utf8))
                // Files are read from the PR's moving head; do not label another head's patch with this one.
                let head = try LocalWaveAgentLauncher.queryGitHub([
                    "pr", "view", pr.url.absoluteString, "--json", "headRefOid,baseRefOid", "--jq", "[.headRefOid, .baseRefOid] | join(\" \")"
                ]).trimmingCharacters(in: .whitespacesAndNewlines)
                guard head == "\(pr.headRefOid) \(pr.baseRefOid)" else {
                    throw LocalLfError(errorDescription: "The PR changed while loading diffs. Refresh to read the new head.")
                }
                return pages.flatMap { $0 }
            }.value
            guard !Task.isCancelled, generation == fileGeneration, pullRequest?.url == pr.url, pullRequest?.headRefOid == pr.headRefOid, pullRequest?.baseRefOid == pr.baseRefOid else { return }
            files = next
            filesError = nil
        } catch {
            guard !Task.isCancelled, generation == fileGeneration, pullRequest?.url == pr.url, pullRequest?.headRefOid == pr.headRefOid, pullRequest?.baseRefOid == pr.baseRefOid else { return }
            filesError = error.localizedDescription
        }
    }
}
#endif
