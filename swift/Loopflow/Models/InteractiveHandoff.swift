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
}
