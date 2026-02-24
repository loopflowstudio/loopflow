import Foundation

public enum WaveContentParser {
    public static func parse(repoRoot: URL, waveName: String) -> WaveContent? {
        let normalizedName = waveName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedName.isEmpty else { return nil }

        let waveDirectory = repoRoot
            .appendingPathComponent("wave", isDirectory: true)
            .appendingPathComponent(normalizedName, isDirectory: true)

        let readmeURL = waveDirectory.appendingPathComponent("README.md", isDirectory: false)
        let readmeSections = parseReadmeSections(at: readmeURL)
        let roadmapItems = parseRoadmapItems(in: waveDirectory)

        if readmeSections.vision == nil,
           readmeSections.goals == nil,
           readmeSections.risks == nil,
           readmeSections.metrics == nil,
           roadmapItems.isEmpty {
            return nil
        }

        return WaveContent(
            vision: readmeSections.vision,
            goals: readmeSections.goals,
            risks: readmeSections.risks,
            metrics: readmeSections.metrics,
            roadmapItems: roadmapItems
        )
    }

    private struct ReadmeSections {
        var vision: String?
        var goals: String?
        var risks: String?
        var metrics: String?
    }

    private enum ReadmeSection {
        case vision
        case goals
        case risks
        case metrics
    }

    private static func parseReadmeSections(at url: URL) -> ReadmeSections {
        guard let text = try? String(contentsOf: url, encoding: .utf8) else {
            return ReadmeSections()
        }

        var sectionBuffers: [ReadmeSection: [String]] = [:]
        var currentSection: ReadmeSection?

        for line in text.components(separatedBy: .newlines) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)

            if let section = section(forHeader: trimmed) {
                currentSection = section
                continue
            }

            if trimmed.hasPrefix("## ") {
                currentSection = nil
                continue
            }

            if let currentSection {
                sectionBuffers[currentSection, default: []].append(line)
            }
        }

        return ReadmeSections(
            vision: normalizedSectionText(sectionBuffers[.vision]),
            goals: normalizedSectionText(sectionBuffers[.goals]),
            risks: normalizedSectionText(sectionBuffers[.risks]),
            metrics: normalizedSectionText(sectionBuffers[.metrics])
        )
    }

    private static func section(forHeader header: String) -> ReadmeSection? {
        switch header {
        case "## Vision":
            return .vision
        case "## Goals":
            return .goals
        case "## Risks":
            return .risks
        case "## Metrics":
            return .metrics
        default:
            return nil
        }
    }

    private static func normalizedSectionText(_ lines: [String]?) -> String? {
        guard let lines else { return nil }
        let text = lines
            .joined(separator: "\n")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return text.isEmpty ? nil : text
    }

    private static func parseRoadmapItems(in waveDirectory: URL) -> [RoadmapItem] {
        let fileManager = FileManager.default

        guard let files = try? fileManager.contentsOfDirectory(
            at: waveDirectory,
            includingPropertiesForKeys: [.isRegularFileKey],
            options: [.skipsHiddenFiles]
        ) else {
            return []
        }

        let roadmapFiles = files.filter { file in
            let name = file.lastPathComponent
            return name.range(of: "^[0-9]{2}-.+\\.md$", options: .regularExpression) != nil
        }

        return roadmapFiles.compactMap(parseRoadmapItem).sorted { lhs, rhs in
            if lhs.number != rhs.number {
                return lhs.number < rhs.number
            }
            return lhs.id < rhs.id
        }
    }

    private static func parseRoadmapItem(_ fileURL: URL) -> RoadmapItem? {
        let stem = fileURL.deletingPathExtension().lastPathComponent
        guard let number = Int(stem.prefix(2)) else {
            return nil
        }

        guard let text = try? String(contentsOf: fileURL, encoding: .utf8) else {
            return RoadmapItem(id: stem, number: number, title: stem, isShipped: false)
        }

        let lines = text.components(separatedBy: .newlines)
        let title = firstHeading(in: lines) ?? stem
        let isShipped = lines.contains {
            $0.trimmingCharacters(in: .whitespaces) == "## Shipped"
        }

        return RoadmapItem(id: stem, number: number, title: title, isShipped: isShipped)
    }

    private static func firstHeading(in lines: [String]) -> String? {
        for line in lines {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("# ") {
                let title = String(trimmed.dropFirst(2)).trimmingCharacters(in: .whitespaces)
                if !title.isEmpty {
                    return title
                }
            }
        }
        return nil
    }
}
