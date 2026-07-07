import Foundation

public enum WavePlanParser {
    public static func parse(repoRoot: URL, waveName: String) -> WavePlan? {
        let normalizedName = waveName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedName.isEmpty else { return nil }

        let waveDirectory = repoRoot
            .appendingPathComponent("wave", isDirectory: true)
            .appendingPathComponent(normalizedName, isDirectory: true)
        let goalURL = waveDirectory.appendingPathComponent("GOAL.md", isDirectory: false)

        let objective = parseObjective(at: goalURL)
        let projects = parseProjects(in: waveDirectory.appendingPathComponent("projects", isDirectory: true))

        guard objective != nil || !projects.isEmpty else {
            return nil
        }

        return WavePlan(
            objective: objective ?? "",
            projects: projects
        )
    }

    private static func parseObjective(at url: URL) -> String? {
        guard let text = try? String(contentsOf: url, encoding: .utf8) else {
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

        return normalizedText(buffer)
    }

    private static func parseProjects(in projectsDirectory: URL) -> [WaveProject] {
        let fm = FileManager.default
        guard let files = try? fm.contentsOfDirectory(
            at: projectsDirectory,
            includingPropertiesForKeys: [.isRegularFileKey],
            options: [.skipsHiddenFiles]
        ) else {
            return []
        }

        return files
            .filter { $0.pathExtension == "md" }
            .sorted { $0.lastPathComponent.localizedCaseInsensitiveCompare($1.lastPathComponent) == .orderedAscending }
            .compactMap(parseProject)
    }

    private static func parseProject(at url: URL) -> WaveProject? {
        guard let text = try? String(contentsOf: url, encoding: .utf8) else {
            return nil
        }

        let lines = text.components(separatedBy: .newlines)
        let title = parseTitle(from: lines) ?? url.deletingPathExtension().lastPathComponent
        let summary = parseSummary(from: lines)
        let krs = parseKRs(from: lines)

        return WaveProject(
            id: url.deletingPathExtension().lastPathComponent,
            title: title,
            summary: summary,
            krs: krs
        )
    }

    private static func parseTitle(from lines: [String]) -> String? {
        for line in lines {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard trimmed.hasPrefix("# ") else { continue }
            let title = String(trimmed.dropFirst(2)).trimmingCharacters(in: .whitespacesAndNewlines)
            return title.isEmpty ? nil : title
        }
        return nil
    }

    private static func parseSummary(from lines: [String]) -> String? {
        var buffer: [String] = []
        var hasSeenTitle = false

        for line in lines {
            let trimmed = line.trimmingCharacters(in: .whitespaces)

            if !hasSeenTitle {
                if trimmed.hasPrefix("# ") {
                    hasSeenTitle = true
                }
                continue
            }

            if trimmed.hasPrefix("## ") {
                break
            }

            buffer.append(line)
        }

        return normalizedText(buffer)
    }

    private static func parseKRs(from lines: [String]) -> [String] {
        var isKRs = false
        var current: [String] = []
        var items: [String] = []

        func finishCurrent() {
            let text = current
                .joined(separator: " ")
                .trimmingCharacters(in: .whitespacesAndNewlines)
            if !text.isEmpty {
                items.append(text)
            }
            current = []
        }

        for line in lines {
            let trimmed = line.trimmingCharacters(in: .whitespaces)

            if trimmed == "## KRs" {
                isKRs = true
                continue
            }

            if isKRs, trimmed.hasPrefix("## ") {
                finishCurrent()
                break
            }

            guard isKRs else { continue }

            if trimmed.hasPrefix("- ") {
                finishCurrent()
                current = [String(trimmed.dropFirst(2))]
            } else if !trimmed.isEmpty {
                current.append(trimmed)
            }
        }

        finishCurrent()
        return items
    }

    private static func stripFrontmatter(from lines: [String]) -> [String] {
        guard lines.first?.trimmingCharacters(in: .whitespaces) == "---" else {
            return lines
        }

        for index in lines.indices.dropFirst() where lines[index].trimmingCharacters(in: .whitespaces) == "---" {
            return Array(lines.dropFirst(index + 1))
        }

        return lines
    }

    private static func normalizedText(_ lines: [String]) -> String? {
        let text = lines
            .joined(separator: "\n")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return text.isEmpty ? nil : text
    }
}
