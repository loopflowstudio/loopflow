import Foundation

public struct TaskLocationObservation: Codable, Equatable, Sendable {
    public let request: String
    public let repositoryID: String
    public let taskID: String
    public let machineID: String
    public let observedAt: Int64
    public let location: TaskLocation

    enum CodingKeys: String, CodingKey {
        case request, location
        case repositoryID = "repository_id", taskID = "task_id"
        case machineID = "machine_id", observedAt = "observed_at"
    }
}

public enum TaskLocation: Codable, Equatable, Sendable {
    case recorded(taskID: String, checkout: String?)
    case unrecorded
    case unavailable(reason: String)

    private enum CodingKeys: String, CodingKey {
        case state, checkout, reason
        case taskID = "task_id"
    }
    private enum State: String, Codable { case recorded, unrecorded, unavailable }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(State.self, forKey: .state) {
        case .recorded:
            self = .recorded(taskID: try values.decode(String.self, forKey: .taskID),
                             checkout: try values.decodeIfPresent(String.self, forKey: .checkout))
        case .unrecorded: self = .unrecorded
        case .unavailable: self = .unavailable(reason: try values.decode(String.self, forKey: .reason))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .recorded(let taskID, let checkout):
            try values.encode(State.recorded, forKey: .state)
            try values.encode(taskID, forKey: .taskID)
            try values.encode(checkout, forKey: .checkout)
        case .unrecorded: try values.encode(State.unrecorded, forKey: .state)
        case .unavailable(let reason):
            try values.encode(State.unavailable, forKey: .state)
            try values.encode(reason, forKey: .reason)
        }
    }
}
