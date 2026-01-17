// Service for assembling context preview by invoking `lf -c`.

import Foundation
import AppKit

struct ContextPreviewService {

    func assemblePreview(
        prompt: String?,
        args: String,
        context: [URL],
        attachedFiles: [URL],
        includeDocs: Bool,
        includeDiff: Bool,
        includeDiffFiles: Bool,
        includePaste: Bool,
        includeSummaries: Bool,
        in repoURL: URL
    ) async -> ContextPreview {
        // Build command args matching buildCommand() logic
        var cmdArgs = ["lf"]

        if let p = prompt, !p.isEmpty {
            cmdArgs.append(p)
        } else {
            cmdArgs.append(":")
        }

        if !args.isEmpty {
            cmdArgs.append(args)
        }

        // Context folders
        for url in context {
            let relativePath = url.path().replacingOccurrences(of: repoURL.path() + "/", with: "")
            cmdArgs.append("-x")
            cmdArgs.append(relativePath)
        }

        // Attached files
        for url in attachedFiles {
            let filePath = url.path(percentEncoded: false)
            if filePath.hasPrefix(repoURL.path()) {
                let relativePath = String(filePath.dropFirst(repoURL.path().count))
                cmdArgs.append("-x")
                cmdArgs.append(relativePath.hasPrefix("/") ? String(relativePath.dropFirst()) : relativePath)
            } else {
                cmdArgs.append("-x")
                cmdArgs.append(filePath)
            }
        }

        // Context flags
        if includeDiff {
            cmdArgs.append("--diff")
        }
        if !includeDiffFiles {
            cmdArgs.append("--no-diff-files")
        }
        if !includeDocs {
            cmdArgs.append("--no-docs")
        }
        if includePaste {
            cmdArgs.append("--paste")
        }
        if !includeSummaries {
            cmdArgs.append("--no-summaries")
        }

        cmdArgs.append("-c")

        do {
            let output = try await run(cmdArgs, in: repoURL)
            return parseOutput(
                output,
                includeDocs: includeDocs,
                includeDiff: includeDiff,
                includeDiffFiles: includeDiffFiles,
                includePaste: includePaste,
                attachedFiles: attachedFiles,
                repoURL: repoURL
            )
        } catch {
            return .empty
        }
    }

    func copyAssembledContext(
        prompt: String?,
        args: String,
        context: [URL],
        attachedFiles: [URL],
        includeDocs: Bool,
        includeDiff: Bool,
        includeDiffFiles: Bool,
        includePaste: Bool,
        includeSummaries: Bool,
        in repoURL: URL
    ) async -> String? {
        // The `-c` flag already copies to clipboard, so just call it
        var cmdArgs = ["lf"]

        if let p = prompt, !p.isEmpty {
            cmdArgs.append(p)
        } else {
            cmdArgs.append(":")
        }

        if !args.isEmpty {
            cmdArgs.append(args)
        }

        for url in context {
            let relativePath = url.path().replacingOccurrences(of: repoURL.path() + "/", with: "")
            cmdArgs.append("-x")
            cmdArgs.append(relativePath)
        }

        for url in attachedFiles {
            let filePath = url.path(percentEncoded: false)
            if filePath.hasPrefix(repoURL.path()) {
                let relativePath = String(filePath.dropFirst(repoURL.path().count))
                cmdArgs.append("-x")
                cmdArgs.append(relativePath.hasPrefix("/") ? String(relativePath.dropFirst()) : relativePath)
            } else {
                cmdArgs.append("-x")
                cmdArgs.append(filePath)
            }
        }

        if includeDiff {
            cmdArgs.append("--diff")
        }
        if !includeDiffFiles {
            cmdArgs.append("--no-diff-files")
        }
        if !includeDocs {
            cmdArgs.append("--no-docs")
        }
        if includePaste {
            cmdArgs.append("--paste")
        }
        if !includeSummaries {
            cmdArgs.append("--no-summaries")
        }

        cmdArgs.append("-c")

        do {
            _ = try await run(cmdArgs, in: repoURL)
            // Return clipboard contents
            return NSPasteboard.general.string(forType: .string)
        } catch {
            return nil
        }
    }

    private func run(_ args: [String], in directory: URL) async throws -> String {
        try await withCheckedThrowingContinuation { continuation in
            let process = Process()
            let pipe = Pipe()

            process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
            process.arguments = args
            process.currentDirectoryURL = directory
            process.standardOutput = pipe
            process.standardError = pipe

            do {
                try process.run()
                process.waitUntilExit()

                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                let output = String(data: data, encoding: .utf8) ?? ""
                continuation.resume(returning: output)
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }

    private func parseOutput(
        _ output: String,
        includeDocs: Bool,
        includeDiff: Bool,
        includeDiffFiles: Bool,
        includePaste: Bool,
        attachedFiles: [URL],
        repoURL: URL
    ) -> ContextPreview {
        // Parse output like:
        // Tokens: 54,963
        //
        // files          40,584 ██████████████
        //   Maestro      19,241 ███████
        //     ...
        // docs           13,732 ████
        //   ux-research.md  4,066 █
        //   ...

        var sections: [ContextSection] = []

        // Find section lines (lines with category at indent level 0)
        let lines = output.components(separatedBy: "\n")

        var currentCategory: String?
        var currentItems: [ContextItem] = []
        var currentTokens: Int = 0

        for line in lines {
            // Skip empty lines and total line
            if line.isEmpty || line.hasPrefix("Tokens:") || line.hasPrefix("Copied") {
                continue
            }

            // Check if this is a top-level category (no leading spaces)
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            let leadingSpaces = line.prefix(while: { $0 == " " }).count

            if leadingSpaces == 0 && !trimmed.isEmpty {
                // Save previous category if any
                if let cat = currentCategory {
                    let kind = mapCategoryToKind(cat)
                    let isEnabled = isSectionEnabled(kind, includeDocs: includeDocs, includeDiff: includeDiff, includeDiffFiles: includeDiffFiles, includePaste: includePaste)
                    sections.append(ContextSection(
                        kind: kind,
                        items: currentItems,
                        isEnabled: isEnabled
                    ))
                }

                // Parse new category
                let parts = parseTokenLine(trimmed)
                currentCategory = parts.name
                currentTokens = parts.tokens
                currentItems = []
            } else if leadingSpaces == 2 && currentCategory != nil {
                // Top-level item within category
                let parts = parseTokenLine(trimmed)
                let item = ContextItem(
                    name: parts.name,
                    preview: nil,  // Could load preview lazily
                    tokens: parts.tokens,
                    path: parts.name
                )
                currentItems.append(item)
            }
            // Deeper nesting is rolled up into parent item
        }

        // Save last category
        if let cat = currentCategory {
            let kind = mapCategoryToKind(cat)
            let isEnabled = isSectionEnabled(kind, includeDocs: includeDocs, includeDiff: includeDiff, includeDiffFiles: includeDiffFiles, includePaste: includePaste)
            sections.append(ContextSection(
                kind: kind,
                items: currentItems,
                isEnabled: isEnabled
            ))
        }

        // Add attached files section if there are any
        if !attachedFiles.isEmpty {
            let attachedItems = attachedFiles.map { url in
                ContextItem(
                    name: url.lastPathComponent,
                    preview: nil,
                    tokens: 0,  // Tokens counted in files section
                    path: url.path()
                )
            }
            sections.append(ContextSection(
                kind: .attached,
                items: attachedItems,
                isEnabled: true
            ))
        }

        return ContextPreview(sections: sections)
    }

    private func parseTokenLine(_ line: String) -> (name: String, tokens: Int) {
        // Parse lines like "files          40,584 ██████████████"
        // Or "README.md     1,484 ▏"
        let pattern = #"^(.+?)\s+([\d,]+)\s*[█▏]*$"#
        guard let regex = try? NSRegularExpression(pattern: pattern) else {
            return (line, 0)
        }

        let range = NSRange(line.startIndex..., in: line)
        guard let match = regex.firstMatch(in: line, range: range) else {
            return (line.trimmingCharacters(in: .whitespaces), 0)
        }

        let nameRange = Range(match.range(at: 1), in: line)!
        let tokensRange = Range(match.range(at: 2), in: line)!

        let name = String(line[nameRange]).trimmingCharacters(in: .whitespaces)
        let tokensStr = String(line[tokensRange]).replacingOccurrences(of: ",", with: "")
        let tokens = Int(tokensStr) ?? 0

        return (name, tokens)
    }

    private func mapCategoryToKind(_ category: String) -> ContextKind {
        switch category.lowercased() {
        case "docs": return .docs
        case "files": return .files
        case "diff": return .diff
        case "clipboard": return .clipboard
        case "attached": return .attached
        default: return .files
        }
    }

    private func isSectionEnabled(
        _ kind: ContextKind,
        includeDocs: Bool,
        includeDiff: Bool,
        includeDiffFiles: Bool,
        includePaste: Bool
    ) -> Bool {
        switch kind {
        case .docs: return includeDocs
        case .files: return includeDiffFiles
        case .diff: return includeDiff
        case .clipboard: return includePaste
        case .attached: return true
        }
    }
}
