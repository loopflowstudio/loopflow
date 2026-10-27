import Foundation

/// Times `lf` prints outside its Desktop shapes are RFC 3339.
extension KeyedDecodingContainer {
    func decodeRFC3339(forKey key: Key) throws -> Date {
        let text = try decode(String.self, forKey: key)
        guard let date = try? Date.ISO8601FormatStyle().parse(text) else {
            throw DecodingError.dataCorruptedError(
                forKey: key, in: self, debugDescription: "expected an RFC 3339 time, got \(text)")
        }
        return date
    }
}

extension KeyedEncodingContainer {
    mutating func encodeRFC3339(_ date: Date, forKey key: Key) throws {
        try encode(date.formatted(Date.ISO8601FormatStyle()), forKey: key)
    }
}
