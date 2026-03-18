import Foundation

/// Two attention paths:
/// - `interactive`: `lf` is at a step that needs a human
/// - `algedonic`: something is wrong and the system is escalating
public enum AttentionKind: String, Sendable, CaseIterable, Hashable {
    case interactive
    case algedonic

    public var icon: String {
        switch self {
        case .interactive: return "person.fill.questionmark"
        case .algedonic: return "exclamationmark.triangle"
        }
    }

    public var label: String {
        switch self {
        case .interactive: return "Interactive"
        case .algedonic: return "Escalation"
        }
    }

    /// Accept legacy kind strings during migration.
    public init?(rawValue: String) {
        switch rawValue {
        case "interactive": self = .interactive
        case "algedonic": self = .algedonic
        case "design_review", "code_review", "calibration": self = .interactive
        case "queue_failure", "step_failure": self = .algedonic
        default: return nil
        }
    }
}

public enum AttentionStatus: String, Sendable, CaseIterable, Hashable {
    case surfaced
    case viewed
    case resolved
}

/// Context for interactive attention items (step needs human input).
public struct InteractiveAttentionContext: Sendable, Hashable {
    public let step: String?
    public let terminalSessionId: String?
    public let designPath: String?
}

<<<<<<< HEAD
<<<<<<< HEAD
=======
>>>>>>> bb36fbcb (attention: collapse kinds to Interactive/Algedonic, add HTTP create/resolve API)
/// Context for algedonic attention items (system escalation).
public struct AlgedonicAttentionContext: Sendable, Hashable {
    public let step: String?
    public let error: String?
    public let reason: String?
    public let conflictFiles: [String]
<<<<<<< HEAD
}

public enum AttentionContext: Sendable, Hashable {
    case interactive(InteractiveAttentionContext)
    case algedonic(AlgedonicAttentionContext)
=======
public struct DesignReviewAttentionContext: Sendable, Hashable {
    public let step: String
    public let designPath: String?
    public let terminalSessionId: String?
}

public struct CalibrationAttentionContext: Sendable, Hashable {
    public let step: String
    public let chordPath: String?
    public let terminalSessionId: String?
=======
>>>>>>> bb36fbcb (attention: collapse kinds to Interactive/Algedonic, add HTTP create/resolve API)
}

public enum AttentionContext: Sendable, Hashable {
    case interactive(InteractiveAttentionContext)
    case algedonic(AlgedonicAttentionContext)
    case raw(String)
>>>>>>> 07c9c6ed (attention queue completion: wire interactive steps into attention queue)
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
<<<<<<< HEAD
<<<<<<< HEAD
=======
>>>>>>> bb36fbcb (attention: collapse kinds to Interactive/Algedonic, add HTTP create/resolve API)
    /// Parse typed context from JSON based on kind.
    static func context(kind: AttentionKind, json: [String: Any]) -> AttentionContext {
        switch kind {
        case .interactive:
            return .interactive(
                InteractiveAttentionContext(
<<<<<<< HEAD
=======
    /// Parse typed context from JSON based on discriminator fields.
    /// Context type is determined by the `step` field first, then payload shape.
    static func context(json: [String: Any]) -> AttentionContext {
        let step = json["step"] as? String ?? ""
        let terminalSessionId = json["terminal_session_id"] as? String

        // Design review: step starts with "code/design"
        if step.hasPrefix("code/design") {
            return .designReview(
                DesignReviewAttentionContext(
                    step: step,
                    designPath: json["design_path"] as? String,
                    terminalSessionId: terminalSessionId
                )
            )
        }
        // Calibration: step starts with "chord/"
        if step.hasPrefix("chord/") {
            return .calibration(
                CalibrationAttentionContext(
                    step: step,
                    chordPath: json["design_path"] as? String,
                    terminalSessionId: terminalSessionId
                )
            )
        }
        // Queue failure: has "reason" field
        if json["reason"] != nil {
            return .queueFailure(
                QueueFailureAttentionContext(
                    reason: json["reason"] as? String,
                    conflictFiles: json["conflict_files"] as? [String] ?? [],
                    error: json["error"] as? String
                )
            )
        }
        // Code review: has "pr_url" field
        if json["pr_url"] != nil {
            let url = (json["pr_url"] as? String).flatMap(URL.init(string:))
            return .codeReview(
                CodeReviewAttentionContext(
                    prURL: url,
                    prNumber: json["pr_number"] as? Int,
                    prTitle: json["pr_title"] as? String,
                    branch: json["branch"] as? String
                )
            )
        }
        // Step failure: has "error" field
        if json["error"] != nil {
            return .stepFailure(
                StepFailureAttentionContext(
>>>>>>> 07c9c6ed (attention queue completion: wire interactive steps into attention queue)
=======
>>>>>>> bb36fbcb (attention: collapse kinds to Interactive/Algedonic, add HTTP create/resolve API)
                    step: json["step"] as? String,
                    terminalSessionId: json["terminal_session_id"] as? String,
                    designPath: json["design_path"] as? String
                )
            )
        case .algedonic:
            return .algedonic(
                AlgedonicAttentionContext(
                    step: json["step"] as? String,
                    error: json["error"] as? String,
                    reason: json["reason"] as? String,
                    conflictFiles: json["conflict_files"] as? [String] ?? []
                )
            )
        }
    }
}
