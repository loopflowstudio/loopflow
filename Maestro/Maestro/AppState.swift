// Central app state using Observable macro (macOS 15+).

import Foundation
import SwiftUI

struct OutputLine: Identifiable {
    let id = UUID()
    let text: String
    let timestamp: Date
}

@MainActor
@Observable
final class AppState {
    var currentRepo: URL?
    var config: LoopflowConfig?
    var worktrees: [Worktree] = []
    var prompts: [PromptCard] = []
    var pipelines: [PipelineDef] = []
    var agents: [Agent] = []
    var voices: [Voice] = []

    // Prompt launcher state
    var selectedPrompt: PromptCard?
    var selectedVoices: [Voice] = []
    var promptArgs: String = ""
    var includeDocs: Bool = true
    var includeDiff: Bool = false
    var includeDiffFiles: Bool = true
    var includePaste: Bool = false
    var includeSummaries: Bool = true
    var selectedContextFolders: Set<URL> = []
    var attachedFiles: [URL] = []  // Files dropped or added via UI
    var runMode: RunMode = .auto
    var estimatedTokens: Int = 0

    // Sidebar state
    var selectedWorktree: Worktree?
    var selectedPipeline: PipelineDef?

    // Live output state
    var liveOutputBySession: [String: [OutputLine]] = [:]
    var activeSessionIds: Set<String> = []

    // Loading state
    var isLoading: Bool = false
    var errorMessage: String?
    var agentsAvailable: Bool = false

    // Services
    private let worktreeService = WorktreeService()
    private let configLoader = ConfigLoader()
    private let promptService = PromptService()
    private let pipelineService = PipelineService()
    private let agentService = AgentService()
    private var eventService: LFDEventService?
    private let voiceService = VoiceService()

    func openRepo(_ url: URL) async {
        currentRepo = url
        isLoading = true
        errorMessage = nil

        // Ensure daemon is running on first launch
        let setupService = SetupService()
        do {
            try await setupService.ensureDaemonRunning()
            // Start event subscription after daemon is running
            startEventSubscription()
        } catch {
            // Non-fatal - daemon setup failed but app can still work
        }

        do {
            config = try configLoader.load(from: url)

            // Initialize toggles from config
            includeDocs = config?.docs ?? true
            includeDiff = config?.diff ?? false
            includeDiffFiles = config?.diffFiles ?? true
            includePaste = config?.paste ?? false
            includeSummaries = config?.hasSummaries ?? false

            // Initialize context folders from config
            if let contextPaths = config?.context {
                if contextPaths == ["."] {
                    // "." means include all - select all root directories
                    selectedContextFolders = Set(listRootFolders(in: url))
                } else {
                    selectedContextFolders = Set(contextPaths.map { path in
                        url.appendingPathComponent(path)
                    })
                }
            } else {
                selectedContextFolders = []
            }

            await refreshWorktrees()

            // Auto-select first worktree so launch button always has a target
            if selectedWorktree == nil {
                selectedWorktree = worktrees.first
            }

            prompts = try promptService.loadPrompts(from: url, config: config)
            pipelines = pipelineService.loadPipelines(from: url)
            refreshVoices()

            // Initialize selected voices from config
            if let voiceNames = config?.voiceNames, !voiceNames.isEmpty {
                selectedVoices = voices.filter { voiceNames.contains($0.name) }
            }

            await estimateTokens()
            await refreshAgents()
        } catch {
            errorMessage = error.localizedDescription
        }

        isLoading = false
    }

    func refreshWorktrees() async {
        guard let repo = currentRepo else { return }

        do {
            let previousSelection = selectedWorktree?.branch
            worktrees = try await worktreeService.list(in: repo)

            // Preserve selection by matching on branch name
            if let branch = previousSelection {
                selectedWorktree = worktrees.first { $0.branch == branch }
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func createWorktree(name: String, baseBranch: String? = nil) async throws {
        guard let repo = currentRepo else { return }

        try await worktreeService.create(name: name, in: repo, baseBranch: baseBranch)
        await refreshWorktrees()
    }

    func deleteWorktree(_ worktree: Worktree) async throws {
        guard let repo = currentRepo else { return }

        try await worktreeService.remove(name: worktree.branch, in: repo)
        await refreshWorktrees()
    }

    func createPR(for worktree: Worktree) async throws {
        let worktreeURL = URL(fileURLWithPath: worktree.path)
        try await worktreeService.createPR(in: worktreeURL)
        await refreshWorktrees()
    }

    func landPR(for worktree: Worktree) async throws {
        let worktreeURL = URL(fileURLWithPath: worktree.path)
        try await worktreeService.landPR(in: worktreeURL)
        await refreshWorktrees()
    }

    func startEventSubscription() {
        eventService = LFDEventService()

        Task {
            try? await eventService?.subscribe(
                to: ["worktree.*", "session.*", "output.line"]
            ) { [weak self] event in
                Task { @MainActor in
                    switch event {
                    case .worktree:
                        await self?.refreshWorktrees()
                    case .session(let sessionEvent):
                        self?.handleSessionEvent(sessionEvent)
                    case .output(let outputEvent):
                        self?.handleOutputEvent(outputEvent)
                    }
                }
            }
        }
    }

    func refreshVoices() {
        guard let repo = currentRepo else { return }
        voices = voiceService.loadVoices(from: repo)
    }

    func refreshPipelines() {
        guard let repo = currentRepo else { return }
        pipelines = pipelineService.loadPipelines(from: repo)
    }

    func createPipeline(name: String) {
        guard let repo = currentRepo else { return }
        let newPipeline = PipelineDef(name: name, steps: [])
        do {
            try pipelineService.savePipeline(newPipeline, in: repo)
            refreshPipelines()
            selectedPipeline = pipelines.first { $0.name == name }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func savePipeline(_ pipeline: PipelineDef) {
        guard let repo = currentRepo else { return }
        do {
            try pipelineService.savePipeline(pipeline, in: repo)
            refreshPipelines()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func deletePipeline(_ pipeline: PipelineDef) {
        guard let repo = currentRepo else { return }
        do {
            try pipelineService.deletePipeline(named: pipeline.name, in: repo)
            if selectedPipeline?.name == pipeline.name {
                selectedPipeline = nil
            }
            refreshPipelines()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func handleSessionEvent(_ event: SessionEvent) {
        if event.status == "running" {
            activeSessionIds.insert(event.id)
            liveOutputBySession[event.id] = []
        } else if event.status == "completed" || event.status == "error" {
            activeSessionIds.remove(event.id)
        }
    }

    private func handleOutputEvent(_ event: OutputEvent) {
        guard activeSessionIds.contains(event.sessionId) else { return }

        let line = OutputLine(text: event.text, timestamp: event.timestamp)
        liveOutputBySession[event.sessionId, default: []].append(line)

        // Cap buffer at 1000 lines to prevent memory bloat
        if liveOutputBySession[event.sessionId]?.count ?? 0 > 1000 {
            liveOutputBySession[event.sessionId]?.removeFirst()
        }
    }

    func refreshAgents() async {
        do {
            agents = try await agentService.list()
            agentsAvailable = true
        } catch {
            // API not running - agents feature unavailable
            agents = []
            agentsAvailable = false
        }
    }

    func startAgent(_ agent: Agent) async throws {
        guard let repo = currentRepo else { return }
        try await agentService.start(agentId: agent.id, repoRoot: repo)
        await refreshAgents()
    }

    func stopAgent(_ agent: Agent) async throws {
        try await agentService.stop(agentId: agent.id)
        await refreshAgents()
    }

    func estimateTokens() async {
        guard let repo = currentRepo else { return }

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
            in: repo
        )
    }

    func buildCommand(pipeline: PipelineDef? = nil) -> String {
        var parts = ["lf"]

        // If running a pipeline, just use the pipeline name
        if let pipeline = pipeline {
            parts.append(pipeline.name)
            // Pipelines don't take args or many of the flags
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

        // Context folders
        for folder in selectedContextFolders {
            if let path = folder.path(percentEncoded: false).components(separatedBy: currentRepo?.path() ?? "").last {
                parts.append("-x")
                parts.append(path.hasPrefix("/") ? String(path.dropFirst()) : path)
            }
        }

        // Attached files (relative paths if inside repo, otherwise absolute)
        for file in attachedFiles {
            let filePath = file.path(percentEncoded: false)
            if let repoPath = currentRepo?.path(),
               filePath.hasPrefix(repoPath) {
                // Relative path
                let relativePath = String(filePath.dropFirst(repoPath.count))
                parts.append("-x")
                parts.append(relativePath.hasPrefix("/") ? String(relativePath.dropFirst()) : relativePath)
            } else {
                // Absolute path for files outside repo
                parts.append("-x")
                parts.append(filePath)
            }
        }

        // Voices
        if !selectedVoices.isEmpty {
            let voiceNames = selectedVoices.map { $0.name }.joined(separator: ",")
            parts.append("--voice")
            parts.append(voiceNames)
        }

        // Docs/diff/paste/summaries flags (only include if different from default)
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

        return parts.joined(separator: " ")
    }

    private func listRootFolders(in url: URL) -> [URL] {
        let fm = FileManager.default
        guard let contents = try? fm.contentsOfDirectory(
            at: url,
            includingPropertiesForKeys: [.isDirectoryKey]
        ) else {
            return []
        }

        let excludePatterns = config?.exclude ?? []

        return contents.filter { item in
            let name = item.lastPathComponent
            // Filter hidden files
            guard !name.hasPrefix(".") else { return false }

            // Check if excluded by config patterns
            for pattern in excludePatterns {
                if name == pattern || pattern.hasPrefix(name + "/") {
                    return false
                }
            }

            let isDir = (try? item.resourceValues(forKeys: [.isDirectoryKey]))?.isDirectory ?? false
            return isDir
        }
    }

    private func shellEscape(_ string: String) -> String {
        // If string contains any shell metacharacters, wrap in single quotes
        // and escape any single quotes within
        let needsQuoting = string.contains { char in
            " \t\n'\"\\$`!*?[]{}()<>|&;#~".contains(char)
        }

        if needsQuoting {
            // Use single quotes, escaping embedded single quotes as '\''
            let escaped = string.replacingOccurrences(of: "'", with: "'\\''")
            return "'\(escaped)'"
        }

        return string
    }
}
