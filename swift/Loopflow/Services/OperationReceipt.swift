import Foundation

/// A semantic stage resolves in the transaction that changes its displayed
/// facts. Supersession ends the intended effect and requires replacement coverage;
/// delivery alone cannot acknowledge a projection.
public struct OperationCommit: Codable, Equatable, Sendable {
    public enum Resolution: String, Codable, Sendable {
        case committed, definitelyRefused = "definitely_refused", superseded
    }
    public let resolution: Resolution
    public let stage: String
    public let resources: [String]
    public let fence: CommitFence
    public let result: JSONValue
    public let planningChanges: [PlanningChangeRef]

    private enum CodingKeys: String, CodingKey {
        case resolution, stage, resources, fence, result
        case planningChanges = "planning_changes"
    }
}

public struct PlanningChangeRef: Codable, Equatable, Sendable {
    public enum Kind: String, Codable, Sendable { case task, project }
    public let kind: Kind
    public let owner: String
    public let id: String
}

/// Current evidence for these exact planning changes; the saved result stays historical.
public struct OperationPlanningReading: Codable, Equatable, Sendable {
    public let fence: CommitFence
    public let changes: [OperationPlanningChange]
}

public struct OperationPlanningChange: Codable, Equatable, Sendable {
    public enum State: String, Codable, Sendable {
        case pending, uncertain, delivered, superseded, conflict, unavailable
    }
    public let change: PlanningChangeRef
    public let state: State
    public let error: String?

    private enum CodingKeys: String, CodingKey { case change, state, error }

    public init(from decoder: any Decoder) throws {
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        change = try fields.decode(PlanningChangeRef.self, forKey: .change)
        state = try fields.decode(State.self, forKey: .state)
        error = try fields.decode(String?.self, forKey: .error)
    }

    public func encode(to encoder: any Encoder) throws {
        var fields = encoder.container(keyedBy: CodingKeys.self)
        try fields.encode(change, forKey: .change)
        try fields.encode(state, forKey: .state)
        try fields.encode(error, forKey: .error)
    }
}

public struct CommitFence: Codable, Equatable, Sendable {
    public let machine: String
    public let epoch: String
    public let schema: UInt32
    public let revisions: [String: Int64]

    /// Only a complete reading of this exact query can incorporate its effect.
    /// An unknown domain stays unconfirmed until this client understands it.
    public func isCovered(by reading: ReadingStamp, query: String) -> Bool {
        guard reading.complete, reading.query == query,
              machine == reading.machine, epoch == reading.epoch, schema == reading.schema,
              !revisions.isEmpty else { return false }
        return revisions.allSatisfy { domain, revision in
            let current: Int64
            switch domain {
            case "planning": current = reading.revisions.planning
            case "sessions": current = reading.revisions.sessions
            case "graphs": current = reading.revisions.graphs
            case "processes": current = reading.revisions.processes
            case "usage": current = reading.revisions.usage
            default: return false
            }
            return current >= revision
        }
    }
}

/// A dispatched command may still be running or may have lost its worker.
/// Neither that state nor an unavailable lookup authorizes a second dispatch.
public struct OperationReceipt: Codable, Equatable, Sendable {
    public enum Delivery: String, Codable, Sendable {
        case untracked, queued, dispatched, finished, uncertain
    }
    public enum Validation: String, Codable, Sendable {
        case unbound, pending, candidate, bound, conflict
    }
    public let requestID: String
    public let kind: String
    public let fingerprint: String?
    public let submittedFingerprint: String?
    public let validation: Validation
    public let delivery: Delivery
    public let revision: Int64
    public let result: JSONValue?
    public let error: String?
    public let commits: [OperationCommit]
    public let planning: OperationPlanningReading?
    public let createdAt: Int64
    public let updatedAt: Int64

    private enum CodingKeys: String, CodingKey {
        case requestID = "request_id", kind, fingerprint, validation, delivery, revision, result, error, commits
        case submittedFingerprint = "submitted_fingerprint"
        case planning
        case createdAt = "created_at", updatedAt = "updated_at"
    }

    public init(from decoder: any Decoder) throws {
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        requestID = try fields.decode(String.self, forKey: .requestID)
        kind = try fields.decode(String.self, forKey: .kind)
        fingerprint = try fields.decode(String?.self, forKey: .fingerprint)
        submittedFingerprint = try fields.decode(String?.self, forKey: .submittedFingerprint)
        validation = try fields.decode(Validation.self, forKey: .validation)
        delivery = try fields.decode(Delivery.self, forKey: .delivery)
        revision = try fields.decode(Int64.self, forKey: .revision)
        result = try fields.decode(JSONValue?.self, forKey: .result)
        error = try fields.decode(String?.self, forKey: .error)
        commits = try fields.decode([OperationCommit].self, forKey: .commits)
        planning = try fields.decode(OperationPlanningReading?.self, forKey: .planning)
        createdAt = try fields.decode(Int64.self, forKey: .createdAt)
        updatedAt = try fields.decode(Int64.self, forKey: .updatedAt)
    }

    public func encode(to encoder: any Encoder) throws {
        var fields = encoder.container(keyedBy: CodingKeys.self)
        try fields.encode(requestID, forKey: .requestID)
        try fields.encode(kind, forKey: .kind)
        try fields.encode(fingerprint, forKey: .fingerprint)
        try fields.encode(submittedFingerprint, forKey: .submittedFingerprint)
        try fields.encode(validation, forKey: .validation)
        try fields.encode(delivery, forKey: .delivery)
        try fields.encode(revision, forKey: .revision)
        try fields.encode(result, forKey: .result)
        try fields.encode(error, forKey: .error)
        try fields.encode(commits, forKey: .commits)
        try fields.encode(planning, forKey: .planning)
        try fields.encode(createdAt, forKey: .createdAt)
        try fields.encode(updatedAt, forKey: .updatedAt)
    }
}
