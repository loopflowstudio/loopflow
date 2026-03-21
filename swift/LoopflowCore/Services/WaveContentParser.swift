import Foundation

public enum WaveContentParser {
    public static func parse(repoRoot: URL, waveName: String, branch: String? = nil) -> WaveContent? {
        let normalizedName = waveName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedName.isEmpty else { return nil }

        let waveDirectory = repoRoot
            .appendingPathComponent("wave", isDirectory: true)
            .appendingPathComponent(normalizedName, isDirectory: true)

        let readmeURL = waveDirectory.appendingPathComponent("README.md", isDirectory: false)
        let readmeSections = parseReadmeSections(at: readmeURL)
        let roadmapItems = parseRoadmapItems(in: waveDirectory)
        let (scratchDoc, scratchDocPath) = parseScratchDoc(repoRoot: repoRoot, branch: branch)

        if readmeSections.vision == nil,
           readmeSections.strategy == nil,
           readmeSections.goals == nil,
           readmeSections.risks == nil,
           readmeSections.metrics == nil,
           roadmapItems.isEmpty,
           scratchDoc == nil {
            return nil
        }

        return WaveContent(
            vision: readmeSections.vision,
            strategy: readmeSections.strategy,
            goals: readmeSections.goals,
            risks: readmeSections.risks,
            metrics: readmeSections.metrics,
            roadmapItems: roadmapItems,
            scratchDoc: scratchDoc,
            scratchDocPath: scratchDocPath
        )
    }

    private struct ReadmeSections {
        var vision: String?
        var strategy: String?
        var goals: String?
        var risks: String?
        var metrics: String?
    }

    private enum ReadmeSection {
        case vision
        case strategy
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
            strategy: normalizedSectionText(sectionBuffers[.strategy]),
            goals: normalizedSectionText(sectionBuffers[.goals]),
            risks: normalizedSectionText(sectionBuffers[.risks]),
            metrics: normalizedSectionText(sectionBuffers[.metrics])
        )
    }

    private static func section(forHeader header: String) -> ReadmeSection? {
        switch header {
        case "## Vision":
            return .vision
        case "## Strategy":
            return .strategy
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

        return files
            .compactMap(parseRoadmapMetadata)
            .sorted { lhs, rhs in
                if lhs.order != rhs.order {
                    return lhs.order < rhs.order
                }
                return lhs.fileName < rhs.fileName
            }
            .compactMap(parseRoadmapItem)
    }

    private static func parseRoadmapItem(_ metadata: RoadmapMetadata) -> RoadmapItem? {
        guard let text = try? String(contentsOf: metadata.fileURL, encoding: .utf8) else {
            return RoadmapItem(
                id: metadata.stem,
                number: metadata.number,
                title: metadata.stem,
                slug: metadata.slug,
                fileName: metadata.fileName,
                priority: metadata.priority,
                isShipped: false,
                filePath: metadata.fileURL.path
            )
        }

        let lines = text.components(separatedBy: .newlines)
        let title = firstHeading(in: lines) ?? metadata.stem
        let isShipped = lines.contains {
            $0.trimmingCharacters(in: .whitespaces) == "## Shipped"
        }

        let content: String? = isShipped ? nil : extractContent(from: lines)

        return RoadmapItem(
            id: metadata.stem,
            number: metadata.number,
            title: title,
            slug: metadata.slug,
            fileName: metadata.fileName,
            priority: metadata.priority,
            isShipped: isShipped,
            content: content,
            filePath: metadata.fileURL.path
        )
    }

    private struct RoadmapMetadata {
        let fileURL: URL
        let stem: String
        let fileName: String
        let slug: String
        let number: Int
        let priority: RoadmapPriority
        let order: RoadmapOrder
    }

    private enum RoadmapOrder: Comparable {
        case bucket(RoadmapPriority)
        case legacy(Int)

        fileprivate static func < (lhs: RoadmapOrder, rhs: RoadmapOrder) -> Bool {
            lhs.sortKey < rhs.sortKey
        }

        private var sortKey: (Int, Int) {
            switch self {
            case let .bucket(priority):
                (0, priority.rawValue)
            case let .legacy(number):
                (1, number)
            }
        }
    }

    private static func parseRoadmapMetadata(for fileURL: URL) -> RoadmapMetadata? {
        guard fileURL.pathExtension.lowercased() == "md" else { return nil }
        let fileName = fileURL.lastPathComponent
        let stem = fileURL.deletingPathExtension().lastPathComponent
        let parts = stem.split(separator: "-", maxSplits: 1, omittingEmptySubsequences: false)
        guard parts.count == 2 else { return nil }

        let prefix = String(parts[0])
        let slug = String(parts[1])
        let normalizedPrefix = prefix.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !slug.isEmpty,
              let priority = RoadmapPriority.from(prefix: prefix),
              let number = Int(normalizedPrefix) else {
            return nil
        }

        return RoadmapMetadata(
            fileURL: fileURL,
            stem: stem,
            fileName: fileName,
            slug: slug,
            number: number,
            priority: priority,
            order: bucketPriority(from: prefix).map(RoadmapOrder.bucket) ?? .legacy(number)
        )
    }

    private static func bucketPriority(from prefix: String) -> RoadmapPriority? {
        switch prefix {
        case "1":
            .urgent
        case "2":
            .high
        case "3":
            .medium
        case "4":
            .low
        default:
            nil
        }
    }

    private static func extractContent(from lines: [String]) -> String? {
        // Drop lines up to and including the first `# ` heading, take up to 20 lines after.
        var foundHeading = false
        var contentLines: [String] = []

        for line in lines {
            if !foundHeading {
                if line.trimmingCharacters(in: .whitespaces).hasPrefix("# ") {
                    foundHeading = true
                }
                continue
            }
            if contentLines.count >= 20 { break }
            contentLines.append(line)
        }

        let text = contentLines.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
        return text.isEmpty ? nil : text
    }

    private static func parseScratchDoc(repoRoot: URL, branch: String?) -> (String?, String?) {
        guard let branch, !branch.isEmpty else { return (nil, nil) }
        let scratchURL = repoRoot
            .appendingPathComponent("scratch", isDirectory: true)
            .appendingPathComponent("\(branch).md", isDirectory: false)
        guard let text = try? String(contentsOf: scratchURL, encoding: .utf8) else {
            return (nil, nil)
        }
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return (nil, nil) }
        return (trimmed, scratchURL.path)
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
