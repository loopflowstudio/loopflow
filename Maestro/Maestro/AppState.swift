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
    enum UITestMode: String {
        case emptyWorkspaces = "empty-workspaces"
        case sampleWorkspaces = "sample-workspaces"
    }

    struct ScreenshotMode {
        let outputPath: String
        let repoPath: String?
        let windowSize: (Int, Int)?
        let selectBranch: String?
        let mockLoops: Bool

        static func fromArgs() -> ScreenshotMode? {
            let args = ProcessInfo.processInfo.arguments
            guard let captureIndex = args.firstIndex(of: "--capture"),
                  args.count > captureIndex + 1 else {
                return nil
            }

            let outputPath = args[captureIndex + 1]
            var repoPath: String?
            var windowSize: (Int, Int)?
            var selectBranch: String?
            var mockLoops = false

            if let repoIndex = args.firstIndex(of: "--repo"), args.count > repoIndex + 1 {
                repoPath = args[repoIndex + 1]
            }

            if let sizeIndex = args.firstIndex(of: "--size"), args.count > sizeIndex + 1 {
                let sizeStr = args[sizeIndex + 1]
                let parts = sizeStr.split(separator: "x")
                if parts.count == 2, let w = Int(parts[0]), let h = Int(parts[1]) {
                    windowSize = (w, h)
                }
            }

            if let selectIndex = args.firstIndex(of: "--select"), args.count > selectIndex + 1 {
                selectBranch = args[selectIndex + 1]
            }

            if args.contains("--mock-loops") {
                mockLoops = true
            }

            return ScreenshotMode(
                outputPath: outputPath,
                repoPath: repoPath,
                windowSize: windowSize,
                selectBranch: selectBranch,
                mockLoops: mockLoops
            )
        }
    }

    var currentRepo: URL?
    var config: LoopflowConfig?
    var worktrees: [Worktree] = []
    var prompts: [PromptCard] = []
    var pipelines: [PipelineDef] = []
    var voices: [Voice] = []
    var loops: [Loop] = []

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
    var selectedLoop: Loop?

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
    private var autoPruneInFlight: Bool = false
    private var autoSyncTask: Task<Void, Never>?
    private var listDebounceTask: Task<Void, Never>?
    var refreshMessage: String?
    var isRefreshingWorktrees: Bool = false

    // Loading state
    var isLoading: Bool = false
    var errorMessage: String?

    // Daemon connection state
    var lfdConnected: Bool = false

    // Services
    private let worktreeService = WorktreeService()
    private let configLoader = ConfigLoader()
    private let promptService = PromptService()
    private let pipelineService = PipelineService()
    private let loopService = LoopService()
    private var eventService: LFDEventService?
    private let voiceService = VoiceService()
    private let contextPreviewService = ContextPreviewService()
    private let resultsService = ResultsService()

    static func uiTestMode() -> UITestMode? {
        let args = ProcessInfo.processInfo.arguments
        if let index = args.firstIndex(of: "-ui-test-mode"), args.count > index + 1 {
            return UITestMode(rawValue: args[index + 1])
        }
        if let mode = ProcessInfo.processInfo.environment["MAESTRO_UI_TEST_MODE"] {
            return UITestMode(rawValue: mode)
        }
        return nil
    }

    func configureForUITest(_ mode: UITestMode, repoURL: URL) {
        currentRepo = repoURL
        config = nil
        prompts = []
        pipelines = []
        voices = []
        loops = []
        selectedWorktree = nil
        isLoading = false
        errorMessage = nil

        switch mode {
        case .emptyWorkspaces:
            worktrees = []
        case .sampleWorkspaces:
            worktrees = [
                Worktree(
                    path: "/tmp/loopflow-ui-tests/feature-a",
                    branch: "feature-a",
                    baseBranch: "main",
                    isDirty: false,
                    aheadMain: 2,
                    behindMain: 0,
                    aheadRemote: 0,
                    behindRemote: 0,
                    prURL: URL(string: "https://github.com/org/repo/pull/12"),
                    prNumber: 12,
                    prState: .open,
                    hasCodeWorkspace: false,
                    isRebasing: false,
                    isMerging: false
                )
            ]
        }
    }

    func configureMockLoops() {
        loops = [
            Loop(
                id: "mock-loop-1",
                type: .loop,
                goalName: "test-coverage",
                repo: currentRepo?.path ?? "/tmp/demo",
                loopMain: "loop-test-coverage",
                status: .running,
                iteration: 3,
                prLimit: 5,
                mergeMode: .pr,
                pid: 12345,
                createdAt: Date().addingTimeInterval(-3600),
                currentRunId: "run-123",
                currentStep: "implement"
            ),
            Loop(
                id: "mock-loop-2",
                type: .loop,
                goalName: "docs-sync",
                repo: currentRepo?.path ?? "/tmp/demo",
                loopMain: "loop-docs-sync",
                status: .idle,
                iteration: 12,
                prLimit: 3,
                mergeMode: .pr,
                pid: nil,
                createdAt: Date().addingTimeInterval(-86400)
            )
        ]
        lfdConnected = true
    }

    func openRepo(_ url: URL) async {
        currentRepo = url
        isLoading = true
        errorMessage = nil

        do {
            // Fast path: load config and list worktrees (no sync)
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
                    selectedContextFolders = Set(listRootFolders(in: url))
                } else {
                    selectedContextFolders = Set(contextPaths.map { url.appendingPathComponent($0) })
                }
            } else {
                selectedContextFolders = []
            }

            // List worktrees immediately (no sync - fast)
            worktrees = try await worktreeService.list(in: url)

            // Auto-select first feature worktree
            if selectedWorktree == nil {
                selectedWorktree = worktrees.first { $0.branch != "main" }
            }

            // Load prompts and voices (fast, local files)
            prompts = try promptService.loadPrompts(from: url, config: config)
            pipelines = pipelineService.loadPipelines(from: url)
            refreshVoices()

            if let voiceNames = config?.voiceNames, !voiceNames.isEmpty {
                selectedVoices = voices.filter { voiceNames.contains($0.name) }
            }
        } catch {
            errorMessage = error.localizedDescription
        }

        isLoading = false

        // Slow operations in background (don't block UI)
        Task {
            // Start daemon and event subscription
            let setupService = SetupService()
            if (try? await setupService.ensureDaemonRunning()) != nil {
                startEventSubscription()
            }

            // Background enrichment: sync, loops, tokens, staleness
            await syncAndEnrich()
            await refreshLoops()
            await estimateTokens()
        }

        startAutoSyncTimer()
    }

    // MARK: - Refresh Operations

    /// Fast list refresh - debounced, no sync. Use for LFD events and after actions.
    func listWorktrees() {
        listDebounceTask?.cancel()
        listDebounceTask = Task {
            try? await Task.sleep(for: .milliseconds(100))
            guard !Task.isCancelled else { return }
            await _listWorktrees()
        }
    }

    private func _listWorktrees() async {
        guard let repo = currentRepo else { return }
        do {
            worktrees = try await worktreeService.list(in: repo)
            if let branch = selectedWorktree?.branch {
                selectedWorktree = worktrees.first { $0.branch == branch }
            }
        } catch {
            // Silent failure for background refresh
        }
    }

    /// Manual refresh with feedback - syncs and lists.
    func refreshWorktrees(showFeedback: Bool = false) async {
        guard let repo = currentRepo else { return }

        do {
            if showFeedback {
                isRefreshingWorktrees = true
                refreshMessage = "Syncing..."
            }
            let syncSucceeded = (try? await worktreeService.sync(in: repo)) != nil
            worktrees = try await worktreeService.list(in: repo)

            if let branch = selectedWorktree?.branch {
                selectedWorktree = worktrees.first { $0.branch == branch }
            }

            if showFeedback {
                refreshMessage = syncSucceeded ? "Refreshed" : "Refresh (sync failed)"
            }
        } catch {
            errorMessage = error.localizedDescription
            if showFeedback {
                refreshMessage = "Refresh failed"
            }
        }

        if showFeedback {
            isRefreshingWorktrees = false
            Task { @MainActor in
                try? await Task.sleep(for: .seconds(2))
                if refreshMessage != nil {
                    refreshMessage = nil
                }
            }
        }
    }

    /// Background enrichment - sync, staleness, CI, draft PRs. Called by timer.
    private func syncAndEnrich() async {
        guard let repo = currentRepo else { return }

        // Sync with remote
        _ = try? await worktreeService.sync(in: repo)

        // Refresh list
        await _listWorktrees()

        // Enrich with slow operations (staleness, CI, draft PRs)
        await detectStaleness()
        await fetchCIStatus()
        await createDraftPRsIfNeeded(in: repo)
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

            let hasDiff = await worktreeService.hasDiffAgainstBase(worktree)
            guard hasDiff else { continue }

            // Create draft PR in background (don't block or show errors)
            Task.detached { [worktreeService] in
                let worktreeURL = URL(fileURLWithPath: worktree.path)
                try? await worktreeService.createDraftPR(branch: worktree.branch, in: worktreeURL)
            }
        }
    }

    private func detectStaleness() async {
        guard let repo = currentRepo else { return }

        let prunable = await worktreeService.getPrunableBranches(in: repo)
        var stalenessMap = await worktreeService.detectStalenessForAll(worktrees, in: repo)

        for branch in prunable {
            stalenessMap[branch] = .merged
        }

        // Update worktrees with staleness info
        for i in worktrees.indices {
            if let staleness = stalenessMap[worktrees[i].branch] {
                worktrees[i].staleness = staleness
            }
        }

        await autoPruneCompletedWorktrees(stalenessMap, in: repo)
    }

    private func fetchCIStatus() async {
        guard let repo = currentRepo else { return }

        let ciStatusMap = await worktreeService.getCIStatusForAll(worktrees, in: repo)

        // Update worktrees with CI status
        for i in worktrees.indices {
            if let status = ciStatusMap[worktrees[i].branch] {
                worktrees[i].ciStatus = status
            }
        }
    }

    private func autoPruneCompletedWorktrees(_ stalenessMap: [String: Staleness], in repo: URL) async {
        if autoPruneInFlight {
            return
        }

        let candidates = worktrees.filter { worktree in
            guard worktree.branch != "main", worktree.branch != "master" else { return false }
            guard worktree.isDirty == false, worktree.isMerging == false, worktree.isRebasing == false else { return false }
            guard let staleness = stalenessMap[worktree.branch] else { return false }

            switch staleness {
            case .merged:
                return true
            case .remoteDeleted:
                return worktree.aheadMain == 0
            case .active, .inactive:
                return false
            }
        }

        if candidates.isEmpty {
            return
        }

        autoPruneInFlight = true
        let branchesToPrune = Set(candidates.map(\.branch))

        // Remove from local state immediately (optimistic update)
        worktrees.removeAll { branchesToPrune.contains($0.branch) }
        if let selected = selectedWorktree, branchesToPrune.contains(selected.branch) {
            selectedWorktree = nil
        }

        // Then delete from disk in background (no cascading refresh)
        Task.detached { [worktreeService] in
            for branch in branchesToPrune {
                _ = try? await worktreeService.remove(name: branch, in: repo)
            }
        }

        autoPruneInFlight = false
    }

    private func startAutoSyncTimer() {
        autoSyncTask?.cancel()
        autoSyncTask = Task { [weak self] in
            while !Task.isCancelled {
                guard let self else { return }
                await self.syncAndEnrich()
                try? await Task.sleep(for: .seconds(60))
            }
        }
    }

    func createWorktree(name: String, baseBranch: String? = nil) async throws {
        guard let repo = currentRepo else { return }

        try await worktreeService.create(name: name, in: repo, baseBranch: baseBranch)
        await _listWorktrees()  // Immediate refresh - caller needs the new worktree in the list
    }

    func deleteWorktree(_ worktree: Worktree) async throws {
        guard let repo = currentRepo else { return }

        try await worktreeService.remove(name: worktree.branch, in: repo)
        listWorktrees()
    }

    func createPR(for worktree: Worktree) async throws {
        let worktreeURL = URL(fileURLWithPath: worktree.path)
        try await worktreeService.createPR(in: worktreeURL)
        listWorktrees()
    }

    func landPR(for worktree: Worktree) async throws {
        let worktreeURL = URL(fileURLWithPath: worktree.path)
        try await worktreeService.landPR(in: worktreeURL)
        listWorktrees()
    }

    func syncMain() async throws {
        guard let repo = currentRepo else { return }
        try await worktreeService.sync(in: repo)
        listWorktrees()
    }

    func pruneWorktrees(dryRun: Bool = false) async throws -> [String] {
        guard let repo = currentRepo else { return [] }
        let pruned = try await worktreeService.prune(in: repo, dryRun: dryRun)
        if !dryRun {
            listWorktrees()
        }
        return pruned
    }

    func startEventSubscription() {
        eventService = LFDEventService()

        Task {
            await eventService?.subscribe(
                to: ["worktree.*", "session.*", "output.line", "loop.*"],
                onEvent: { [weak self] event in
                    Task { @MainActor in
                        switch event {
                        case .worktree:
                            self?.listWorktrees()
                        case .session(let sessionEvent):
                            self?.handleSessionEvent(sessionEvent)
                        case .output(let outputEvent):
                            self?.handleOutputEvent(outputEvent)
                        case .loop(_):
                            await self?.refreshLoops()
                        }
                    }
                },
                onConnectionChange: { [weak self] connected in
                    Task { @MainActor in
                        self?.lfdConnected = connected
                    }
                }
            )
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

    func refreshLoops() async {
        guard let repo = currentRepo else { return }
        do {
            loops = try await loopService.listLoops(repo: repo)
        } catch {
            loops = []
        }
    }

    func liveOutput(for loop: Loop) -> [OutputLine] {
        // Loop runs use their run ID as session ID for output tracking
        guard let runId = loop.currentRunId else { return [] }
        return liveOutputBySession[runId] ?? []
    }

    func squashLandLoop(_ loop: Loop) async throws {
        guard let repo = currentRepo else { return }
        try await loopService.squashLand(loop: loop, repoRoot: repo)
        await refreshLoops()
        listWorktrees()
    }

    func connectLfd() async throws {
        try await loopService.connectLfd()
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
