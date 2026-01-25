// Central app state using Observable macro (macOS 15+).

import Foundation
import SwiftUI
import LoopflowCore

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
    var flows: [Flow] = []
    var goals: [Goal] = []
    var agents: [Agent] = []

    // Prompt launcher state
    var selectedPrompt: PromptCard?
    var selectedGoals: [Goal] = []
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

    // Sidebar state - agent is primary selection
    var selectedAgent: Agent?
    var selectedFlow: Flow?

    // Computed: worktree for selected agent (worktrees are implementation details)
    var selectedWorktree: Worktree? {
        get {
            guard let agent = selectedAgent,
                  let path = agent.worktreePath else { return _selectedWorktree }
            return worktrees.first { $0.path == path } ?? _selectedWorktree
        }
        set {
            _selectedWorktree = newValue
        }
    }
    private var _selectedWorktree: Worktree?

    // Live output state
    var liveOutputBySession: [String: [OutputLine]] = [:]
    var activeSessionIds: Set<String> = []
    var activeWorktreePaths: Set<String> = []  // Worktree paths with running sessions
    private var stepRunWorktreeMap: [String: String] = [:]  // step run ID → worktree path

    // Results panel state
    var stepRunBaselines: [String: StepRunBaseline] = [:]  // step run ID → baseline
    var stepRunResults: [String: StepRunResult] = [:]  // step run ID → result
    var showResultsLog: Bool = false  // Toggle for streaming log view
    private var stepRunStepMap: [String: String] = [:]  // step run ID → step/prompt name
    private var stepRunStartMap: [String: Date] = [:]  // step run ID → start time
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
    private let flowService = FlowService()
    private let agentService = AgentService()
    private var eventService: LFDEventService?
    private let goalService = GoalService()
    private let contextPreviewService = ContextPreviewService()
    private let resultsService = ResultsService()

    static func uiTestMode() -> UITestMode? {
        let args = ProcessInfo.processInfo.arguments
        if let index = args.firstIndex(of: "-ui-test-mode"), args.count > index + 1 {
            return UITestMode(rawValue: args[index + 1])
        }
        if let mode = ProcessInfo.processInfo.environment["CONCERTO_UI_TEST_MODE"] {
            return UITestMode(rawValue: mode)
        }
        return nil
    }

    func configureForUITest(_ mode: UITestMode, repoURL: URL) {
        currentRepo = repoURL
        config = nil
        prompts = []
        flows = []
        goals = []
        agents = []
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
                    isMerging: false,
                    hasDiff: true
                )
            ]
        }
    }

    func configureMockAgents() {
        agents = [
            Agent(
                id: "mock-agent-1",
                name: "swift-falcon",
                area: ["src/auth"],
                goal: [],
                flow: "ship",
                repo: currentRepo?.path ?? "/tmp/demo",
                stimulus: Stimulus(kind: .loop),
                paused: false,
                status: .running,
                iteration: 3,
                worktreePath: nil,
                branch: "agent-auth-feature",
                prLimit: 5,
                mergeMode: .pr,
                pid: 12345,
                createdAt: Date().addingTimeInterval(-3600)
            ),
            Agent(
                id: "mock-agent-2",
                name: "crystal-melody",
                area: ["src/api"],
                goal: ["product-engineer"],
                flow: "ship",
                repo: currentRepo?.path ?? "/tmp/demo",
                stimulus: Stimulus(kind: .loop),
                paused: false,
                status: .waiting,
                iteration: 5,
                worktreePath: nil,
                branch: "agent-api-refactor",
                prLimit: 3,
                mergeMode: .pr,
                pid: nil,
                createdAt: Date().addingTimeInterval(-7200)
            ),
            Agent(
                id: "mock-agent-3",
                name: "Quick fix",
                area: ["."],
                goal: [],
                flow: "debug",
                repo: currentRepo?.path ?? "/tmp/demo",
                stimulus: Stimulus(kind: .manual),
                paused: true,
                status: .idle,
                iteration: 0,
                worktreePath: nil,
                branch: nil,
                prLimit: 5,
                mergeMode: .pr,
                pid: nil,
                createdAt: Date().addingTimeInterval(-86400)
            ),
            Agent(
                id: "mock-agent-4",
                name: "Nightly polish",
                area: ["."],
                goal: [],
                flow: "polish",
                repo: currentRepo?.path ?? "/tmp/demo",
                stimulus: Stimulus(kind: .cron, cron: "0 9 * * *"),
                paused: true,
                status: .idle,
                iteration: 12,
                worktreePath: nil,
                branch: nil,
                prLimit: 5,
                mergeMode: .pr,
                pid: nil,
                createdAt: Date().addingTimeInterval(-172800)
            )
        ]
        lfdConnected = true
    }

    func openRepo(_ url: URL) async {
        let startTime = CFAbsoluteTimeGetCurrent()
        currentRepo = url
        isLoading = true
        errorMessage = nil

        do {
            // Fast path: load config and list worktrees (no sync)
            let t0 = CFAbsoluteTimeGetCurrent()
            config = try configLoader.load(from: url)
            LoggingService.append("openRepo.config elapsed=\(Int((CFAbsoluteTimeGetCurrent() - t0) * 1000))ms")

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

            // List worktrees immediately (no sync, no full details - fast)
            let t1 = CFAbsoluteTimeGetCurrent()
            worktrees = try await worktreeService.list(in: url, full: false)
            LoggingService.append("openRepo.listWorktrees elapsed=\(Int((CFAbsoluteTimeGetCurrent() - t1) * 1000))ms")

            // Auto-select first feature worktree
            if selectedWorktree == nil {
                selectedWorktree = worktrees.first { $0.branch != "main" }
            }

            // Load prompts and goals (fast, local files)
            let t2 = CFAbsoluteTimeGetCurrent()
            prompts = try promptService.loadPrompts(from: url, config: config)
            flows = flowService.loadFlows(from: url)  // Sync fallback
            refreshGoals()
            LoggingService.append("openRepo.prompts elapsed=\(Int((CFAbsoluteTimeGetCurrent() - t2) * 1000))ms")

            if let goalNames = config?.goalNames, !goalNames.isEmpty {
                selectedGoals = goals.filter { goalNames.contains($0.name) }
            }
        } catch {
            errorMessage = error.localizedDescription
        }

        isLoading = false
        LoggingService.append("openRepo.total elapsed=\(Int((CFAbsoluteTimeGetCurrent() - startTime) * 1000))ms")

        // Slow operations in background (don't block UI)
        Task {
            // Try to start daemon, but always subscribe (reconnect loop handles failures)
            let setupService = SetupService()
            try? await setupService.ensureDaemonRunning()
            startEventSubscription()

            // Background enrichment: sync, agents, flows, tokens, staleness
            await syncAndEnrich()
            await refreshAgents()
            await refreshFlowsAsync()  // Load flows from API (includes builtins and steps)
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

    /// Background enrichment - sync, staleness, CI. Called by timer.
    private func syncAndEnrich() async {
        guard let repo = currentRepo else { return }

        // Sync with remote
        _ = try? await worktreeService.sync(in: repo)

        // Refresh list (lfd provides staleness + CI, CLI fallback needs separate enrichment)
        await _listWorktrees()

        // Skip staleness/CI detection when lfd is available (already included in response)
        let lfdAvailable = await LFDClient.shared.isAvailable
        if !lfdAvailable {
            await detectStaleness()
            await fetchCIStatus()
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

    func landBranch(for worktree: Worktree) async throws {
        let worktreeURL = URL(fileURLWithPath: worktree.path)
        try await worktreeService.landBranch(in: worktreeURL)
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
        LoggingService.append("startEventSubscription called, eventService=\(eventService != nil)", category: LoggingService.Category.lfd)
        // Don't create duplicate subscriptions
        if eventService != nil { return }
        LoggingService.append("creating LFDEventService", category: LoggingService.Category.lfd)
        eventService = LFDEventService()

        Task {
            await eventService?.subscribe(
                to: ["worktree.*", "session.*", "output.line", "agent.*"],
                onEvent: { [weak self] event in
                    Task { @MainActor in
                        switch event {
                        case .worktree(let worktreeEvent):
                            self?.handleWorktreeEvent(worktreeEvent)
                        case .session(let sessionEvent):
                            self?.handleSessionEvent(sessionEvent)
                        case .output(let outputEvent):
                            self?.handleOutputEvent(outputEvent)
                        case .agent(_):
                            await self?.refreshAgents()
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

    private func handleWorktreeEvent(_ event: WorktreeEvent) {
        // Handle pruned events - remove worktree from list
        if event.name == "worktree.pruned", let branch = event.branch {
            worktrees.removeAll { $0.branch == branch }
            if selectedWorktree?.branch == branch {
                selectedWorktree = nil
            }
            return
        }

        // If event includes full worktree status, update in-place (no re-fetch needed)
        if let updatedWorktree = event.worktree, let branch = event.branch {
            if let index = worktrees.firstIndex(where: { $0.branch == branch }) {
                worktrees[index] = updatedWorktree
                if selectedWorktree?.branch == branch {
                    selectedWorktree = updatedWorktree
                }
            } else {
                // New worktree - add to list
                worktrees.append(updatedWorktree)
            }
            return
        }

        // Fall back to full refresh if event doesn't include status
        listWorktrees()
    }

    func refreshGoals() {
        guard let repo = currentRepo else { return }
        goals = goalService.loadGoals(from: repo)
    }

    func refreshFlows() {
        guard let repo = currentRepo else { return }
        flows = flowService.loadFlows(from: repo)
    }

    func refreshFlowsAsync() async {
        guard let repo = currentRepo else { return }
        let loaded = await flowService.loadFlowsAsync(from: repo)
        if !loaded.isEmpty {
            flows = loaded
        }
    }

    func createFlow(name: String) {
        guard let repo = currentRepo else { return }
        let newFlow = Flow(name: name, steps: [])
        do {
            try flowService.saveFlow(newFlow, in: repo)
            refreshFlows()
            selectedFlow = flows.first { $0.name == name }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func saveFlow(_ flow: Flow) {
        guard let repo = currentRepo else { return }
        do {
            try flowService.saveFlow(flow, in: repo)
            refreshFlows()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func deleteFlow(_ flow: Flow) {
        guard let repo = currentRepo else { return }
        do {
            try flowService.deleteFlow(named: flow.name, in: repo)
            if selectedFlow?.name == flow.name {
                selectedFlow = nil
            }
            refreshFlows()
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
                stepRunWorktreeMap[event.id] = worktree
            }

            // Track step/prompt name and start time for results
            if let step = event.step {
                stepRunStepMap[event.id] = step
            }
            stepRunStartMap[event.id] = Date()

            // Capture baseline for results computation
            if let worktree = event.worktree {
                Task {
                    let baseline = await resultsService.captureBaseline(
                        stepRunId: event.id,
                        worktree: URL(fileURLWithPath: worktree)
                    )
                    await MainActor.run {
                        stepRunBaselines[event.id] = baseline
                        // Create running result entry
                        stepRunResults[event.id] = StepRunResult.running(
                            stepRunId: event.id,
                            step: event.step ?? "step",
                            worktree: worktree,
                            startedAt: stepRunStartMap[event.id] ?? Date()
                        )
                    }
                }
            }
        } else if event.status == "completed" || event.status == "error" {
            activeSessionIds.remove(event.id)
            // Remove worktree from active set
            if let worktree = stepRunWorktreeMap.removeValue(forKey: event.id) {
                // Only remove if no other sessions running in same worktree
                let otherSessionsInWorktree = stepRunWorktreeMap.values.contains(worktree)
                if !otherSessionsInWorktree {
                    activeWorktreePaths.remove(worktree)
                }
            }

            // Compute results
            if let baseline = stepRunBaselines[event.id] {
                let step = stepRunStepMap[event.id] ?? "step"
                let startedAt = stepRunStartMap[event.id] ?? Date()
                let status: StepRunResultStatus = event.status == "completed" ? .completed : .error

                Task {
                    let result = await resultsService.computeResults(
                        baseline: baseline,
                        step: step,
                        status: status,
                        startedAt: startedAt,
                        endedAt: Date()
                    )
                    await MainActor.run {
                        stepRunResults[event.id] = result
                    }
                }
            }

            // Clean up tracking maps
            stepRunStepMap.removeValue(forKey: event.id)
            stepRunStartMap.removeValue(forKey: event.id)
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

    func loadDiffPreview(for stepRunId: String, fileIndex: Int) async {
        guard var result = stepRunResults[stepRunId],
              fileIndex < result.filesChanged.count,
              result.filesChanged[fileIndex].diffPreview == nil,
              let baseline = stepRunBaselines[stepRunId] else { return }

        let file = result.filesChanged[fileIndex]
        let worktree = URL(fileURLWithPath: result.worktree)

        let preview = await resultsService.loadDiffPreview(
            for: file,
            baselineSHA: baseline.headSHA,
            in: worktree
        )

        result.filesChanged[fileIndex].diffPreview = preview
        stepRunResults[stepRunId] = result
    }

    func clearCompletedResults() {
        // Clear results for step runs that are complete (keep running ones)
        for stepRunId in stepRunResults.keys {
            if let result = stepRunResults[stepRunId], result.status != .running {
                stepRunResults.removeValue(forKey: stepRunId)
                stepRunBaselines.removeValue(forKey: stepRunId)
                liveOutputBySession.removeValue(forKey: stepRunId)
            }
        }
    }

    var currentStepRunResult: StepRunResult? {
        // Filter by selected worktree if one is selected
        let results = stepRunResults.values
        let filtered: [StepRunResult]

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
        guard let repo = currentRepo else { return }
        do {
            agents = try await agentService.listAgents(repo: repo)
            // Update selected agent if it exists in the refreshed list
            if let selected = selectedAgent,
               let updated = agents.first(where: { $0.id == selected.id }) {
                selectedAgent = updated
            }
        } catch {
            agents = []
        }
    }

    func createAgent(name: String) async throws {
        guard let repo = currentRepo else { return }

        // Generate name if empty
        let agentName = name.isEmpty ? NameGenerator.generate() : name

        // Create agent via lfd - starts with defaults, user configures later
        let agent = try await agentService.createAgent(name: agentName, repo: repo)

        // Add to list and select
        agents.insert(agent, at: 0)
        selectedAgent = agent
    }

    func runAgent(
        agent: Agent,
        area: [String]? = nil,
        goal: [String]? = nil,
        flow: String? = nil,
        stimulus: Stimulus? = nil
    ) async throws {
        try await agentService.run(
            agentId: agent.id,
            area: area,
            goal: goal,
            flow: flow,
            stimulus: stimulus
        )

        await refreshAgents()
    }

    func stopAgent(_ agent: Agent) async throws {
        try await agentService.stopAgent(agentId: agent.id)
        await refreshAgents()
    }

    func cloneAgent(_ agent: Agent) async throws -> Agent {
        let cloned = try await agentService.cloneAgent(agentId: agent.id)
        agents.insert(cloned, at: 0)
        selectedAgent = cloned
        return cloned
    }

    func updateAgent(
        _ agent: Agent,
        area: [String]? = nil,
        goal: [String]? = nil,
        flow: String? = nil,
        stimulus: Stimulus? = nil,
        paused: Bool? = nil
    ) async throws {
        _ = try await agentService.updateAgent(
            agentId: agent.id,
            area: area,
            goal: goal,
            flow: flow,
            stimulus: stimulus,
            paused: paused
        )
        await refreshAgents()
    }

    func liveOutput(for agent: Agent) -> [OutputLine] {
        // Agent runs use their ID as session ID for output tracking
        return liveOutputBySession[agent.id] ?? []
    }

    func connectLfd() async throws {
        // Try to install/start daemon if not running, but don't fail if it errors
        try? await agentService.connectLfd()
        // Start event subscription (has its own reconnect loop)
        startEventSubscription()
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

    func buildCommand(flow: Flow? = nil) -> String {
        var parts = ["lf"]

        // If running a flow, just use the flow name
        if let flow = flow {
            parts.append(flow.name)
            // Flows don't take args or many of the flags
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

        // Goals
        if !selectedGoals.isEmpty {
            let goalNames = selectedGoals.map { $0.name }.joined(separator: ",")
            parts.append("--goal")
            parts.append(goalNames)
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
