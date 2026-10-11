import Foundation

public struct PlanningSyncStatus: Decodable, Sendable, Hashable {
    public let connected: Bool
    public let changes: [PlanningSyncChange]
}

public enum PlanningSyncState: String, Decodable, Sendable, Hashable {
    case pending, uncertain
    case adoptedLinear = "adopted_linear"

    public var label: String {
        switch self {
        case .pending: "Saved locally; pending Linear sync"
        case .uncertain: "Awaiting Linear confirmation"
        case .adoptedLinear: "Adopted Linear change"
        }
    }
}

public struct PlanningSyncChange: Decodable, Sendable, Hashable, Identifiable {
    public let id: String
    public let field: String
    public let state: PlanningSyncState
    public let localValue: JSONValue
    public let linearValue: JSONValue?
    public let error: String?

    enum CodingKeys: String, CodingKey {
        case id, field, state, error
        case localValue = "local_value"
        case linearValue = "linear_value"
    }

    public var text: String {
        var text = "\(state.label) · \(field)"
        if state == .adoptedLinear {
            text += " · saved: \(display(localValue)) · Linear: \(display(linearValue ?? .null))"
        }
        if let error { text += " · \(error)" }
        return text
    }

    private func display(_ value: JSONValue) -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        guard let data = try? encoder.encode(value), let text = String(data: data, encoding: .utf8) else {
            return "Value unavailable"
        }
        return text
    }
}
