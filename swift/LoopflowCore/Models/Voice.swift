// Voice model for reusable personas.
// Reads from .lf/voices/*.md in a repository.

import Foundation

public struct Voice: Sendable, Identifiable, Hashable {
    public let id: String       // filename without .md
    public let name: String
    public let content: String
    public let path: URL

    public var displayName: String {
        name.replacingOccurrences(of: "-", with: " ").capitalized
    }
}
