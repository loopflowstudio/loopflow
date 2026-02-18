import Foundation

public struct ChatMessageRecord: Sendable, Hashable {
    public let id: String
    public let role: String
    public let content: String
    public let createdAt: Date?

    public init(id: String, role: String, content: String, createdAt: Date?) {
        self.id = id
        self.role = role
        self.content = content
        self.createdAt = createdAt
    }
}

public enum ChatTurnPhase: String, Sendable, Hashable {
    case progress
    case final
}

public enum ChatTurnEvent: Sendable, Hashable {
    case message(content: String, phase: ChatTurnPhase)
    case memoryEdit(op: String, block: String, detail: String)
    case done
    case failed(code: String, message: String)

    public var isTerminal: Bool {
        switch self {
        case .done, .failed:
            return true
        case .message, .memoryEdit:
            return false
        }
    }
}
