import Foundation

/// One planning Task's complete Linear comment thread (`lf pm task comments --json`).
/// A partial provider read is an error, never a shorter thread, so the count is
/// `comments.count`. Optional fields must be present as `null`.
public struct TaskComments: Decodable, Sendable, Hashable {
    public let identifier: String
    public let comments: [TaskComment]
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
