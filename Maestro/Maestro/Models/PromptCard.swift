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
