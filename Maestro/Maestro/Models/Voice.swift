// Voice model for reusable personas.
// Reads from .lf/voices/*.md in a repository.

import Foundation

struct Voice: Identifiable, Hashable {
    let id: String       // filename without .md
    let name: String
    let content: String
    let path: URL

    var displayName: String {
        name.replacingOccurrences(of: "-", with: " ").capitalized
    }
}
