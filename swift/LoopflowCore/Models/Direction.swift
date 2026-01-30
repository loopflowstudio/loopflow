// Direction model for reusable judgment and intent.
// Reads from .lf/directions/*.md in a repository.

import Foundation

public struct Direction: Sendable, Identifiable, Hashable {
    public let id: String       // filename without .md
    public let name: String
    public let content: String
    public let path: URL

    public init(id: String, name: String, content: String, path: URL) {
        self.id = id
        self.name = name
        self.content = content
        self.path = path
    }

    public var displayName: String {
        name.replacingOccurrences(of: "-", with: " ").capitalized
    }
}
