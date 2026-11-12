import Foundation

/// A JSON value's bytes as the server sent them. Whoever knows the value's type
/// decodes it, once; nothing in between parses or re-encodes it.
public struct RawJSON: Sendable, Equatable {
    public let bytes: Data

    public init(_ bytes: Data) { self.bytes = bytes }

    public func decode<T: Decodable>(_ type: T.Type = T.self) throws -> T {
        try JSONDecoder().decode(type, from: bytes)
    }

    public var isNull: Bool { bytes.elementsEqual("null".utf8) }

    /// What `lf <read>` prints for this value: bare text for a string, JSON otherwise.
    public var printed: String { string ?? String(decoding: bytes, as: UTF8.self) }

    /// The text of a JSON string; nil for any other value.
    public var string: String? {
        guard bytes.first == UInt8(ascii: "\""), bytes.last == UInt8(ascii: "\""), bytes.count >= 2 else { return nil }
        // serde escapes only quotes, backslashes and control characters.
        if !bytes.contains(UInt8(ascii: "\\")) {
            return String(decoding: bytes.dropFirst().dropLast(), as: UTF8.self)
        }
        return try? decode(String.self)
    }

    /// The fields of a JSON object, each value's bytes left as they are. One pass
    /// over the bytes; nested values are skipped, not parsed.
    public func fields() throws -> [String: RawJSON] {
        try bytes.withUnsafeBytes { buffer in
            var scanner = Scanner(bytes: buffer.bindMemory(to: UInt8.self))
            var fields: [String: RawJSON] = [:]
            try scanner.expect(UInt8(ascii: "{"))
            if try scanner.consume(UInt8(ascii: "}")) { return fields }
            while true {
                scanner.skipSpace()
                let key = try scanner.string()
                try scanner.expect(UInt8(ascii: ":"))
                scanner.skipSpace()
                let value = try scanner.value()
                let start = bytes.startIndex
                fields[String(decoding: buffer[key], as: UTF8.self)] = RawJSON(bytes[(start + value.lowerBound)..<(start + value.upperBound)])
                if try scanner.consume(UInt8(ascii: "}")) { return fields }
                try scanner.expect(UInt8(ascii: ","))
            }
        }
    }

    private struct Scanner {
        let bytes: UnsafeBufferPointer<UInt8>
        var index = 0

        mutating func skipSpace() {
            while index < bytes.count, [0x20, 0x0A, 0x0D, 0x09].contains(bytes[index]) { index += 1 }
        }

        mutating func expect(_ byte: UInt8) throws {
            skipSpace()
            guard index < bytes.count, bytes[index] == byte else { throw Self.malformed }
            index += 1
        }

        mutating func consume(_ byte: UInt8) throws -> Bool {
            skipSpace()
            guard index < bytes.count else { throw Self.malformed }
            guard bytes[index] == byte else { return false }
            index += 1
            return true
        }

        /// A string's contents, between its quotes, still escaped.
        mutating func string() throws -> Range<Int> {
            guard index < bytes.count, bytes[index] == UInt8(ascii: "\"") else { throw Self.malformed }
            index += 1
            let start = index
            while index < bytes.count {
                switch bytes[index] {
                case UInt8(ascii: "\\"): index += 2
                case UInt8(ascii: "\""):
                    index += 1
                    return start..<(index - 1)
                default: index += 1
                }
            }
            throw Self.malformed
        }

        /// One whole value, quotes and brackets included.
        mutating func value() throws -> Range<Int> {
            let start = index
            guard index < bytes.count else { throw Self.malformed }
            switch bytes[index] {
            case UInt8(ascii: "\""): _ = try string()
            case UInt8(ascii: "{"), UInt8(ascii: "["):
                var depth = 0
                scan: while index < bytes.count {
                    switch bytes[index] {
                    case UInt8(ascii: "\""): _ = try string(); continue scan
                    case UInt8(ascii: "{"), UInt8(ascii: "["): depth += 1
                    case UInt8(ascii: "}"), UInt8(ascii: "]"):
                        depth -= 1
                        if depth == 0 { index += 1; break scan }
                    default: break
                    }
                    index += 1
                }
                guard depth == 0 else { throw Self.malformed }
            default:
                while index < bytes.count,
                      ![UInt8(ascii: ","), UInt8(ascii: "}"), UInt8(ascii: "]"), 0x20, 0x0A, 0x0D, 0x09].contains(bytes[index]) {
                    index += 1
                }
            }
            guard index > start else { throw Self.malformed }
            return start..<index
        }

        static let malformed = RegistryQueryError("The server sent malformed JSON")
    }
}
