import Foundation

public enum AttentionKind: String, Sendable, CaseIterable, Hashable {
    case designReview = "design_review"
    case codeReview = "code_review"
    case calibration
    case queueFailure = "queue_failure"
    case stepFailure = "step_failure"

    public var icon: String {
        switch self {
        case .designReview: return "doc.text.magnifyingglass"
        case .codeReview: return "checkmark.seal"
        case .calibration: return "dial.high"
        case .queueFailure: return "exclamationmark.triangle"
        case .stepFailure: return "xmark.octagon"
        }
    }

    public var label: String {
        switch self {
        case .designReview: return "Design"
        case .codeReview: return "Review"
        case .calibration: return "Calibration"
        case .queueFailure: return "Queue"
        case .stepFailure: return "Failure"
        }
    }
}

public enum AttentionStatus: String, Sendable, CaseIterable, Hashable {
    case surfaced
    case viewed
    case resolved
}

/// Context for code review attention items.
public struct CodeReviewAttentionContext: Sendable, Hashable {
    public let prURL: URL?
    public let prNumber: Int?
    public let prTitle: String?
    public let branch: String?
}

/// Context for queue failure attention items (rebase conflicts, blocked queues).
public struct QueueFailureAttentionContext: Sendable, Hashable {
    public let reason: String?
    public let conflictFiles: [String]
    public let error: String?
}

/// Context for step failure attention items (agent crashed, build failed).
public struct StepFailureAttentionContext: Sendable, Hashable {
    public let step: String?
    public let terminalSessionId: String?
    public let designPath: String?
}

public enum AttentionContext: Sendable, Hashable {
    case codeReview(CodeReviewAttentionContext)
    case queueFailure(QueueFailureAttentionContext)
    case stepFailure(StepFailureAttentionContext)
    case raw(String)
}

public struct AttentionItem: Identifiable, Sendable, Hashable {
    public let id: String
    public let waveId: String
    public let runId: String?
    public let kind: AttentionKind
    public var status: AttentionStatus
    public let title: String
    public let summary: String
    public let context: AttentionContext
    public let surfacedAt: Date
    public var viewedAt: Date?
    public var resolvedAt: Date?

    public init(
        id: String,
        waveId: String,
        runId: String?,
        kind: AttentionKind,
        status: AttentionStatus,
        title: String,
        summary: String,
        context: AttentionContext,
        surfacedAt: Date,
        viewedAt: Date? = nil,
        resolvedAt: Date? = nil
    ) {
        self.id = id
        self.waveId = waveId
        self.runId = runId
        self.kind = kind
        self.status = status
        self.title = title
        self.summary = summary
        self.context = context
        self.surfacedAt = surfacedAt
        self.viewedAt = viewedAt
        self.resolvedAt = resolvedAt
    }

    public var isResolved: Bool { status == .resolved }
}

public extension AttentionItem {
    static func context(kind: AttentionKind, json: [String: Any]) -> AttentionContext {
        switch kind {
        case .codeReview:
            let url = (json["pr_url"] as? String).flatMap(URL.init(string:))
            return .codeReview(
                CodeReviewAttentionContext(
                    prURL: url,
                    prNumber: json["pr_number"] as? Int,
                    prTitle: json["pr_title"] as? String,
                    branch: json["branch"] as? String
                )
            )
        case .queueFailure:
            return .queueFailure(
                QueueFailureAttentionContext(
                    reason: json["reason"] as? String,
                    conflictFiles: json["conflict_files"] as? [String] ?? [],
                    error: json["error"] as? String
                )
            )
        case .stepFailure:
            return .stepFailure(
                StepFailureAttentionContext(
                    step: json["step"] as? String,
                    terminalSessionId: json["terminal_session_id"] as? String,
                    designPath: json["design_path"] as? String
                )
            )
        case .designReview, .calibration:
            fallthrough
        @unknown default:
            let data = (try? JSONSerialization.data(withJSONObject: json, options: [.sortedKeys])) ?? Data()
            return .raw(String(data: data, encoding: .utf8) ?? "{}")
        }
    }
}
