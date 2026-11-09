import Foundation

/// A ticket occupies its position in one request's stream while other IDs keep
/// flowing. Callers serialize delivery and completion with their reader.
public struct ServerMessageOrder {
    private var waiting: [String: [Data]] = [:]
    private var bytes = 0
    private var count = 0

    public init() {}

    public mutating func receive(_ line: Data) throws -> ServerMessage? {
        let message = try JSONDecoder().decode(ServerMessage.self, from: line)
        guard let id = message.requestID else { return message }
        if waiting[id] != nil {
            guard count < 64, bytes + line.count <= ServerProtocol.maxBulk else {
                throw RegistryQueryError("Server updates fell behind a large message; reconnect for current Work")
            }
            waiting[id]?.append(line)
            count += 1
            bytes += line.count
            return nil
        }
        if case .bulk = message {
            guard waiting.count < 64 else {
                throw RegistryQueryError("Too many server transfers; reconnect for current Work")
            }
            waiting[id] = []
        }
        return message
    }

    /// Deliver the payload first, then feed these lines back through `receive`:
    /// a second ticket may defer the remaining messages again.
    public mutating func complete(_ id: String, payload: Data) throws -> (ServerMessage, [Data])? {
        guard waiting[id] != nil else { return nil } // Canceled or disconnected.
        let message = try JSONDecoder().decode(ServerMessage.self, from: payload)
        switch message {
        case .reading, .response:
            guard message.requestID == id else {
                throw RegistryQueryError("Server bulk message answered a different request")
            }
        default: throw RegistryQueryError("Server bulk ticket did not contain an answer")
        }
        return (message, discard(id))
    }

    @discardableResult
    public mutating func discard(_ id: String) -> [Data] {
        let lines = waiting.removeValue(forKey: id) ?? []
        count -= lines.count
        bytes -= lines.reduce(0) { $0 + $1.count }
        return lines
    }
}

private extension ServerMessage {
    var requestID: String? {
        switch self {
        case .response(let id, _, _), .reading(let id, _, _), .bulk(let id, _, _), .ended(let id): id
        case .error(let id, _, _, _): id
        case .welcome, .status, .admitted, .bridged: nil
        }
    }
}
