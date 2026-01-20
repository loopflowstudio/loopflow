// PromptCard model representing a prompt from .claude/commands/.

import Foundation

enum RunMode: String, Codable, CaseIterable {
    case auto
    case interactive
}

/// Source of a prompt (local repo or external skill library).
enum PromptSource: Hashable {
    case local                    // .claude/commands/ or .lf/
    case external(String)         // External skill source name (e.g., "superpowers")

    var isExternal: Bool {
        if case .external = self { return true }
        return false
    }

    var sourceName: String? {
        if case .external(let name) = self { return name }
        return nil
    }
}

struct PromptCard: Identifiable, Hashable {
    var id: String { name }

    let name: String
    let content: String
    let defaultMode: RunMode
    let source: PromptSource

    init(name: String, content: String, defaultMode: RunMode, source: PromptSource = .local) {
        self.name = name
        self.content = content
        self.defaultMode = defaultMode
        self.source = source
    }

    var displayName: String {
        name.replacingOccurrences(of: "_", with: " ")
    }

    /// For external skills, the prefix is part of the name (e.g., "sp:brainstorm")
    var isExternalSkill: Bool {
        source.isExternal
    }

    /// Short description extracted from the first non-frontmatter, non-header line
    var shortDescription: String? {
        let lines = content.components(separatedBy: .newlines)
        var inFrontmatter = false
        var pastFrontmatter = false

        for line in lines {
            let trimmed = line.trimmingCharacters(in: .whitespaces)

            // Skip empty lines
            if trimmed.isEmpty { continue }

            // Handle YAML frontmatter
            if trimmed == "---" {
                if !pastFrontmatter {
                    inFrontmatter = !inFrontmatter
                    if !inFrontmatter { pastFrontmatter = true }
                }
                continue
            }
            if inFrontmatter { continue }

            // Skip markdown headers
            if trimmed.hasPrefix("#") { continue }

            // Found first content line - truncate if needed
            let maxLength = 60
            if trimmed.count > maxLength {
                let endIndex = trimmed.index(trimmed.startIndex, offsetBy: maxLength)
                return String(trimmed[..<endIndex]) + "..."
            }
            return trimmed
        }
        return nil
    }
}
