import Foundation

/// Reads the one piece of wave planning state that remains in the repository.
/// Projects and KRs come from `RegistryQuery`'s SQLite-backed PM snapshot.
public enum WavePlanParser {
    public static func objective(repoRoot: URL, waveName: String) -> String? {
        let normalizedName = waveName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedName.isEmpty else { return nil }

        let goalURL = repoRoot
            .appendingPathComponent("wave", isDirectory: true)
            .appendingPathComponent(normalizedName, isDirectory: true)
            .appendingPathComponent("GOAL.md", isDirectory: false)
        guard let text = try? String(contentsOf: goalURL, encoding: .utf8) else {
            return nil
        }

        let lines = stripFrontmatter(from: text.components(separatedBy: .newlines))
        var buffer: [String] = []
        var isObjective = false
        for line in lines {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed == "## Objective" {
                isObjective = true
                continue
            }
            if isObjective, trimmed.hasPrefix("## ") {
                break
            }
            if isObjective {
                buffer.append(line)
            }
        }

        let objective = buffer
            .joined(separator: "\n")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return objective.isEmpty ? nil : objective
    }

    private static func stripFrontmatter(from lines: [String]) -> [String] {
        guard lines.first?.trimmingCharacters(in: .whitespaces) == "---" else {
            return lines
        }
        for index in lines.indices.dropFirst()
            where lines[index].trimmingCharacters(in: .whitespaces) == "---" {
            return Array(lines.dropFirst(index + 1))
        }
        return lines
    }
}
