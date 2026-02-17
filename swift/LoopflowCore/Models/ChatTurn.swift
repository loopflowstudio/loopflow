import Foundation

public enum ChatTurnStatus: String, Sendable, Hashable {
    case running
    case completed
    case failed
}

public struct ChatTurn: Sendable, Hashable {
    public var id: String
    public var waveId: String
    public var status: ChatTurnStatus

    public init(id: String, waveId: String, status: ChatTurnStatus) {
        self.id = id
        self.waveId = waveId
        self.status = status
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
