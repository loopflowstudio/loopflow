// Primary data state - agents, flows, goals, config, worktrees.

import Foundation
import SwiftUI
import LoopflowCore

@MainActor
@Observable
final class RepoState {
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

    // Selection
    var selectedAgent: Agent?
    var selectedFlow: Flow?

    // Loading
    var isLoading: Bool = false
    var errorMessage: String?
    var refreshMessage: String?
    var isRefreshingWorktrees: Bool = false

    // Daemon connection
    var lfdConnected: Bool = false

    // Computed: worktree for selected agent
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

    // Services
    private let worktreeService = WorktreeService()
    private let configLoader = ConfigLoader()
    private let promptService = PromptService()
    private let flowService = FlowService()
    private let agentService = AgentService()
    private let goalService = GoalService()
    private var eventService: LFDEventService?

    // Background tasks
    private var autoSyncTask: Task<Void, Never>?
    private var listDebounceTask: Task<Void, Never>?
    private var autoPruneInFlight: Bool = false

    // Keep reference to session state for event handling
    private weak var _sessionState: SessionState?

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

    func openRepo(_ url: URL, launcherState: LauncherState, sessionState: SessionState) async {
        let startTime = CFAbsoluteTimeGetCurrent()
        currentRepo = url
        isLoading = true
        errorMessage = nil
        _sessionState = sessionState

        do {
            let t0 = CFAbsoluteTimeGetCurrent()
            config = try configLoader.load(from: url)
            LoggingService.append("openRepo.config elapsed=\(Int((CFAbsoluteTimeGetCurrent() - t0) * 1000))ms")

            // Initialize launcher state from config
            launcherState.initializeFromConfig(config, repoURL: url, goals: goals)

            // Load prompts and goals
            let t2 = CFAbsoluteTimeGetCurrent()
            prompts = try promptService.loadPrompts(from: url, config: config)
            flows = flowService.loadFlows(from: url)
            refreshGoals()
            LoggingService.append("openRepo.prompts elapsed=\(Int((CFAbsoluteTimeGetCurrent() - t2) * 1000))ms")

            // Re-initialize launcher goals after goals are loaded
            if let goalNames = config?.goalNames, !goalNames.isEmpty {
                launcherState.selectedGoals = goals.filter { goalNames.contains($0.name) }
            }
        } catch {
            errorMessage = error.localizedDescription
        }

        isLoading = false
        LoggingService.append("openRepo.total elapsed=\(Int((CFAbsoluteTimeGetCurrent() - startTime) * 1000))ms")

        // Background operations
        Task {
            let setupService = SetupService()
            try? await setupService.ensureDaemonRunning()
            startEventSubscription(sessionState: sessionState)

            await refreshAgents()
            await refreshFlowsAsync()
            await launcherState.estimateTokens(repoURL: url)
        }

        startAutoSyncTimer()
    }

    // MARK: - Refresh Operations

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

    private func syncAndEnrich() async {
        guard let repo = currentRepo else { return }

        _ = try? await worktreeService.sync(in: repo)
        await _listWorktrees()

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

        for i in worktrees.indices {
            if let status = ciStatusMap[worktrees[i].branch] {
                worktrees[i].ciStatus = status
            }
        }
    }

    private func autoPruneCompletedWorktrees(_ stalenessMap: [String: Staleness], in repo: URL) async {
        if autoPruneInFlight { return }

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

        if candidates.isEmpty { return }

        autoPruneInFlight = true
        let branchesToPrune = Set(candidates.map(\.branch))

        worktrees.removeAll { branchesToPrune.contains($0.branch) }
        if let selected = selectedWorktree, branchesToPrune.contains(selected.branch) {
            selectedWorktree = nil
        }

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

    // MARK: - Worktree Operations

    func createWorktree(name: String, baseBranch: String? = nil) async throws {
        guard let repo = currentRepo else { return }
        try await worktreeService.create(name: name, in: repo, baseBranch: baseBranch)
        await _listWorktrees()
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

    // MARK: - Event Subscription

    func startEventSubscription(sessionState: SessionState) {
        LoggingService.append("startEventSubscription called", category: LoggingService.Category.lfd)
        if eventService != nil { return }
        LoggingService.append("creating LFDEventService", category: LoggingService.Category.lfd)
        eventService = LFDEventService()

        Task {
            await eventService?.subscribe(
                to: ["worktree.*", "session.*", "output.line", "agent.*"],
                onEvent: { [weak self, weak sessionState] event in
                    Task { @MainActor in
                        guard let self, let sessionState else { return }
                        switch event {
                        case .worktree(let worktreeEvent):
                            self.handleWorktreeEvent(worktreeEvent)
                        case .session(let sessionEvent):
                            self.handleSessionEvent(sessionEvent, sessionState: sessionState)
                        case .output(let outputEvent):
                            sessionState.appendOutput(
                                sessionId: outputEvent.sessionId,
                                text: outputEvent.text,
                                timestamp: outputEvent.timestamp
                            )
                        case .agent(_):
                            await self.refreshAgents()
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
        if event.name == "worktree.pruned", let branch = event.branch {
            worktrees.removeAll { $0.branch == branch }
            if selectedWorktree?.branch == branch {
                selectedWorktree = nil
            }
            return
        }

        if let updatedWorktree = event.worktree, let branch = event.branch {
            if let index = worktrees.firstIndex(where: { $0.branch == branch }) {
                worktrees[index] = updatedWorktree
                if selectedWorktree?.branch == branch {
                    selectedWorktree = updatedWorktree
                }
            } else {
                worktrees.append(updatedWorktree)
            }
            return
        }

        listWorktrees()
    }

    private func handleSessionEvent(_ event: SessionEvent, sessionState: SessionState) {
        if event.status == nil {
            // session.started
            sessionState.startSession(
                id: event.id,
                agentId: nil,
                step: event.step ?? "step",
                worktree: event.worktree ?? ""
            )
        } else if event.status == "completed" || event.status == "error" {
            sessionState.endSession(id: event.id, status: event.status ?? "completed")
        }
    }

    // MARK: - Goals & Flows

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

    // MARK: - Agents

    func refreshAgents() async {
        guard let repo = currentRepo else { return }
        do {
            agents = try await agentService.listAgents(repo: repo)
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

        let agentName = name.isEmpty ? NameGenerator.generate() : name
        let agent = try await agentService.createAgent(name: agentName, repo: repo)

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

    func connectLfd() async throws {
        try? await agentService.connectLfd()
        if let sessionState = _sessionState {
            startEventSubscription(sessionState: sessionState)
        }
    }
}
