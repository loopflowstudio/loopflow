// Primary data state - waves, flows, and lfd connection.

import Foundation
import SwiftUI

@MainActor
@Observable
public final class RepoState {
    enum WaveCreationReadinessError: LocalizedError {
        case missingRepo

        var errorDescription: String? {
            switch self {
            case .missingRepo:
                "Select a repository in Connection Settings first."
            }
        }
    }

    public enum UITestMode: String {
        case emptyWorkspaces = "empty-workspaces"
        case sampleWorkspaces = "sample-workspaces"
        case mockWaves = "mock-waves"
    }

    public struct ScreenshotMode {
        public let outputPath: String
        public let repoPath: String?
        public let windowSize: (Int, Int)?
        public let selectBranch: String?
        public let mockLoops: Bool
        public let mockConfig: String?
        public let selectTab: String?

        public init(
            outputPath: String,
            repoPath: String?,
            windowSize: (Int, Int)?,
            selectBranch: String?,
            mockLoops: Bool,
            mockConfig: String?,
            selectTab: String?
        ) {
            self.outputPath = outputPath
            self.repoPath = repoPath
            self.windowSize = windowSize
            self.selectBranch = selectBranch
            self.mockLoops = mockLoops
            self.mockConfig = mockConfig
            self.selectTab = selectTab
        }

        public static func fromArgs() -> ScreenshotMode? {
            let args = ProcessInfo.processInfo.arguments

            func arg(_ flag: String) -> String? {
                guard let i = args.firstIndex(of: flag), args.count > i + 1 else { return nil }
                return args[i + 1]
            }

            guard let outputPath = arg("--snapshot") else { return nil }

            var windowSize: (Int, Int)?
            if let sizeStr = arg("--size") {
                let parts = sizeStr.split(separator: "x")
                if parts.count == 2, let w = Int(parts[0]), let h = Int(parts[1]) {
                    windowSize = (w, h)
                }
            }

            return ScreenshotMode(
                outputPath: outputPath,
                repoPath: arg("--repo"),
                windowSize: windowSize,
                selectBranch: arg("--select"),
                mockLoops: args.contains("--mock-loops"),
                mockConfig: arg("--mock-config"),
                selectTab: arg("--tab")
            )
        }
    }

    public var currentRepo: URL? {
        didSet {
            terminalWorkspaceStore.configure(repoKey: currentRepo?.path())
            multiplexerStore.configure(repoKey: currentRepo?.path())
        }
    }
    public var repoTarget: RepoTarget?
    public var flows: [Flow] = []
    public var availableDirections: [String] = []

    // Wave state — delegated to WaveStore
    public let waveStore = WaveStore()
    public let attentionStore = AttentionStore()
    public let runStore = RunStore()
    public let worktreeStore = WorktreeStore()
    public let terminalWorkspaceStore = TerminalWorkspaceStore()
    public let multiplexerStore = MultiplexerStore()
    public let authProviderStore = AuthProviderStore()
    private var sessionStates: [String: SessionState] = [:]
    private var waitingSessionIds: [String: String] = [:]
    private var optimisticInteractiveWaveIds: Set<String> = []
    private var autoPresentTerminalWaveIds: Set<String> = []

    public var waves: [WaveViewModel] { waveStore.ordered }
    public var waveGroups: WaveGroups { waveStore.groups }

    // Selection — ID-based, derived from store
    public var selectedWaveId: String? {
        didSet {
            if selectedWaveId != nil {
                showingFlows = false
            }
        }
    }

    public var showingFlows = false

    /// Fetch the flow + step catalog for the currently selected repo.
    public func fetchCatalog() async throws -> Catalog {
        let repo = currentRepo?.path
        return try await waveService.fetchCatalog(repo: repo)
    }

    public var selectedWave: WaveViewModel? {
        get { selectedWaveId.flatMap { waveStore.wave(for: $0) } }
        set { selectedWaveId = newValue?.id }
    }

    public func sessionState(for waveId: String, joinSessionId: String? = nil) -> SessionState {
        if let state = sessionStates[waveId] {
            if let joinSessionId {
                state.joinSession(joinSessionId)
            }
            return state
        }
        let repoRoot = currentRepo?.path() ?? FileManager.default.currentDirectoryPath
        let wave = waveStore.wave(for: waveId)
        let sessionStep = wave?.activeRun?.currentStep ?? "design"
        let state = SessionState(
            waveId: waveId,
            sessionConfig: AgentSessionConfig(
                step: sessionStep,
                repoRoot: repoRoot,
                directions: wave?.direction ?? [],
                area: wave?.area.first,
                wave: {
                    guard let name = wave?.name, !name.isEmpty else { return nil }
                    return name
                }(),
                clientHasUI: true
            ),
            waveService: waveService
        )
        if let joinSessionId {
            state.joinSession(joinSessionId)
        }
        sessionStates[waveId] = state
        return state
    }

    public func interactiveSessionId(for waveId: String) -> String? {
        if let sessionId = waitingSessionIds[waveId] {
            return sessionId
        }
        guard waveStore.wave(for: waveId)?.status == .waiting else {
            return nil
        }
        return UserDefaults.standard.string(forKey: "session.\(waveId)")
    }

    public func isOptimisticallyStartingInteractiveSession(for waveId: String) -> Bool {
        optimisticInteractiveWaveIds.contains(waveId)
    }

    public func shouldShowInteractiveSession(for wave: WaveViewModel) -> Bool {
        isOptimisticallyStartingInteractiveSession(for: wave.id)
            || interactiveSessionId(for: wave.id) != nil
            || wave.status == .waiting
    }

    func setOptimisticInteractiveSessionStart(for waveId: String, isStarting: Bool) {
        if isStarting {
            optimisticInteractiveWaveIds.insert(waveId)
            sessionState(for: waveId).setAwaitingSession(true)
            return
        }
        optimisticInteractiveWaveIds.remove(waveId)
        sessionStates[waveId]?.setAwaitingSession(false)
    }

    public func markAutoPresentTerminal(for waveId: String) {
        autoPresentTerminalWaveIds.insert(waveId)
    }

    public func consumeAutoPresentTerminal(for waveId: String) -> Bool {
        autoPresentTerminalWaveIds.remove(waveId) != nil
    }

    private func resetTransientWaveState(includeSessionStates: Bool = true) {
        if includeSessionStates {
            sessionStates.removeAll()
        }
        waitingSessionIds.removeAll()
        attentionStore.removeAll()
        terminalWorkspaceStore.setAll([])
        optimisticInteractiveWaveIds.removeAll()
        autoPresentTerminalWaveIds.removeAll()
    }

    // In-flight actions (land) — buttons disable while pending
    private(set) var inFlightActions: Set<String> = []

    public func isActionInFlight(_ waveId: String) -> Bool {
        inFlightActions.contains(waveId)
    }

    // Loading
    public var isLoading: Bool = false
    public var errorMessage: String?

    // Connection
    public let connectionStore = ConnectionStore()
    public var connectionState: ConnectionState = .disconnected(nil)
    public var lfdConnected: Bool = false
    public var hasCompletedInitialLoad: Bool = false
    public var availableRemoteRepos: [RemoteRepo] = []

    // Services
    private let startBundledDaemon: (@MainActor () async throws -> ServerConnection)?
    private let shellCommandRunner: WaveService.ShellCommandRunner?
    private var waveService: WaveService
    /// Local discovery via `lf ls` (see `RegistryQuery`). Injected on platforms
    /// that can shell `lf`; `nil` means the registry is unavailable in this UI.
    public var registryQuery: RegistryQuery?
    private var snapshotTask: Task<Void, Never>?
    private weak var outputBuffer: OutputBuffer?

    public init(
        startBundledDaemon: (@MainActor () async throws -> ServerConnection)? = nil,
        shellCommandRunner: WaveService.ShellCommandRunner? = nil,
        registryQuery: RegistryQuery? = nil
    ) {
        self.startBundledDaemon = startBundledDaemon
        self.shellCommandRunner = shellCommandRunner
        self.registryQuery = registryQuery
        let connection = connectionStore.activeConnection
        waveService = Self.makeWaveService(
            connection: connection,
            token: connectionStore.token(for: connection),
            shellCommandRunner: shellCommandRunner
        )
        authProviderStore.bindService(waveService)

        waveStore.onStatusChange = { [weak self] wave, oldStatus, newStatus in
            self?.handleWaveStatusChange(wave: wave, from: oldStatus, to: newStatus)
        }
    }

    public static func uiTestMode() -> UITestMode? {
        let args = ProcessInfo.processInfo.arguments
        if let index = args.firstIndex(of: "-ui-test-mode"), args.count > index + 1 {
            return UITestMode(rawValue: args[index + 1])
        }
        if let mode = ProcessInfo.processInfo.environment["CONCERTO_UI_TEST_MODE"] {
            return UITestMode(rawValue: mode)
        }
        return nil
    }

    public func configureForUITest(_ mode: UITestMode, repoURL: URL) {
        currentRepo = repoURL
        repoTarget = .local(repoURL)
        flows = []
        waveStore.removeAll()
        worktreeStore.removeAll()
        resetTransientWaveState()
        selectedWaveId = nil
        isLoading = false
        errorMessage = nil
        let selectBranch = ProcessInfo.processInfo.environment["CONCERTO_UI_TEST_SELECT_BRANCH"]

        switch mode {
        case .emptyWorkspaces:
            break
        case .sampleWorkspaces, .mockWaves:
            configureMockWaves()
            if mode == .mockWaves, let selectBranch {
                selectedWaveId = waves.first { $0.branch == selectBranch }?.id
            }
        }
    }

    public func configureMockWaves() {
        lfdConnected = true
        hasCompletedInitialLoad = true
        connectionState = .connected
        let repo = currentRepo?.path ?? "/tmp/demo"
        waveStore.setAll([
            WaveViewModel(
                api: Wave(
                    id: "mock-wave-1",
                    name: "swift-falcon",
                    repo: repo,
                    direction: [],
                    area: ["src/auth"],
                    triggers: [Trigger(signal: .repo, flow: "integrate")],
                    status: .running,
                    iteration: 3
                ),
                branch: "wave-auth-feature",
                prLimit: 5,
                mergeMode: .pr,
                pid: 12345
            ),
            WaveViewModel(
                api: Wave(
                    id: "mock-wave-2",
                    name: "crystal-melody",
                    repo: repo,
                    direction: ["clarity"],
                    area: ["src/api"],
                    triggers: [Trigger(signal: .repo, flow: "integrate")],
                    status: .waiting,
                    iteration: 5
                ),
                branch: "wave-api-refactor",
                prLimit: 3,
                mergeMode: .pr
            ),
            WaveViewModel(
                api: Wave(
                    id: "mock-wave-3",
                    name: "Quick fix",
                    repo: repo,
                    direction: [],
                    area: ["."],
                    triggers: [],
                    status: .idle,
                    iteration: 0
                ),
                prLimit: 5,
                mergeMode: .pr
            ),
            WaveViewModel(
                api: Wave(
                    id: "mock-wave-4",
                    name: "Nightly polish",
                    repo: repo,
                    direction: [],
                    area: ["."],
                    triggers: [Trigger(signal: .ciFailure, flow: "ci-fix")],
                    status: .idle,
                    iteration: 12
                ),
                prLimit: 5,
                mergeMode: .pr
            ),
            WaveViewModel(
                api: Wave(
                    id: "mock-wave-5",
                    name: "broken-deploy",
                    repo: repo,
                    direction: ["clarity"],
                    area: ["src/deploy"],
                    triggers: [],
                    status: .failed,
                    iteration: 2,
                    activeRun: Run(
                        id: "run-failed-1",
                        waveId: "mock-wave-5",
                        flow: "build",
                        area: "src/deploy",
                        repo: repo,
                        direction: ["clarity"],
                        status: .failed,
                        currentStep: "implement",
                        error: "Build failed: missing dependency 'libcrypto'",
                        createdAt: Date().addingTimeInterval(-3600)
                    )
                ),
                branch: "wave-deploy-fix",
                prLimit: 5,
                mergeMode: .pr
            )
        ])

        // Populate mock runs for waves with history
        let mockRuns: [Run] = [
            Run(
                id: "run-1",
                waveId: "mock-wave-1",
                flow: "build",
                area: "src/auth",
                repo: repo,
                direction: [],
                status: .completed,
                iteration: 1,
                branch: "wave-auth-feature",
                currentStep: "gate",
                pr: PullRequest(url: URL(string: "https://github.com/example/repo/pull/42")!, number: 42, state: .merged, title: "Add auth middleware"),
                startedAt: Date().addingTimeInterval(-7200),
                endedAt: Date().addingTimeInterval(-6600),
                createdAt: Date().addingTimeInterval(-7200)
            ),
            Run(
                id: "run-2",
                waveId: "mock-wave-1",
                flow: "build",
                area: "src/auth",
                repo: repo,
                direction: [],
                status: .completed,
                iteration: 2,
                branch: "wave-auth-feature",
                currentStep: "gate",
                pr: PullRequest(url: URL(string: "https://github.com/example/repo/pull/45")!, number: 45, state: .open, title: "Add OAuth token refresh"),
                startedAt: Date().addingTimeInterval(-3600),
                endedAt: Date().addingTimeInterval(-3000),
                createdAt: Date().addingTimeInterval(-3600)
            ),
            Run(
                id: "run-3",
                waveId: "mock-wave-1",
                flow: "build",
                area: "src/auth",
                repo: repo,
                direction: [],
                status: .running,
                iteration: 3,
                branch: "wave-auth-feature",
                currentStep: "implement",
                startedAt: Date().addingTimeInterval(-300),
                createdAt: Date().addingTimeInterval(-300)
            ),
        ]
        runStore.setRuns(for: "mock-wave-1", mockRuns)
    }

    public func configureMockWavesEmpty() {
        lfdConnected = true
        connectionState = .connected
        waveStore.removeAll()
        attentionStore.removeAll()
    }

    public func openRepo(_ url: URL, outputBuffer: OutputBuffer, skipBackgroundRefresh: Bool = false) async {
        let startTime = CFAbsoluteTimeGetCurrent()
        let canonicalURL = canonicalRepoURL(url)
        if currentRepo?.path() != canonicalURL.path() {
            cancelTrackedSessions()
        }

        resetTransientWaveState()
        hasCompletedInitialLoad = false
        currentRepo = canonicalURL
        repoTarget = .local(canonicalURL)
        errorMessage = nil
        LoggingService.append("openRepo.total elapsed=\(Int((CFAbsoluteTimeGetCurrent() - startTime) * 1000))ms")

        // Background operations (skip in screenshot mode to avoid overwriting mock data)
        if !skipBackgroundRefresh {
            Task {
                if connectionStore.mode == .remote, connectionStore.configuredRemoteConnection == nil {
                    return
                }
                do {
                    try await connectLfd(outputBuffer: outputBuffer)
                } catch {
                    errorMessage = "Failed to connect: \(error.localizedDescription)"
                }
            }
        }
    }

    public func closeRepo() {
        cancelTrackedSessions()
        snapshotTask?.cancel()
        snapshotTask = nil
        resetTransientWaveState()
    }

    // MARK: - Registry snapshot

    /// Load the wave registry and keep it fresh. There is no telemetry stream:
    /// discovery is a QUERY (`RegistryQuery`/`lf ls`) that this re-runs on a
    /// slow cadence, and a wave's live motion
    /// is its own per-wave SSE (`WaveChatConnection`), opened by the detail
    /// view. Replaces the deleted `/ws` connected snapshot + push.
    public func startEventSubscription(outputBuffer: OutputBuffer) {
        self.outputBuffer = outputBuffer
        guard snapshotTask == nil else { return }
        snapshotTask = Task { [weak self] in
            await self?.runSnapshotLoop()
        }
    }

    private func runSnapshotLoop() async {
        await refreshRegistrySnapshot()
        preloadWaveContent(for: waves)
        await refreshAttention()
        await refreshSessions()
        await refreshWorktrees()
        hasCompletedInitialLoad = true
        await refreshFlowsAsync()

        while !Task.isCancelled {
            try? await Task.sleep(for: .seconds(5))
            if Task.isCancelled { return }
            await refreshRegistrySnapshot()
            await refreshAttention()
        }
    }

    /// Re-read which waves exist for this repo. `applyConnectedSnapshot` scopes
    /// a machine-wide `lf ls` to this repo.
    private func refreshRegistrySnapshot() async {
        guard let repoTarget, case .local(let url) = repoTarget, let registryQuery else { return }
        do {
            let loaded = try await registryQuery.waves(repoPath: url.path)
            applyConnectedSnapshot(loaded)
            preloadWaveContent(for: waves)
            if let selectedWaveId {
                loadStatus(for: selectedWaveId)
            }
            updateConnectionState(.connected)
        } catch {
            LoggingService.model("refreshRegistrySnapshot: error=\(error.localizedDescription)")
        }
    }

    // MARK: - Flows

    public func refreshFlowsAsync() async {
        guard let repo = repoTarget else { return }
        guard let result = try? await waveService.listFlowsAndDirections(repo: repo) else {
            return
        }
        if !result.flows.isEmpty {
            flows = result.flows
        }
        availableDirections = result.directions
    }

    // MARK: - Waves

    public func refreshWaves() async {
        guard let repo = repoTarget, case .local(let url) = repo, let registryQuery else {
            LoggingService.model("refreshWaves: no repoTarget")
            return
        }
        LoggingService.model("refreshWaves: starting for repo=\(repo.path)")
        do {
            let newWaves = try await registryQuery.waves(repoPath: url.path)
            LoggingService.model("refreshWaves: got \(newWaves.count) waves")
            waveStore.setAll(newWaves.map(makeWaveViewModel))
            preloadWaveContent(for: waves)
            await refreshAttention()
            if let selectedWaveId {
                loadWaveContent(for: selectedWaveId)
            }
        } catch {
            LoggingService.model("refreshWaves: error=\(error.localizedDescription)")
            waveStore.removeAll()
            attentionStore.removeAll()
        }
    }


    private func roadmapWaveNames(in repoURL: URL) -> [String] {
        let waveRoot = repoURL.appendingPathComponent("wave", isDirectory: true)
        guard let entries = try? FileManager.default.contentsOfDirectory(
            at: waveRoot,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles]
        ) else {
            return []
        }

        return entries.compactMap { entry in
            guard let values = try? entry.resourceValues(forKeys: [.isDirectoryKey]),
                  values.isDirectory == true
            else {
                return nil
            }
            return entry.lastPathComponent
        }
        .sorted()
    }

    public func refreshAttention() async {
        guard registryQuery != nil else {
            attentionStore.removeAll()
            return
        }
        do {
            var items: [AttentionItem] = []
            for wave in waves {
                let status = try await statusSnapshot(for: wave.id)
                items.append(contentsOf: status.attention)
                runStore.setRuns(for: wave.id, status.runs)
            }
            attentionStore.setAll(items)
        } catch {
            LoggingService.model("refreshAttention: error=\(error.localizedDescription)")
        }
    }

    public func refreshSessions() async {
        guard let repo = repoTarget else {
            terminalWorkspaceStore.setAll([])
            return
        }
        do {
            let sessions = try await waveService.listSessions(repo: repo, activeOnly: true)
            terminalWorkspaceStore.setAll(sessions)
        } catch {
            LoggingService.model("refreshSessions: error=\(error.localizedDescription)")
        }
    }

    public func loadSession(id: String, select: Bool = false) async {
        do {
            let session: Session = try await waveService.getSession(id)
            terminalWorkspaceStore.upsert(session, select: select)
        } catch {
            LoggingService.model("loadSession: error=\(error.localizedDescription)")
        }
    }

    @discardableResult
    public func cancelSession(_ id: String) async throws -> Session {
        let session = try await waveService.cancelSession(id)
        terminalWorkspaceStore.upsert(session, select: false)
        return session
    }

    public func createSession(
        waveId: String,
        flow: String,
        worktree: String,
        agent: String
    ) async throws -> SessionLaunchResponse {
        let response = try await waveService.createSession(
            waveId: waveId,
            flow: flow,
            worktree: worktree,
            agent: agent
        )
        terminalWorkspaceStore.upsert(response.session, select: true)
        return response
    }

    public func attachSession(_ id: String) async throws -> SessionConnectionInfo {
        try await waveService.attachSession(id)
    }

    @discardableResult
    public func startSession(_ id: String) async throws -> Session {
        let session = try await waveService.startSession(id)
        terminalWorkspaceStore.upsert(session, select: false)
        return session
    }

    public func selectSession(_ id: String?, waveId: String? = nil) {
        focusSession(id, waveId: waveId)
    }

    /// Navigate to a terminal session from the attention queue.
    /// Selects the wave, switches to the terminal tab, and selects the session.
    public func openSession(_ sessionId: String) {
        focusSession(sessionId, autoPresent: true)
    }

    private func focusSession(_ sessionId: String?, waveId: String? = nil, autoPresent: Bool = false) {
        terminalWorkspaceStore.select(sessionId, waveId: waveId)
        guard let sessionId,
              let session = terminalWorkspaceStore.sessionsById[sessionId] else {
            return
        }
        selectedWaveId = session.waveId
        if autoPresent {
            markAutoPresentTerminal(for: session.waveId)
        }
        // A live run session repoints the wave's terminal pane at its tmux
        // session, so focusing it lands on the running flow.
        if session.runId != nil, !session.status.isTerminal {
            attachTerminalPane(to: session)
        }
        loadWaveContent(for: session.waveId)
        loadRuns(for: session.waveId)
    }

    public func loadRuns(for waveId: String) {
        loadStatus(for: waveId)
    }

    public func loadStatus(for waveId: String) {
        Task {
            guard let status = try? await statusSnapshot(for: waveId) else { return }
            runStore.setRuns(for: waveId, status.runs)
            attentionStore.setAll(status.attention)
        }
    }

    private func statusSnapshot(for waveId: String) async throws
        -> (runs: [Run], attention: [AttentionItem], mind: String?) {
        guard let registryQuery,
              let wave = waveStore.wave(for: waveId)
        else {
            return ([], [], nil)
        }
        let cwd = currentRepo?.path()
        return try await registryQuery.status(wave: wave.name, waveId: wave.id, cwd: cwd)
    }

    /// Point the wave's terminal pane at a run's live tmux session so opening
    /// the wave lands the user on the running flow, not the empty default
    /// `lf-<waveId>-<paneId>` session. Called when a run session is focused.
    func attachTerminalPane(to session: Session) {
        guard let pane = multiplexerStore.pane(ofType: .terminal, for: session.waveId) else {
            return
        }
        if pane.config.terminalSessionId != session.id {
            var config = pane.config
            config.terminalSessionId = session.id
            multiplexerStore.updatePaneConfig(pane.id, config: config, for: session.waveId)
        }
        markAutoPresentTerminal(for: session.waveId)
    }

    public func refreshWorktrees() async {
        guard let repo = repoTarget else { return }
        worktreeStore.setAll((try? await waveService.listWorktrees(repo: repo)) ?? [])
    }

    public func markAttentionViewed(_ id: String) async throws -> AttentionItem {
        let item = try await waveService.markAttentionViewed(id)
        attentionStore.set(item)
        return item
    }

    public func combinePRs(_ waveId: String) async throws -> CombinePRsResult {
        let result = try await waveService.combinePRs(waveId)
        loadRuns(for: waveId)
        return result
    }

    private func handleWaveStatusChange(wave: WaveViewModel, from oldStatus: WaveStatus?, to newStatus: WaveStatus) {
        // Note: loadWaveContent is driven by the snapshot loop / selection — not duplicated here.
        switch newStatus {
        case .waiting:
            let step = wave.recentSteps.first?.step ?? "step"
            NotificationService.shared.notifyNeedsInteractive(
                waveId: wave.id,
                waveName: wave.displayName,
                step: step
            )

        case .failed:
            let step = wave.recentSteps.first?.step ?? "unknown step"
            let message = "Error in \(step)"
            NotificationService.shared.notifyError(
                waveId: wave.id,
                waveName: wave.displayName,
                message: message
            )

        case .idle:
            guard oldStatus == .running, let prNumber = wave.prNumber, wave.prState == .open else {
                break
            }
            NotificationService.shared.notifyPRReady(
                waveId: wave.id,
                waveName: wave.displayName,
                prNumber: prNumber
            )

        case .running, .paused:
            break
        }
    }

    public func updateRoadmapPriority(
        wave: WaveViewModel,
        item: RoadmapItem,
        priority: RoadmapPriority
    ) throws {
        guard repoTarget?.isRemote != true else {
            throw WaveServiceError.commandFailed("Roadmap priority is only editable for local repositories.")
        }
        guard let filePath = item.filePath else {
            throw WaveServiceError.commandFailed("Roadmap item is missing its file path.")
        }

        let sourceURL = URL(fileURLWithPath: filePath)
        let destinationURL = sourceURL
            .deletingLastPathComponent()
            .appendingPathComponent("\(priority.filenamePrefix)-\(item.slug).md", isDirectory: false)

        if sourceURL.lastPathComponent == destinationURL.lastPathComponent {
            return
        }
        if FileManager.default.fileExists(atPath: destinationURL.path) {
            throw WaveServiceError.commandFailed("A roadmap item named \(destinationURL.lastPathComponent) already exists.")
        }

        try FileManager.default.moveItem(at: sourceURL, to: destinationURL)
        loadWaveContent(for: wave.id)
    }

    public func addTrigger(wave: WaveViewModel, signal: Trigger.Signal, flow: String? = nil) async throws {
        let trigger = try await waveService.addTrigger(wave.id, signal: signal, flow: flow)
        _ = waveStore.applyOptimistic(wave.id) { $0.triggers.append(trigger) }
        waveStore.commitMutation(wave.id)
    }

    public func removeTrigger(wave: WaveViewModel, triggerId: String) async throws {
        try await waveService.removeTrigger(wave.id, triggerId: triggerId)
        _ = waveStore.applyOptimistic(wave.id) { $0.triggers.removeAll { $0.id == triggerId } }
        waveStore.commitMutation(wave.id)
    }

    public func stopWave(_ wave: WaveViewModel) async throws {
        try await optimisticAction(wave.id, mutation: { $0.status = .idle }) {
            try await self.waveService.stop(wave.id)
        }
    }


    public func deleteWave(_ wave: WaveViewModel) async throws {
        waveStore.applyDelete(wave.id)
        if selectedWaveId == wave.id { selectedWaveId = nil }

        do {
            try await waveService.deleteWave(wave.id)
            waveStore.commitMutation(wave.id)
            runStore.clear(for: wave.id)
            outputBuffer?.clearOutput(for: wave.id)
            await refreshFlowsAsync()
        } catch {
            waveStore.rollback(wave)
            throw error
        }
    }

    public func updateWave(
        _ wave: WaveViewModel,
        area: [String]? = nil,
        direction: [String]? = nil,
        status: WaveStatus? = nil,
        agent: String? = nil,
        stepAgents: [String: String]? = nil
    ) async throws {
        try await optimistic(wave.id, mutation: { w in
            if let area { w.area = area }
            if let direction { w.direction = direction }
            if let status { w.status = status }
            if let agent { w.agent = agent.isEmpty ? nil : agent }
            if let stepAgents { w.stepAgents = stepAgents }
        }) {
            let config = WaveConfigUpdate(
                area: area,
                direction: direction,
                status: status,
                agent: agent,
                stepAgents: stepAgents
            )
            _ = try await self.waveService.updateWave(wave.id, config: config)
        }
    }

    public func landWave(_ wave: WaveViewModel) async throws {
        inFlightActions.insert(wave.id)
        defer { inFlightActions.remove(wave.id) }
        try await waveService.landWave(wave.id)
    }

    public func nextWave(_ wave: WaveViewModel) async throws {
        try await optimisticAction(wave.id, mutation: { $0.status = .idle }) {
            _ = try await self.waveService.nextWave(wave.id)
        }
    }

    public func connectLfd(outputBuffer: OutputBuffer) async throws {
        updateConnectionState(.connecting(.startingDaemon))
        let connection = try await resolveConnection()
        try await performConnectionHandshake(connection: connection, outputBuffer: outputBuffer)
    }

    public func connect(to connection: ServerConnection, outputBuffer: OutputBuffer) async throws {
        connectionStore.setRemoteConnection(connection)
        try await connectLfd(outputBuffer: outputBuffer)
    }

    public func connectBundled(outputBuffer: OutputBuffer) async throws {
        connectionStore.setMode(.bundled)
        try await connectLfd(outputBuffer: outputBuffer)
    }

    private func resolveConnection() async throws -> ServerConnection {
        switch connectionStore.mode {
        case .bundled:
            guard let startBundledDaemon else {
                throw WaveServiceError.commandFailed("Bundled daemon is not available on this platform.")
            }
            guard currentRepo != nil else {
                throw WaveCreationReadinessError.missingRepo
            }
            let bundledConnection = try await startBundledDaemon()
            connectionStore.setBundledRuntimeConnection(bundledConnection)
            return connectionStore.activeConnection
        case .remote:
            guard let connection = connectionStore.configuredRemoteConnection else {
                throw WaveServiceError.commandFailed("Configure a remote server in Connection Settings.")
            }
            connectionStore.setMode(.remote)
            return connection
        }
    }

    private func performConnectionHandshake(
        connection: ServerConnection,
        outputBuffer: OutputBuffer
    ) async throws {
        self.outputBuffer = outputBuffer
        rebuildServices(for: connection)

        if connectionStore.mode == .bundled {
            // Bundled daemon is localhost — skip TLS trust check and repo discovery.
            try await runConnectionPhase(.authCheck, connection: connection) {
                try await waveService.checkConnection()
            }

            availableRemoteRepos = []
            if let currentRepo {
                repoTarget = .local(currentRepo)
            }
        } else {
            for phase in [ConnectionPhase.tlsTrustCheck, .authCheck] {
                try await runConnectionPhase(phase, connection: connection) {
                    try await waveService.checkConnection()
                }
            }

            let repos = try await runConnectionPhase(.repoDiscovery, connection: connection) {
                try await waveService.listRepos()
            }

            availableRemoteRepos = repos
            if let remote = repos.first {
                selectRemoteRepo(path: remote.path)
            } else {
                repoTarget = nil
            }
        }

        resetTransientWaveState()

        startEventSubscription(outputBuffer: outputBuffer)
        updateConnectionState(.connected)
        await refreshWaves()
    }

    public func ensureReadyToCreateWave(outputBuffer: OutputBuffer) async throws {
        if !lfdConnected {
            try await connectLfd(outputBuffer: outputBuffer)
            try await Task.sleep(for: .milliseconds(500))
        }

        guard repoTarget != nil else {
            throw WaveCreationReadinessError.missingRepo
        }
    }

    public func selectRemoteRepo(path: String) {
        cancelTrackedSessions()
        resetTransientWaveState()
        let host = connectionStore.activeConnection.host
        connectionStore.setMode(.remote)
        repoTarget = .remote(path: path, host: host)
        currentRepo = URL(fileURLWithPath: path)
    }

    public func clearPinnedCertificate() {
        connectionStore.clearPinnedCertificate()
    }

    public func trustNewCertificate() {
        guard case .trustRequired(let requirement) = connectionState else { return }
        connectionStore.trustNewCertificate(requirement)
    }

    public var isRemoteTarget: Bool {
        repoTarget?.isRemote ?? false
    }

    public func checkConnectionHealth() async {
        if connectionState == .connected {
            do {
                try await waveService.checkConnection()
            } catch {
                updateConnectionState(.disconnected(.networkUnavailable))
            }
        }
    }

    public var isConnected: Bool {
        connectionState.isConnected
    }

    public var isActivelyConnecting: Bool {
        if case .connecting = connectionState { return true }
        if case .reconnecting = connectionState { return true }
        if lfdConnected && !hasCompletedInitialLoad { return true }
        return false
    }

    public var connectionSummary: String {
        switch connectionState {
        case .connected:
            return "Connected"
        case .connecting(let phase):
            switch phase {
            case .startingDaemon: return "Starting daemon…"
            case .tlsTrustCheck: return "Checking TLS trust…"
            case .authCheck: return "Authenticating…"
            case .repoDiscovery: return "Loading repos…"
            case .wsProbe: return "Probing event stream…"
            }
        case .reconnecting(let attempt, _, _):
            return "Reconnecting (attempt \(attempt))…"
        case .authFailed(let message):
            return message ?? "Authentication failed"
        case .trustRequired:
            return "Trust required"
        case .disconnected(let reason):
            switch reason {
            case .networkUnavailable:
                return "Network unavailable"
            case .timeout:
                return "Connection timed out"
            case .daemonTimeout:
                return "Agent timed out — check server logs"
            case .serverUnreachable:
                return "Server unreachable"
            case .serverError(let status):
                return "Server error (\(status))"
            case .wsClosed:
                return "Connection closed"
            case .authRejected:
                return "Authentication failed"
            case .trustMismatch:
                return "Certificate changed"
            case .unknown(let message):
                return message
            case nil:
                return "Disconnected"
            }
        }
    }

    private func rebuildServices(for connection: ServerConnection) {
        let token = connectionStore.token(for: connection)
        waveService = Self.makeWaveService(
            connection: connection,
            token: token,
            shellCommandRunner: shellCommandRunner
        )
        authProviderStore.bindService(waveService)
        outputBuffer?.configureConnection(connection, tokenProvider: { token })

        snapshotTask?.cancel()
        snapshotTask = nil
    }

    private func canonicalRepoURL(_ url: URL) -> URL {
        url.resolvingSymlinksInPath().standardizedFileURL
    }

    private func mapConnectionError(_ error: Error, connection: ServerConnection) -> ConnectionState {
        if let serviceError = error as? WaveServiceError {
            switch serviceError {
            case .authRejected(let message):
                return .authFailed(message)
            case .trustMismatch(let oldFingerprint, let newFingerprint):
                return .trustRequired(
                    .certificateChanged(
                        host: connection.host,
                        port: connection.port,
                        oldFingerprint: oldFingerprint,
                        newFingerprint: newFingerprint
                    )
                )
            case .daemonTimeout:
                return .disconnected(.daemonTimeout)
            case .timeout:
                return .disconnected(.timeout)
            case .networkUnavailable:
                return .disconnected(.networkUnavailable)
            case .serverUnreachable:
                return .disconnected(.serverUnreachable)
            case .serverError(let status, _):
                return .disconnected(.serverError(status: status))
            case .commandFailed(let message):
                return .disconnected(.unknown(message))
            }
        }

        if let urlError = error as? URLError {
            switch urlError.code {
            case .timedOut:
                return .disconnected(.timeout)
            case .notConnectedToInternet, .networkConnectionLost:
                return .disconnected(.networkUnavailable)
            case .cannotFindHost, .cannotConnectToHost, .dnsLookupFailed:
                return .disconnected(.serverUnreachable)
            default:
                return .disconnected(.unknown(urlError.localizedDescription))
            }
        }

        return .disconnected(.unknown(error.localizedDescription))
    }

    private func updateConnectionState(_ state: ConnectionState) {
        connectionState = state
        lfdConnected = state.isConnected
        Task {
            await authProviderStore.handleConnectionState(state)
        }
    }

    private func runConnectionPhase<T: Sendable>(
        _ phase: ConnectionPhase,
        connection: ServerConnection,
        operation: @Sendable () async throws -> T
    ) async throws -> T {
        updateConnectionState(.connecting(phase))
        do {
            return try await operation()
        } catch {
            let mappedState = mapConnectionError(error, connection: connection)
            updateConnectionState(mappedState)
            throw error
        }
    }

    private static func makeWaveService(
        connection: ServerConnection,
        token: String?,
        shellCommandRunner: WaveService.ShellCommandRunner?
    ) -> WaveService {
        WaveService(
            connection: connection,
            tokenProvider: { token },
            shellCommandRunner: shellCommandRunner
        )
    }

    private func cancelTrackedSessions() {
        let sessionIds = terminalWorkspaceStore.orderedSessions
            .filter { !$0.status.isTerminal }
            .map(\.id)
        guard !sessionIds.isEmpty else { return }

        let service = waveService
        Task {
            for sessionId in sessionIds {
                _ = try? await service.cancelSession(sessionId)
            }
        }
    }

    // MARK: - Optimistic helpers

    /// Apply optimistic mutation, run API call, commit on success or rollback on error.
    private func optimistic(
        _ id: String,
        mutation: (inout WaveViewModel) -> Void,
        apiCall: () async throws -> Void
    ) async throws {
        let snapshot = waveStore.applyOptimistic(id, mutation)
        do {
            try await apiCall()
            waveStore.commitMutation(id)
        } catch {
            if let snapshot { waveStore.rollback(snapshot) }
            throw error
        }
    }

    /// Like `optimistic`, but also schedules a safety-net refresh after commit.
    /// Used for actions (run/stop/next) where the real state lands on the next
    /// registry snapshot.
    private func optimisticAction(
        _ id: String,
        mutation: (inout WaveViewModel) -> Void,
        apiCall: () async throws -> Void
    ) async throws {
        try await optimistic(id, mutation: mutation, apiCall: apiCall)
        scheduleRefresh(for: id)
    }

    private func scheduleRefresh(for waveId: String, delay: TimeInterval = 10) {
        Task {
            try? await Task.sleep(for: .seconds(delay))
            guard waveStore.wave(for: waveId) != nil else { return }
            await refreshRegistrySnapshot()
        }
    }

    public func loadWaveContent(for waveId: String) {
        guard let repoRoot = currentRepo,
              let wave = waveStore.wave(for: waveId),
              repoTarget?.isRemote != true else {
            return
        }

        let content = WaveContentParser.parse(repoRoot: repoRoot, waveName: wave.name, branch: wave.branch)
        let plan = WavePlanParser.parse(repoRoot: repoRoot, waveName: wave.name)
        _ = waveStore.applyOptimistic(waveId) {
            $0.content = content
            $0.plan = plan
        }
        waveStore.commitMutation(waveId)
    }

    private func preloadWaveContent(for waves: [WaveViewModel]) {
        guard currentRepo != nil, repoTarget?.isRemote != true else {
            return
        }

        for wave in waves {
            loadWaveContent(for: wave.id)
        }
    }

    /// Apply a wave-registry snapshot, scoped to this window's repo. `lf ls`
    /// (and the bundled daemon's REST list) spans every repo on the machine;
    /// without this filter a window would swallow another repo's waves.
    func applyConnectedSnapshot(_ waves: [Wave]) {
        let currentRepoPath = repoTarget?.path.normalizedFilePath
        let scoped = waves.filter { wave in
            wave.repo.normalizedFilePath == currentRepoPath
        }
        waveStore.setAll(scoped.map(makeWaveViewModel))
    }

    private func makeWaveViewModel(api wave: Wave) -> WaveViewModel {
        WaveViewModel(
            api: wave,
            content: waveStore.wave(for: wave.id)?.content,
            plan: waveStore.wave(for: wave.id)?.plan
        )
    }
}

private extension ConnectionState {
    var isConnected: Bool {
        if case .connected = self {
            return true
        }
        return false
    }
}
