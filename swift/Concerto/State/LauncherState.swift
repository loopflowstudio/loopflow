// Context assembly state for prompt launching.

import Foundation
import LoopflowCore

@MainActor
@Observable
final class LauncherState {
    var selectedPrompt: PromptCard?
    var selectedDirections: [Direction] = []
    var selectedModel: WaveModel?
    var promptArgs: String = ""

    // Context toggles
    var includeDocs: Bool = true
    var includeDiff: Bool = false
    var includeDiffFiles: Bool = true
    var includePaste: Bool = false
    var includeSummaries: Bool = true
    var includeChrome: Bool = false
    var selectedContextFolders: Set<URL> = []
    var attachedFiles: [URL] = []
    var excludedFiles: Set<String> = []

    var runMode: RunMode = .auto
    var estimatedTokens: Int = 0
    var contextPreview: ContextPreview = .empty

    // Services
    private let contextPreviewService = ContextPreviewService()

    // MARK: - Initialization from config

    func initializeFromConfig(_ config: LoopflowConfig?, repoURL: URL, directions: [Direction]) {
        includeDocs = config?.docs ?? true
        includeDiff = config?.diff ?? false
        includeDiffFiles = config?.diffFiles ?? true
        includePaste = config?.paste ?? false
        includeSummaries = config?.hasSummaries ?? false
        includeChrome = config?.chrome ?? false

        if let contextPaths = config?.context {
            if contextPaths == ["."] {
                selectedContextFolders = Set(listRootFolders(in: repoURL, exclude: config?.exclude ?? []))
            } else {
                selectedContextFolders = Set(contextPaths.map { repoURL.appendingPathComponent($0) })
            }
        } else {
            selectedContextFolders = []
        }

        if let directionNames = config?.directionNames, !directionNames.isEmpty {
            selectedDirections = directions.filter { directionNames.contains($0.name) }
        }
    }

    // MARK: - Context Operations

    func removeContextItem(_ item: ContextItem, from section: ContextSection) {
        guard let path = item.path else { return }

        if section.kind == .attached {
            attachedFiles.removeAll { $0.path() == path }
        } else {
            excludedFiles.insert(path)
        }
    }

    func buildContextOptions(repoURL: URL) -> ContextOptions {
        let filteredAttached = attachedFiles.filter { !excludedFiles.contains($0.path()) }
        return ContextOptions(
            prompt: selectedPrompt?.name,
            args: promptArgs,
            contextFolders: Array(selectedContextFolders),
            attachedFiles: filteredAttached,
            includeDocs: includeDocs,
            includeDiff: includeDiff,
            includeDiffFiles: includeDiffFiles,
            includePaste: includePaste,
            includeSummaries: includeSummaries,
            repoURL: repoURL
        )
    }

    func refreshContextPreview(repoURL: URL) async {
        let options = buildContextOptions(repoURL: repoURL)
        contextPreview = await contextPreviewService.assemblePreview(options)
    }

    func copyAssembledContext(repoURL: URL) async -> String? {
        let options = buildContextOptions(repoURL: repoURL)
        return await contextPreviewService.copyAssembledContext(options)
    }

    func estimateTokens(repoURL: URL) async {
        let tokenService = TokenEstimator()
        estimatedTokens = await tokenService.estimate(
            prompt: selectedPrompt?.name,
            args: promptArgs,
            context: Array(selectedContextFolders),
            includeDocs: includeDocs,
            includeDiff: includeDiff,
            includeDiffFiles: includeDiffFiles,
            includePaste: includePaste,
            includeSummaries: includeSummaries,
            in: repoURL
        )
    }

    // MARK: - Command Building

    func buildCommand(
        flow: Flow? = nil,
        config: LoopflowConfig?,
        repoURL: URL?
    ) -> String {
        var parts = ["lf"]

        if let flow = flow {
            parts.append(flow.name)
            return parts.joined(separator: " ")
        }

        if let prompt = selectedPrompt {
            parts.append(prompt.name)
        } else if !promptArgs.isEmpty {
            parts.append(":")
        }

        if !promptArgs.isEmpty {
            parts.append(shellEscape(promptArgs))
        }

        if runMode == .interactive {
            parts.append("-i")
        } else {
            parts.append("-a")
        }

        for folder in selectedContextFolders {
            if let path = folder.path(percentEncoded: false).components(separatedBy: repoURL?.path() ?? "").last {
                parts.append("-x")
                parts.append(path.hasPrefix("/") ? String(path.dropFirst()) : path)
            }
        }

        for file in attachedFiles {
            let filePath = file.path(percentEncoded: false)
            if let repoPath = repoURL?.path(),
               filePath.hasPrefix(repoPath) {
                let relativePath = String(filePath.dropFirst(repoPath.count))
                parts.append("-x")
                parts.append(relativePath.hasPrefix("/") ? String(relativePath.dropFirst()) : relativePath)
            } else {
                parts.append("-x")
                parts.append(filePath)
            }
        }

        if !selectedDirections.isEmpty {
            let directionNames = selectedDirections.map { $0.name }.joined(separator: ",")
            parts.append("--goal")
            parts.append(directionNames)
        }

        if let model = selectedModel {
            let configDefault = config?.waveModel
            if model.cliValue != configDefault {
                parts.append("-m")
                parts.append(model.cliValue)
            }
        }

        if !includeDocs {
            parts.append("--no-docs")
        }
        if includeDiff {
            parts.append("--diff")
        }
        if !includeDiffFiles {
            parts.append("--no-diff-files")
        }
        if includePaste {
            parts.append("--paste")
        }
        if !includeSummaries {
            parts.append("--no-summaries")
        }

        let configChrome = config?.chrome ?? false
        if includeChrome != configChrome {
            parts.append(includeChrome ? "--chrome" : "--no-chrome")
        }

        return parts.joined(separator: " ")
    }

    // MARK: - Private

    private func listRootFolders(in url: URL, exclude: [String]) -> [URL] {
        let fm = FileManager.default
        guard let contents = try? fm.contentsOfDirectory(
            at: url,
            includingPropertiesForKeys: [.isDirectoryKey]
        ) else {
            return []
        }

        return contents.filter { item in
            let name = item.lastPathComponent
            guard !name.hasPrefix(".") else { return false }

            for pattern in exclude {
                if name == pattern || pattern.hasPrefix(name + "/") {
                    return false
                }
            }

            let isDir = (try? item.resourceValues(forKeys: [.isDirectoryKey]))?.isDirectory ?? false
            return isDir
        }
    }

    private func shellEscape(_ string: String) -> String {
        let needsQuoting = string.contains { char in
            " \t\n'\"\\$`!*?[]{}()<>|&;#~".contains(char)
        }

        if needsQuoting {
            let escaped = string.replacingOccurrences(of: "'", with: "'\\''")
            return "'\(escaped)'"
        }

        return string
    }
}
