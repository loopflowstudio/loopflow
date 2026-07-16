import Foundation

/// Store-direct `lf handoff attach --json` contract. Presentations consume the
/// structured argv and environment; terminal bytes remain in the terminal.
public struct InteractiveHandoffAttach: Codable, Sendable, Hashable {
    public let sessionId: String
    public let status: InteractiveHandoffStatus
    public let cwd: String
    public let host: String
    public let environment: [String: String]
    public let argv: [String]

    enum CodingKeys: String, CodingKey {
        case status, cwd, host, environment, argv
        case sessionId = "session_id"
    }
}

public enum InteractiveHandoffStatus: String, Codable, Sendable, Hashable {
    case waiting, attached, completed, failed
    case handedBack = "handed_back"

    /// A handoff still expecting a human — its parent is blocked on it. Terminal
    /// outcomes are no longer active. Mirrors Rust `InteractiveHandoff::is_active`.
    public var isActive: Bool {
        switch self {
        case .waiting, .attached: true
        case .completed, .handedBack, .failed: false
        }
    }
}

/// One row of `lf handoff list --json`. Enough to census and display a durable
/// handoff — identity, declared parent, provider, Home, reason, age — without a
/// second read. Deciding to Open re-fetches the attach descriptor through
/// `lf handoff attach`, so this row never carries argv or environment.
/// Mirrors Rust `InteractiveHandoffListRow`. `ageSecs` is nil when the timestamp
/// could not be read; an unknown age is never a zero one.
public struct InteractiveHandoffListRow: Codable, Sendable, Hashable, Identifiable {
    public var id: String { sessionId }

    public let sessionId: String
    public let parentKind: String
    public let parentId: String
    public let waveId: String
    public let status: InteractiveHandoffStatus
    public let provider: String
    public let providerSessionId: String?
    public let home: String
    public let cwd: String
    public let reason: String
    public let createdAt: String
    public let updatedAt: String
    public let ageSecs: Int?

    enum CodingKeys: String, CodingKey {
        case status, provider, home, cwd, reason
        case sessionId = "session_id"
        case parentKind = "parent_kind"
        case parentId = "parent_id"
        case waveId = "wave_id"
        case providerSessionId = "provider_session_id"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
        case ageSecs = "age_secs"
    }
}
