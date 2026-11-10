import Foundation

/// The shared creation receipt returned by `creation.get`.
public struct CreationLookup: Codable, Sendable {
    public enum Disposition: String, Codable, Sendable { case complete, `continue`, inspect }
    public let disposition: Disposition
    public let explanation: String
    public let run: Bool
    public let artifactKey: String?
    public let launch: JSONValue?
    public let createdAt: Int64
    public let updatedAt: Int64
    public let requestId: String
    public let kind: String
    public let milestone: String
    public let result: JSONValue?
    public let taskId: String?
    public let sessionId: String?
    enum CodingKeys: String, CodingKey {
        case kind, milestone, result, disposition, explanation, run, launch
        case artifactKey = "artifact_key", createdAt = "created_at", updatedAt = "updated_at"
        case requestId = "request_id", taskId = "task_id", sessionId = "session_id"
    }

    public func encode(to encoder: Encoder) throws {
        var fields = encoder.container(keyedBy: CodingKeys.self)
        try fields.encode(requestId, forKey: .requestId)
        try fields.encode(kind, forKey: .kind)
        try fields.encode(milestone, forKey: .milestone)
        try fields.encode(disposition, forKey: .disposition)
        try fields.encode(explanation, forKey: .explanation)
        try fields.encode(run, forKey: .run)
        try fields.encode(taskId, forKey: .taskId)
        try fields.encode(sessionId, forKey: .sessionId)
        try fields.encode(artifactKey, forKey: .artifactKey)
        try fields.encode(result, forKey: .result)
        try fields.encode(launch, forKey: .launch)
        try fields.encode(createdAt, forKey: .createdAt)
        try fields.encode(updatedAt, forKey: .updatedAt)
    }
}

