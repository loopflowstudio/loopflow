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
    case runStarted(invocationId: String, traceId: String, execId: String)
    case runFinished(invocationId: String, traceId: String, execId: String, status: String)
    case prStarted(id: String)
    case prPublishRequested(id: String, github: GithubPrSnapshot?)
    case prMergeRequested(
        id: String,
        request: PrMergeRequestSnapshot,
        github: GithubPrSnapshot?
    )
    case prMerged(id: String, github: GithubPrSnapshot?, mergeCommit: String)
    case prAbandoned(id: String, github: GithubPrSnapshot?)
    case steerIssued(id: String, author: WorkAuthor)

    private enum CodingKeys: String, CodingKey {
        case kind, id, status, request, github, author
        case invocationId = "invocation_id"
        case traceId = "trace_id"
        case execId = "exec_id"
        case mergeCommit = "merge_commit"
    }

    private enum Kind: String, Decodable {
        case workCreated = "work_created"
        case runStarted = "run_started"
        case runFinished = "run_finished"
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
        case .runStarted:
            self = .runStarted(
                invocationId: try container.decode(String.self, forKey: .invocationId),
                traceId: try container.decode(String.self, forKey: .traceId),
                execId: try container.decode(String.self, forKey: .execId)
            )
        case .runFinished:
            self = .runFinished(
                invocationId: try container.decode(String.self, forKey: .invocationId),
                traceId: try container.decode(String.self, forKey: .traceId),
                execId: try container.decode(String.self, forKey: .execId),
                status: try container.decode(String.self, forKey: .status)
            )
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
                id: try container.decode(String.self, forKey: .id),
                author: try container.decode(WorkAuthor.self, forKey: .author)
            )
        }
    }
}

public extension WorkActivityFact {
    var invocationId: String? {
        switch self {
        case .runStarted(let invocationId, _, _),
             .runFinished(let invocationId, _, _, _):
            invocationId
        default:
            nil
        }
    }

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
