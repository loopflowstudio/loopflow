import Foundation

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
