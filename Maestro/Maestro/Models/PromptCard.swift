// PromptCard model representing a prompt from .claude/commands/.

import Foundation

enum RunMode: String, Codable, CaseIterable {
    case auto
    case interactive
}

struct PromptCard: Identifiable, Hashable {
    var id: String { name }

    let name: String
    let content: String
    let defaultMode: RunMode

    var displayName: String {
        name.replacingOccurrences(of: "_", with: " ")
    }
}
