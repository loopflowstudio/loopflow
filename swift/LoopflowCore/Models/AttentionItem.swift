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

/// Context for algedonic attention items (system escalation).
public struct AlgedonicAttentionContext: Sendable, Hashable {
    public let step: String?
    public let error: String?
    public let reason: String?
    public let conflictFiles: [String]
}

public enum AttentionContext: Sendable, Hashable {
    case interactive(InteractiveAttentionContext)
    case algedonic(AlgedonicAttentionContext)
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
    /// Parse typed context from JSON based on kind.
    static func context(kind: AttentionKind, json: [String: Any]) -> AttentionContext {
        switch kind {
        case .interactive:
            return .interactive(
                InteractiveAttentionContext(
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
