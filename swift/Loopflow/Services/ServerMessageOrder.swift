import Foundation

/// A ticket occupies its position in one request's stream while other IDs keep
/// flowing. Callers serialize delivery and completion with their reader.
public struct ServerMessageOrder {
    private var waiting: [String: [(ServerMessage, Int)]] = [:]
    private var bytes = 0
    private var count = 0
    private var eventSequence: UInt64 = 0

    public init() {}

    public mutating func receive(_ line: Data) throws -> ServerMessage? {
        let message = try ServerMessage(line: line)
        if case .event(_, let sequence, _) = message {
            let expected = eventSequence.addingReportingOverflow(1)
            eventSequence = sequence
            guard !expected.overflow, sequence == expected.partialValue else { throw ServerEventSequenceGap() }
        }
        return try route(message, bytes: line.count)
    }

    /// Queued messages have already passed the connection-wide sequence check.
    public mutating func resume(_ message: ServerMessage, bytes size: Int) throws -> ServerMessage? {
        try route(message, bytes: size)
    }

    private mutating func route(_ message: ServerMessage, bytes size: Int) throws -> ServerMessage? {
        guard let id = message.requestID else { return message }
        if waiting[id] != nil {
            guard count < 64, bytes + size <= ServerProtocol.maxBulk else {
                throw RegistryQueryError("Server updates fell behind a large message; reconnect for current Work")
            }
            waiting[id]?.append((message, size))
            count += 1
            bytes += size
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

    /// Deliver the payload first, then feed queued messages through `resume`:
    /// a second ticket may defer the remaining messages again.
    public mutating func complete(_ id: String, payload: Data) throws -> (ServerMessage, [(ServerMessage, Int)])? {
        guard waiting[id] != nil else { return nil } // Canceled or disconnected.
        let message = try ServerMessage(line: payload)
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
    public mutating func discard(_ id: String) -> [(ServerMessage, Int)] {
        let lines = waiting.removeValue(forKey: id) ?? []
        count -= lines.count
        bytes -= lines.reduce(0) { $0 + $1.1 }
        return lines
    }
}

private extension ServerMessage {
    var requestID: String? {
        switch self {
        case .response(let id, _, _), .reading(let id, _, _), .bulk(let id, _, _), .ended(let id): id
        case .event(let id, _, _): id
        case .error(let id, _, _, _): id
        case .welcome, .status, .admitted, .bridged: nil
        }
    }
}

/// A missing occurrence requires a new event baseline; ordinary reads remain useful.
public struct ServerEventSequenceGap: Error, Sendable {
    public init() {}
}
