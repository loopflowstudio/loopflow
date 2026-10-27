import Foundation

/// One bounded, ordered window from `lf history`.
public struct WorkActivitySnapshot: Decodable, Sendable, Hashable {
    public let generatedAt: Int64
    public let since: Int64
    public let limit: Int
    public let truncated: Bool
    public let items: [WorkActivityEntry]

    enum CodingKeys: String, CodingKey {
        case since, limit, truncated, items
        case generatedAt = "generated_at"
    }
}

public struct WorkActivityEntry: Decodable, Sendable, Hashable, Identifiable {
    public let id: String
    public let recordedAt: Int64
    public let summary: String
    public let work: WorkReference
    public let subject: String
    public let fact: WorkActivityFact

    enum CodingKeys: String, CodingKey {
        case id, summary, work, subject, fact
        case recordedAt = "recorded_at"
    }
}

/// What a Session did. Mirrors Rust `WorkActivityFact`.
public enum WorkActivityFact: Decodable, Sendable, Hashable {
    case inputCaptured(sessionId: String, captured: Int)
    case inputCompletionRecorded(sessionId: String, captured: Int, status: String)
    case providerHistoryRecorded(sessionId: String, captured: Int?, reference: ProviderHistoryReference, lfProcessId: String?, status: String?)

    private enum CodingKeys: String, CodingKey {
        case kind, status
        case sessionId = "session_id"
        case captured
        case reference
        case lfProcessId = "lf_process_id"
    }

    private enum Kind: String, Decodable {
        case inputCaptured = "input_captured"
        case inputCompletionRecorded = "input_completion_recorded"
        case providerHistoryRecorded = "provider_history_recorded"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(Kind.self, forKey: .kind) {
        case .inputCaptured:
            self = .inputCaptured(
                sessionId: try container.decode(String.self, forKey: .sessionId),
                captured: try container.decode(Int.self, forKey: .captured)
            )
        case .inputCompletionRecorded:
            self = .inputCompletionRecorded(
                sessionId: try container.decode(String.self, forKey: .sessionId),
                captured: try container.decode(Int.self, forKey: .captured),
                status: try container.decode(String.self, forKey: .status)
            )
        case .providerHistoryRecorded:
            self = .providerHistoryRecorded(sessionId: try container.decode(String.self, forKey: .sessionId),
                captured: try container.decodeIfPresent(Int.self, forKey: .captured),
                reference: try container.decode(ProviderHistoryReference.self, forKey: .reference),
                lfProcessId: try container.decodeIfPresent(String.self, forKey: .lfProcessId),
                status: try container.decodeIfPresent(String.self, forKey: .status))
        }
    }
}
