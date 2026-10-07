import Foundation

/// One bounded, ordered window from `lf activity`.
public struct WorkActivitySnapshot: Decodable, Sendable, Hashable {
    public let generatedAt: Int64
    public let since: Int64
    public let limit: Int
    public let truncated: Bool
    public let items: [WorkActivityEntry]

    enum CodingKeys: String, CodingKey {
        case since, limit, truncated, items
        case generatedAt = "generated_at"
    }
}

public struct WorkActivityEntry: Decodable, Sendable, Hashable, Identifiable {
    public let id: String
    public let recordedAt: Int64
    public let summary: String
    public let work: WorkReference
    public let subject: String
    public let fact: WorkActivityFact

    enum CodingKeys: String, CodingKey {
        case id, summary, work, subject, fact
        case recordedAt = "recorded_at"
    }
}

public enum WorkActivityFact: Decodable, Sendable, Hashable {
    case workCreated
    case inputCaptured(sessionId: String, captured: Int)
    case inputCompletionRecorded(sessionId: String, captured: Int, status: String)
    case providerHistoryRecorded(sessionId: String, captured: Int?, reference: ProviderHistoryReference, processLfid: String?, status: String?)
    case prStarted(id: String)
    case prPublishRequested(id: String, github: GithubPrSnapshot?)
    case prMergeRequested(
        id: String,
        request: PrMergeRequestSnapshot,
        github: GithubPrSnapshot?
    )
    case prMerged(id: String, github: GithubPrSnapshot?, mergeCommit: String)
    case prAbandoned(id: String, github: GithubPrSnapshot?)
    case steerIssued(id: Int, author: WorkAuthor)

    private enum CodingKeys: String, CodingKey {
        case kind, id, status, request, github, author
        case sessionId = "session_id"
        case captured
        case reference
        case processLfid = "process_lfid"
        case mergeCommit = "merge_commit"
    }

    private enum Kind: String, Decodable {
        case workCreated = "work_created"
        case inputCaptured = "input_captured"
        case inputCompletionRecorded = "input_completion_recorded"
        case providerHistoryRecorded = "provider_history_recorded"
        case prStarted = "pr_started"
        case prPublishRequested = "pr_publish_requested"
        case prMergeRequested = "pr_merge_requested"
        case prMerged = "pr_merged"
        case prAbandoned = "pr_abandoned"
        case steerIssued = "steer_issued"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(Kind.self, forKey: .kind) {
        case .workCreated:
            self = .workCreated
        case .inputCaptured:
            self = .inputCaptured(
                sessionId: try container.decode(String.self, forKey: .sessionId),
                captured: try container.decode(Int.self, forKey: .captured)
            )
        case .inputCompletionRecorded:
            self = .inputCompletionRecorded(
                sessionId: try container.decode(String.self, forKey: .sessionId),
                captured: try container.decode(Int.self, forKey: .captured),
                status: try container.decode(String.self, forKey: .status)
            )
        case .providerHistoryRecorded:
            self = .providerHistoryRecorded(sessionId: try container.decode(String.self, forKey: .sessionId),
                captured: try container.decodeIfPresent(Int.self, forKey: .captured),
                reference: try container.decode(ProviderHistoryReference.self, forKey: .reference),
                processLfid: try container.decodeIfPresent(String.self, forKey: .processLfid),
                status: try container.decodeIfPresent(String.self, forKey: .status))
        case .prStarted:
            self = .prStarted(id: try container.decode(String.self, forKey: .id))
        case .prPublishRequested:
            self = .prPublishRequested(
                id: try container.decode(String.self, forKey: .id),
                github: try container.decodeIfPresent(GithubPrSnapshot.self, forKey: .github)
            )
        case .prMergeRequested:
            self = .prMergeRequested(
                id: try container.decode(String.self, forKey: .id),
                request: try container.decode(PrMergeRequestSnapshot.self, forKey: .request),
                github: try container.decodeIfPresent(GithubPrSnapshot.self, forKey: .github)
            )
        case .prMerged:
            self = .prMerged(
                id: try container.decode(String.self, forKey: .id),
                github: try container.decodeIfPresent(GithubPrSnapshot.self, forKey: .github),
                mergeCommit: try container.decode(String.self, forKey: .mergeCommit)
            )
        case .prAbandoned:
            self = .prAbandoned(
                id: try container.decode(String.self, forKey: .id),
                github: try container.decodeIfPresent(GithubPrSnapshot.self, forKey: .github)
            )
        case .steerIssued:
            self = .steerIssued(
                id: try container.decode(Int.self, forKey: .id),
                author: try container.decode(WorkAuthor.self, forKey: .author)
            )
        }
    }



}

public extension WorkActivityFact {
    var github: GithubPrSnapshot? {
        switch self {
        case .prPublishRequested(_, let github),
             .prMergeRequested(_, _, let github),
             .prMerged(_, let github, _),
             .prAbandoned(_, let github):
            github
        default:
            nil
        }
    }
}
