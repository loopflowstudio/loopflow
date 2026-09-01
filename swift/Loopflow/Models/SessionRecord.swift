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

/// One resumable Session and the exact command that opens it.
/// Rust owns completion, FlowStep decisions, and provider-client liveness.
public struct SessionRecord: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let kind: SessionKind
    public let work: WorkReference?
    public let title: String
    public let detail: String
    public let cwd: String
    public let state: SessionState
    public let readySummary: String?
    public let openArgv: [String]

    enum CodingKeys: String, CodingKey {
        case id, kind, work, title, detail, cwd, state
        case readySummary = "ready_summary"
        case openArgv = "open_argv"
    }
}
