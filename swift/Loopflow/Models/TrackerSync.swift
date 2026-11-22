import Foundation

/// A Task's or Project's slice of `lf repo tracker`'s actions: what its tracker
/// has yet to confirm (open), and each value the tracker won (lost) until it
/// is answered.
public struct TrackerSync: Decodable, Sendable, Hashable {
    public let actions: [TrackerActionStatus]
}

public enum TrackerActionPhase: String, Decodable, Sendable, Hashable {
    case queued, sent, confirmed, conflicted, retired
}

/// How a person answers a lost value: `lf task sync --keep-theirs` or `--use-mine`.
public enum LostAnswer: String, Encodable, Sendable, Hashable {
    case keepTheirs = "keep_theirs"
    case useMine = "use_mine"
}

/// What a tracker action addresses.
public enum TrackerActionOwnerKind: String, Decodable, Sendable, Hashable {
    case task, project, wave
}

/// The tracker a queued planning change goes to.
public enum TrackerKind: String, Decodable, Sendable, Hashable {
    case linear, asana

    public var displayName: String {
        switch self {
        case .linear: "Linear"
        case .asana: "Asana"
        }
    }
}

public struct TrackerActionStatus: Decodable, Sendable, Hashable, Identifiable {
    public let id: String
    public let seq: Int?
    public let ownerKind: TrackerActionOwnerKind
    public let ownerId: String
    public let kind: String
    public let change: String
    public let subject: String?
    public let phase: TrackerActionPhase
    public let tracker: TrackerKind
    public let error: String?
    public let localValue: JSONValue
    public let trackerValue: JSONValue?

    enum CodingKeys: String, CodingKey {
        case id, seq, kind, change, subject, phase, tracker, error
        case ownerKind = "owner_kind"
        case ownerId = "owner_id"
        case localValue = "local_value"
        case trackerValue = "tracker_value"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        seq = try container.decode(Int?.self, forKey: .seq)
        ownerKind = try container.decode(TrackerActionOwnerKind.self, forKey: .ownerKind)
        ownerId = try container.decode(String.self, forKey: .ownerId)
        kind = try container.decode(String.self, forKey: .kind)
        change = try container.decode(String.self, forKey: .change)
        subject = try container.decode(String?.self, forKey: .subject)
        phase = try container.decode(TrackerActionPhase.self, forKey: .phase)
        tracker = try container.decode(TrackerKind.self, forKey: .tracker)
        error = try container.decode(String?.self, forKey: .error)
        localValue = try container.decode(JSONValue.self, forKey: .localValue)
        trackerValue = try container.decode(JSONValue?.self, forKey: .trackerValue)
    }

    /// The line `lf task sync` prints: `title · queued`,
    /// `title · conflicted · lost: "A" · linear: "B"`.
    public var text: String {
        var text = "\(change) · \(phase.rawValue)"
        if phase == .sent { text += ", waiting for \(tracker.rawValue) to confirm" }
        if phase == .conflicted {
            text += " · lost: \(display(localValue)) · \(tracker.rawValue): \(display(trackerValue ?? .null))"
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
