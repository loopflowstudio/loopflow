import Foundation
import Loopflow

/// Consumer of transport/wire.rs. A new attachment always asks for a fresh
/// snapshot; byte replay and input replay are deliberately separate concerns.
struct OwnerStream {
    static let maxPayload = 4 * 1024 * 1024
    private var frames = Data()
    private var scanner: OwnerEventScanner?
    private var pendingScreen: Data?
    private var baseline: UInt64 = 0
    private var sequence: UInt64 = 0
    private var viewer: String?
    private(set) var ready = false
    private(set) var inputControl = false
    private(set) var geometryControl = false
    private(set) var ended = false
    private var awaiting: UInt64?
    var awaitingAttachment: Bool { awaiting != nil }

    mutating func expectAttachment(_ request: UInt64) {
        awaiting = request
    }

    mutating func receive(_ bytes: Data) throws -> [MobileTerminalEvent] {
        guard !ended else { return [] }
        frames.append(bytes)
        var consumed = 0
        var events: [MobileTerminalEvent] = []
        while frames.count - consumed >= 5 {
            let start = frames.startIndex + consumed
            let tag = frames[start]
            guard (1...4).contains(tag) else { throw RegistryQueryError("Unsupported Session frame") }
            let length = frames[(start + 1)..<(start + 5)].reduce(0) { ($0 << 8) | Int($1) }
            guard length <= Self.maxPayload else { throw RegistryQueryError("Session frame exceeds its limit") }
            guard frames.count - consumed >= length + 5 else { break }
            let body = Data(frames[(start + 5)..<(start + 5 + length)])
            consumed += 5 + length
            switch tag {
            case 1: try reply(body, events: &events)
            case 2:
                guard viewer != nil, pendingScreen != nil else {
                    throw RegistryQueryError("Unexpected Session snapshot")
                }
                try appendScreen(body)
            default:
                guard scanner != nil else { throw RegistryQueryError("Session output arrived before attachment") }
                for piece in scanner!.scan(body) {
                    switch piece {
                    case .bytes(let data):
                        if pendingScreen != nil { try appendScreen(data) }
                        else { events.append(.output(data)) }
                    case .event(let event): try apply(event, events: &events)
                    }
                }
            }
            if ended { break }
        }
        frames.removeFirst(consumed)
        return events
    }

    private mutating func appendScreen(_ bytes: Data) throws {
        guard (pendingScreen?.count ?? 0) + bytes.count <= Self.maxPayload else {
            throw RegistryQueryError("Session snapshot exceeds its limit")
        }
        pendingScreen?.append(bytes)
    }

    private mutating func reply(_ bytes: Data, events: inout [MobileTerminalEvent]) throws {
        let reply = try JSONDecoder().decode(OwnerReply.self, from: bytes)
        guard reply.answers == awaiting else { throw RegistryQueryError("Unexpected Session reply") }
        awaiting = nil
        switch reply.value {
        case .attached(let attachment):
            guard attachment.terminal else { throw SessionUnavailable("This Session does not have an interactive terminal") }
            viewer = attachment.viewer
            scanner = OwnerEventScanner(nonce: attachment.nonce)
            baseline = reply.sequence
            sequence = reply.sequence
            pendingScreen = Data()
            ready = false
            inputControl = attachment.token != nil
            geometryControl = attachment.owner == attachment.viewer
        case .busy:
            events.append(.control(input: inputControl, geometry: geometryControl))
        case .reconnectRequired:
            throw RegistryQueryError("The Session needs a fresh attachment")
        case .accepted, .controlled, .nativeEnded:
            throw RegistryQueryError("Unexpected Session attachment response")
        }
    }

    private mutating func apply(_ event: OwnerEvent, events: inout [MobileTerminalEvent]) throws {
        // Replies can precede already-queued events. A fresh attachment's
        // snapshot supersedes those events; later gaps are never ignored.
        if event.sequence <= baseline { return }
        guard sequence < UInt64.max, event.sequence == sequence + 1 else {
            throw RegistryQueryError("Session events were interrupted; reconnect for a fresh screen")
        }
        sequence = event.sequence
        switch event.value {
        case .resize(let columns, let rows):
            guard columns > 0, rows > 0 else { throw RegistryQueryError("Invalid Session geometry") }
            events.append(.geometry(columns: Int(columns), rows: Int(rows)))
            if let screen = pendingScreen {
                events.append(.snapshot(screen))
                pendingScreen = nil
                ready = true
                events.append(.control(input: inputControl, geometry: geometryControl))
            }
        case .owner(let id, _, let held):
            guard !held || id == viewer else { throw RegistryQueryError("Session geometry belongs to another viewer") }
            geometryControl = held
            if ready { events.append(.control(input: inputControl, geometry: held)) }
        case .control(let held):
            inputControl = held
            if ready { events.append(.control(input: held, geometry: geometryControl)) }
        case .childExited:
            ended = true
            inputControl = false
            geometryControl = false
            events.append(.ended)
        case .programStatus(let status):
            events.append(.programStatus(status))
        case .detached:
            ended = true
            inputControl = false
            geometryControl = false
            events.append(.disconnected("This Session attachment was closed"))
        case .theme, .hint, .returnToSession:
            // These carry presentation hints, never commands for the phone to run.
            break
        }
    }

    static func frame(tag: UInt8, body: Data) throws -> Data {
        guard body.count <= maxPayload else { throw RegistryQueryError("Session input exceeds its limit") }
        var length = UInt32(body.count).bigEndian
        var result = Data([tag])
        withUnsafeBytes(of: &length) { result.append(contentsOf: $0) }
        result.append(body)
        return result
    }
}

private struct OwnerAttachment: Decodable {
    let viewer: String
    let reconnect: String
    let nonce: String
    let token: String?
    let terminal: Bool
    let graceMS: UInt64
    let owner: String?

    enum CodingKeys: String, CodingKey {
        case viewer, reconnect, nonce, token, terminal, owner
        case graceMS = "grace_ms"
    }
}

private struct OwnerReply: Decodable {
    enum Value {
        case attached(OwnerAttachment), busy, reconnectRequired, accepted, controlled(Bool), nativeEnded
    }
    let answers: UInt64
    let sequence: UInt64
    let value: Value
    enum Keys: String, CodingKey { case answers, seq, kind, held }

    init(from decoder: Decoder) throws {
        let box = try decoder.container(keyedBy: Keys.self)
        answers = try box.decode(UInt64.self, forKey: .answers)
        sequence = try box.decode(UInt64.self, forKey: .seq)
        switch try box.decode(String.self, forKey: .kind) {
        case "attached": value = .attached(try OwnerAttachment(from: decoder))
        case "busy": value = .busy
        case "reconnect_required": value = .reconnectRequired
        case "accepted": value = .accepted
        case "controlled": value = .controlled(try box.decode(Bool.self, forKey: .held))
        case "native_ended": value = .nativeEnded
        default: throw RegistryQueryError("Update Loopflow to read this Session response")
        }
    }
}

private struct OwnerEvent: Decodable {
    enum Value {
        case resize(columns: UInt16, rows: UInt16)
        case theme(JSONValue)
        case programStatus(ProgramStatusRecords)
        case owner(viewer: String?, name: String?, held: Bool)
        case control(held: Bool)
        case childExited(UInt8), detached, hint(String), returnToSession(String)
    }
    let sequence: UInt64
    let value: Value
    enum Keys: String, CodingKey { case seq, type, rows, cols, theme, viewer, name, held, code, text, detach, status }

    init(from decoder: Decoder) throws {
        let box = try decoder.container(keyedBy: Keys.self)
        sequence = try box.decode(UInt64.self, forKey: .seq)
        switch try box.decode(String.self, forKey: .type) {
        case "resize": value = .resize(columns: try box.decode(UInt16.self, forKey: .cols), rows: try box.decode(UInt16.self, forKey: .rows))
        case "theme": value = .theme(try box.decode(JSONValue.self, forKey: .theme))
        case "program_status": value = .programStatus(try box.decode(ProgramStatusRecords.self, forKey: .status))
        case "owner": value = .owner(viewer: try box.decodeIfPresent(String.self, forKey: .viewer), name: try box.decodeIfPresent(String.self, forKey: .name), held: try box.decode(Bool.self, forKey: .held))
        case "control": value = .control(held: try box.decode(Bool.self, forKey: .held))
        case "child_exited": value = .childExited(try box.decode(UInt8.self, forKey: .code))
        case "detached": value = .detached
        case "hint": value = .hint(try box.decode(String.self, forKey: .text))
        case "return": value = .returnToSession(try box.decode(String.self, forKey: .detach))
        default: throw RegistryQueryError("Update Loopflow to read this Session event")
        }
    }
}

/// Matches the owner's nonce-qualified APC scanner, including fragmented escape
/// sequences. Other nonces and malformed APCs remain ordinary terminal output.
private struct OwnerEventScanner {
    enum Piece { case bytes(Data), event(OwnerEvent) }
    private let prefix: [UInt8]
    private var held: [UInt8] = []

    init(nonce: String) { prefix = Array("\u{1b}_lf1;\(nonce);".utf8) }

    mutating func scan(_ incoming: Data) -> [Piece] {
        var data = held
        data.append(contentsOf: incoming)
        held.removeAll(keepingCapacity: true)
        var pieces: [Piece] = []
        var text: [UInt8] = []
        var index = 0
        while index < data.count {
            guard data[index] == 27 else { text.append(data[index]); index += 1; continue }
            let common = min(data.count - index, prefix.count)
            guard data[index..<(index + common)].elementsEqual(prefix.prefix(common)) else {
                text.append(27); index += 1; continue
            }
            guard common == prefix.count else { held = Array(data[index...]); break }
            let bodyStart = index + prefix.count
            var end = bodyStart
            while end + 1 < data.count && !(data[end] == 27 && data[end + 1] == 92) { end += 1 }
            if end + 1 < data.count && end + 2 - index <= OwnerStream.maxPayload {
                if let event = try? JSONDecoder().decode(OwnerEvent.self, from: Data(data[bodyStart..<end])) {
                    if !text.isEmpty { pieces.append(.bytes(Data(text))); text.removeAll(keepingCapacity: true) }
                    pieces.append(.event(event))
                } else { text.append(contentsOf: data[index..<(end + 2)]) }
                index = end + 2
            } else if data.count - index <= OwnerStream.maxPayload {
                held = Array(data[index...]); break
            } else { text.append(27); index += 1 }
        }
        if !text.isEmpty { pieces.append(.bytes(Data(text))) }
        return pieces
    }
}
