// Goal model for shaping agent judgment.
// Reads from .lf/goals/*.md in a repository.

import Foundation

public struct Goal: Sendable, Identifiable, Hashable {
    public let id: String       // filename without .md
    public let name: String
    public let content: String
    public let path: URL

    public var displayName: String {
        name.replacingOccurrences(of: "-", with: " ").capitalized
    }
}
