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
        let (scratchDoc, scratchDocPath) = parseScratchDoc(repoRoot: repoRoot, branch: branch)

        if readmeSections.vision == nil,
           readmeSections.strategy == nil,
           readmeSections.goals == nil,
           readmeSections.risks == nil,
           readmeSections.metrics == nil,
           scratchDoc == nil {
            return nil
        }

        return WaveContent(
            vision: readmeSections.vision,
            strategy: readmeSections.strategy,
            goals: readmeSections.goals,
            risks: readmeSections.risks,
            metrics: readmeSections.metrics,
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

        let lines = text.components(separatedBy: .newlines)
        let leadingParagraph = extractLeadingParagraph(from: lines)

        var sectionBuffers: [ReadmeSection: [String]] = [:]
        var currentSection: ReadmeSection?

        for line in lines {
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

        let sectionVision = normalizedSectionText(sectionBuffers[.vision])

        return ReadmeSections(
            vision: leadingParagraph ?? sectionVision,
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

    private static func extractLeadingParagraph(from lines: [String]) -> String? {
        var index = 0

        while index < lines.count {
            let trimmed = lines[index].trimmingCharacters(in: .whitespaces)

            if trimmed.isEmpty {
                index += 1
                continue
            }

            if trimmed.hasPrefix("# ") {
                index += 1
                continue
            }

            if trimmed.hasPrefix("#") {
                if section(forHeader: trimmed) != nil {
                    return nil
                }
                index += 1
                continue
            }

            break
        }

        guard index < lines.count else { return nil }

        var paragraphLines: [String] = []
        while index < lines.count {
            let trimmed = lines[index].trimmingCharacters(in: .whitespaces)
            guard !trimmed.isEmpty, !trimmed.hasPrefix("#") else { break }
            paragraphLines.append(lines[index])
            index += 1
        }

        return normalizedSectionText(paragraphLines)
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
}
