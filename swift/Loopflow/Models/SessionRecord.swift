import Foundation

public enum SessionState: String, Codable, Sendable, Hashable {
    case waiting
    case active
    case ready
    case closed
}

public enum SessionKind: String, Codable, Sendable, Hashable {
    case ask
    case flow
    case interactive
}

public enum SessionActionKind: String, Codable, Sendable, Hashable {
    case open
    case moveHere = "move_here"
    case complete
    case approve
    case iterate
}

public struct SessionAction: Codable, Sendable, Hashable {
    public let kind: SessionActionKind
    public let label: String
    public let help: String
    public let unavailableReason: String?

    enum CodingKeys: String, CodingKey {
        case kind, label, help
        case unavailableReason = "unavailable_reason"
    }
}

/// One resumable Session and the exact command that opens it.
/// Rust owns completion, FlowStep decisions, and provider-client liveness.
public struct SessionRecord: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let kind: SessionKind
    public let work: WorkReference?
    public let waveId: String?
    public let workPath: String?
    public let actions: [SessionAction]
    public let title: String
    public let detail: String
    public let cwd: String
    public let state: SessionState
    public let readySummary: String?
    public let openArgv: [String]
    public let terminalIds: [String]

    public func action(_ kind: SessionActionKind) -> SessionAction? {
        actions.first { $0.kind == kind }
    }

    enum CodingKeys: String, CodingKey {
        case id, kind, work, title, detail, cwd, state
        case waveId = "wave_id"
        case actions
        case workPath = "work_path"
        case readySummary = "ready_summary"
        case openArgv = "open_argv"
        case terminalIds = "terminal_ids"
    }
}
