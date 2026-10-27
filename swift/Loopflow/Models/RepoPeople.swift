import Foundation

/// One owner of a repository's Tasks, from `lf repo people --json`: their Git email,
/// the claim's name, and the GitHub user the email resolves to.
public struct RepoPerson: Codable, Sendable, Equatable {
    public let email: String
    public let name: String?
    public let github: GitHubPerson?

    public init(email: String, name: String?, github: GitHubPerson?) {
        self.email = email; self.name = name; self.github = github
    }
}

public struct GitHubPerson: Codable, Sendable, Equatable, Hashable {
    public let login: String
    public let name: String?
    public let avatarUrl: URL

    public init(login: String, name: String?, avatarUrl: URL) {
        self.login = login; self.name = name; self.avatarUrl = avatarUrl
    }

    enum CodingKeys: String, CodingKey {
        case login, name
        case avatarUrl = "avatar_url"
    }
}
