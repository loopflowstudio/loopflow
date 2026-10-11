import Foundation

/// The saved Task thread, including pending deliveries and refresh failures.
/// Optional fields must be present as `null`.
public struct TaskComments: Decodable, Sendable, Hashable {
    public let identifier: String
    public let comments: [TaskComment]
    public let conflicts: [String: String]
    public let pendingSync: [String]
    public let refreshError: String?

    enum CodingKeys: String, CodingKey {
        case identifier, comments, conflicts
        case pendingSync = "pending_sync"
        case refreshError = "refresh_error"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        identifier = try container.decode(String.self, forKey: .identifier)
        comments = try container.decode([TaskComment].self, forKey: .comments)
        conflicts = try container.decode([String: String].self, forKey: .conflicts)
        pendingSync = try container.decode([String].self, forKey: .pendingSync)
        refreshError = try container.decode(String?.self, forKey: .refreshError)
    }
}

public struct TaskComment: Decodable, Sendable, Hashable, Identifiable {
    public let id: String
    public let body: String
    public let author: TaskCommentAuthor
    public let createdAt: String?

    enum CodingKeys: String, CodingKey {
        case id, body, author
        case createdAt = "created_at"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        body = try container.decode(String.self, forKey: .body)
        author = try container.decode(TaskCommentAuthor.self, forKey: .author)
        createdAt = try container.decode(String?.self, forKey: .createdAt)
    }
}

/// A person has a provider user; an integration has none.
public enum TaskCommentAuthor: Decodable, Sendable, Hashable {
    case person(name: String?)
    case integration

    private enum CodingKeys: String, CodingKey { case kind, name }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "person": self = .person(name: try container.decode(String?.self, forKey: .name))
        case "integration": self = .integration
        case let kind:
            throw DecodingError.dataCorruptedError(
                forKey: .kind, in: container, debugDescription: "unknown comment author '\(kind)'"
            )
        }
    }
}
