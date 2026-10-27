import Foundation

/// One bounded, ordered window from `lf history`.
public struct WorkActivitySnapshot: Decodable, Sendable, Hashable {
    public let truncated: Bool
    public let items: [WorkActivityEntry]
}

public struct WorkActivityEntry: Decodable, Sendable, Hashable, Identifiable {
    public let id: String
    public let recordedAt: Date
    public let summary: String
    public let work: WorkReference
    public let subject: String
    public let fact: WorkActivityFact

    enum CodingKeys: String, CodingKey {
        case id, summary, work, subject, fact
        case recordedAt = "recorded_at"
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(String.self, forKey: .id)
        recordedAt = try c.decodeRFC3339(forKey: .recordedAt)
        summary = try c.decode(String.self, forKey: .summary)
        work = try c.decode(WorkReference.self, forKey: .work)
        subject = try c.decode(String.self, forKey: .subject)
        fact = try c.decode(WorkActivityFact.self, forKey: .fact)
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
