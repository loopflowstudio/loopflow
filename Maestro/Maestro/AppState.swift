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
    var selectedModel: AgentModel?  // nil means use config default
    var promptArgs: String = ""
    var includeDocs: Bool = true
    var includeDiff: Bool = false
    var includeDiffFiles: Bool = true
    var includePaste: Bool = false
    var includeSummaries: Bool = true
    var includeChrome: Bool = false
    var selectedContextFolders: Set<URL> = []
    var attachedFiles: [URL] = []  // Files dropped or added via UI
    var runMode: RunMode = .auto
    var estimatedTokens: Int = 0

    // Context preview state
    var contextPreview: ContextPreview = .empty
    var excludedFiles: Set<String> = []  // Files excluded via preview panel

    // Sidebar state
    var selectedWorktree: Worktree?
    var selectedPipeline: PipelineDef?

    // Live output state
    var liveOutputBySession: [String: [OutputLine]] = [:]
    var activeSessionIds: Set<String> = []
    var activeWorktreePaths: Set<String> = []  // Worktree paths with running sessions
    private var sessionWorktreeMap: [String: String] = [:]  // session ID → worktree path

    // Results panel state
    var sessionBaselines: [String: SessionBaseline] = [:]  // session ID → baseline
    var sessionResults: [String: SessionResult] = [:]  // session ID → result
    var showResultsLog: Bool = false  // Toggle for streaming log view
    private var sessionTaskMap: [String: String] = [:]  // session ID → task name
    private var sessionStartMap: [String: Date] = [:]  // session ID → start time

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
    private let contextPreviewService = ContextPreviewService()
    private let resultsService = ResultsService()

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
            includeChrome = config?.chrome ?? false

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

            // Auto-create draft PRs for pushed branches without PRs
            await createDraftPRsIfNeeded(in: repo)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func createDraftPRsIfNeeded(in repo: URL) async {
        for worktree in worktrees {
            // Skip if: no commits, already has PR, or is main branch
            guard worktree.aheadMain > 0,
                  worktree.prNumber == nil,
                  worktree.branch != "main" else { continue }

            // Check if branch is pushed to remote
            let isPushed = await worktreeService.branchIsPushed(worktree.branch, in: repo)
            guard isPushed else { continue }

            // Create draft PR in background (don't block or show errors)
            Task.detached { [worktreeService] in
                let worktreeURL = URL(fileURLWithPath: worktree.path)
                try? await worktreeService.createDraftPR(branch: worktree.branch, in: worktreeURL)
            }
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

    func syncMain() async throws {
        guard let repo = currentRepo else { return }
        try await worktreeService.sync(in: repo)
        await refreshWorktrees()
    }

    func pruneWorktrees(dryRun: Bool = false) async throws -> [String] {
        guard let repo = currentRepo else { return [] }
        let pruned = try await worktreeService.prune(in: repo, dryRun: dryRun)
        if !dryRun {
            await refreshWorktrees()
        }
        return pruned
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
        // session.started events don't have status, just id/task/worktree
        if event.status == nil {
            // This is a session.started event
            activeSessionIds.insert(event.id)
            liveOutputBySession[event.id] = []
            if let worktree = event.worktree {
                activeWorktreePaths.insert(worktree)
                sessionWorktreeMap[event.id] = worktree
            }

            // Track task name and start time for results
            if let task = event.task {
                sessionTaskMap[event.id] = task
            }
            sessionStartMap[event.id] = Date()

            // Capture baseline for results computation
            if let worktree = event.worktree {
                Task {
                    let baseline = await resultsService.captureBaseline(
                        sessionId: event.id,
                        worktree: URL(fileURLWithPath: worktree)
                    )
                    await MainActor.run {
                        sessionBaselines[event.id] = baseline
                        // Create running result entry
                        sessionResults[event.id] = SessionResult.running(
                            sessionId: event.id,
                            task: event.task ?? "task",
                            worktree: worktree,
                            startedAt: sessionStartMap[event.id] ?? Date()
                        )
                    }
                }
            }
        } else if event.status == "completed" || event.status == "error" {
            activeSessionIds.remove(event.id)
            // Remove worktree from active set
            if let worktree = sessionWorktreeMap.removeValue(forKey: event.id) {
                // Only remove if no other sessions running in same worktree
                let otherSessionsInWorktree = sessionWorktreeMap.values.contains(worktree)
                if !otherSessionsInWorktree {
                    activeWorktreePaths.remove(worktree)
                }
            }

            // Compute results
            if let baseline = sessionBaselines[event.id] {
                let task = sessionTaskMap[event.id] ?? "task"
                let startedAt = sessionStartMap[event.id] ?? Date()
                let status: SessionResultStatus = event.status == "completed" ? .completed : .error

                Task {
                    let result = await resultsService.computeResults(
                        baseline: baseline,
                        task: task,
                        status: status,
                        startedAt: startedAt,
                        endedAt: Date()
                    )
                    await MainActor.run {
                        sessionResults[event.id] = result
                    }
                }
            }

            // Clean up tracking maps
            sessionTaskMap.removeValue(forKey: event.id)
            sessionStartMap.removeValue(forKey: event.id)
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

    func isWorktreeRunning(_ worktree: Worktree) -> Bool {
        activeWorktreePaths.contains(worktree.path)
    }

    // MARK: - Results Panel

    func loadDiffPreview(for sessionId: String, fileIndex: Int) async {
        guard var result = sessionResults[sessionId],
              fileIndex < result.filesChanged.count,
              result.filesChanged[fileIndex].diffPreview == nil,
              let baseline = sessionBaselines[sessionId] else { return }

        let file = result.filesChanged[fileIndex]
        let worktree = URL(fileURLWithPath: result.worktree)

        let preview = await resultsService.loadDiffPreview(
            for: file,
            baselineSHA: baseline.headSHA,
            in: worktree
        )

        result.filesChanged[fileIndex].diffPreview = preview
        sessionResults[sessionId] = result
    }

    func clearCompletedResults() {
        // Clear results for sessions that are complete (keep running ones)
        for sessionId in sessionResults.keys {
            if let result = sessionResults[sessionId], result.status != .running {
                sessionResults.removeValue(forKey: sessionId)
                sessionBaselines.removeValue(forKey: sessionId)
                liveOutputBySession.removeValue(forKey: sessionId)
            }
        }
    }

    var currentSessionResult: SessionResult? {
        // Filter by selected worktree if one is selected
        let results = sessionResults.values
        let filtered: [SessionResult]

        if let selectedPath = selectedWorktree?.path {
            filtered = results.filter { $0.worktree == selectedPath }
        } else {
            filtered = Array(results)
        }

        // Return the most recent result for this worktree
        return filtered
            .sorted { $0.startedAt > $1.startedAt }
            .first
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

    func refreshContextPreview() async {
        guard let options = buildContextOptions() else { return }
        contextPreview = await contextPreviewService.assemblePreview(options)
    }

    func removeContextItem(_ item: ContextItem, from section: ContextSection) {
        guard let path = item.path else { return }

        if section.kind == .attached {
            attachedFiles.removeAll { $0.path() == path }
        } else {
            excludedFiles.insert(path)
        }

        Task {
            await refreshContextPreview()
            await estimateTokens()
        }
    }

    func copyAssembledContext() async -> String? {
        guard let options = buildContextOptions() else { return nil }
        return await contextPreviewService.copyAssembledContext(options)
    }

    private func buildContextOptions() -> ContextOptions? {
        guard let repo = currentRepo else { return nil }
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
            repoURL: repo
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

        // Model (only include if different from config default)
        if let model = selectedModel {
            let configDefault = config?.agentModel
            if model.cliValue != configDefault {
                parts.append("-m")
                parts.append(model.cliValue)
            }
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

        // Chrome flag (only include if different from config default)
        let configChrome = config?.chrome ?? false
        if includeChrome != configChrome {
            parts.append(includeChrome ? "--chrome" : "--no-chrome")
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
