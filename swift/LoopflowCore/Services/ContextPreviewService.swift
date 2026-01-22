// Service for assembling context preview by invoking `lf -c`.

import Foundation
import AppKit

public struct ContextPreviewService: @unchecked Sendable {
    public init() {}

    public func assemblePreview(_ options: ContextOptions) async -> ContextPreview {
        let cmdArgs = buildCommandArgs(options)

        do {
            let output = try await run(cmdArgs, in: options.repoURL)
            return parseOutput(output, options: options)
        } catch {
            return .empty
        }
    }

    public func copyAssembledContext(_ options: ContextOptions) async -> String? {
        let cmdArgs = buildCommandArgs(options)

        do {
            _ = try await run(cmdArgs, in: options.repoURL)
            return NSPasteboard.general.string(forType: .string)
        } catch {
            return nil
        }
    }

    // MARK: - Private

    private func buildCommandArgs(_ options: ContextOptions) -> [String] {
        var cmdArgs = ["lf"]

        if let p = options.prompt, !p.isEmpty {
            cmdArgs.append(p)
        } else {
            cmdArgs.append(":")
        }

        if !options.args.isEmpty {
            cmdArgs.append(options.args)
        }

        let repoPath = options.repoURL.path()
        for url in options.contextFolders {
            let relativePath = url.path().replacingOccurrences(of: repoPath + "/", with: "")
            cmdArgs.append("-x")
            cmdArgs.append(relativePath)
        }

        for url in options.attachedFiles {
            let filePath = url.path(percentEncoded: false)
            if filePath.hasPrefix(repoPath) {
                let relativePath = String(filePath.dropFirst(repoPath.count))
                cmdArgs.append("-x")
                cmdArgs.append(relativePath.hasPrefix("/") ? String(relativePath.dropFirst()) : relativePath)
            } else {
                cmdArgs.append("-x")
                cmdArgs.append(filePath)
            }
        }

        if options.includeDiff { cmdArgs.append("--diff") }
        if !options.includeDiffFiles { cmdArgs.append("--no-diff-files") }
        if !options.includeDocs { cmdArgs.append("--no-docs") }
        if options.includePaste { cmdArgs.append("--paste") }
        if !options.includeSummaries { cmdArgs.append("--no-summaries") }

        cmdArgs.append("-c")
        return cmdArgs
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

    private func parseOutput(_ output: String, options: ContextOptions) -> ContextPreview {
        // Parse output like:
        // Tokens: 54,963
        //
        // files          40,584 ██████████████
        //   Concerto      19,241 ███████
        //     ...
        // docs           13,732 ████
        //   ux-research.md  4,066 █
        //   ...

        var sections: [ContextSection] = []

        // Find section lines (lines with category at indent level 0)
        let lines = output.components(separatedBy: "\n")

        var currentCategory: String?
        var currentItems: [ContextItem] = []

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
                    let isEnabled = isSectionEnabled(kind, options: options)
                    sections.append(ContextSection(
                        kind: kind,
                        items: currentItems,
                        isEnabled: isEnabled
                    ))
                }

                // Parse new category
                currentCategory = parseTokenLine(trimmed).name
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
            let isEnabled = isSectionEnabled(kind, options: options)
            sections.append(ContextSection(
                kind: kind,
                items: currentItems,
                isEnabled: isEnabled
            ))
        }

        // Add attached files section if there are any
        if !options.attachedFiles.isEmpty {
            let attachedItems = options.attachedFiles.map { url in
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

    private func isSectionEnabled(_ kind: ContextKind, options: ContextOptions) -> Bool {
        switch kind {
        case .docs: return options.includeDocs
        case .files: return options.includeDiffFiles
        case .diff: return options.includeDiff
        case .clipboard: return options.includePaste
        case .attached: return true
        }
    }
}
