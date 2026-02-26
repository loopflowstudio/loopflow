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

    public var currentRepo: URL?
    public var repoTarget: RepoTarget?
    public var flows: [Flow] = []
    public var availableDirections: [String] = []

    // Wave state — delegated to WaveStore
    public let waveStore = WaveStore()
    public let runStore = RunStore()
    public let worktreeStore = WorktreeStore()
    public let authProviderStore = AuthProviderStore()
    private var sessionStates: [String: SessionState] = [:]
    private var waitingSessionIds: [String: String] = [:]
    private var optimisticInteractiveWaveIds: Set<String> = []

    public var waves: [WaveViewModel] { waveStore.ordered }
    public var waveGroups: WaveGroups { waveStore.groups }

    // Selection — ID-based, derived from store
    public var selectedWaveId: String?

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
        if isOptimisticallyStartingInteractiveSession(for: wave.id) {
            return true
        }
        if interactiveSessionId(for: wave.id) != nil {
            return true
        }
        return wave.status == .waiting
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
    public var availableRemoteRepos: [RemoteRepo] = []

    // Services
    private let startBundledDaemon: (@MainActor () async throws -> ServerConnection)?
    private let shellCommandRunner: WaveService.ShellCommandRunner?
    private var waveService: WaveService
    private var eventService: EventService?
    private weak var outputBuffer: OutputBuffer?

    public init(
        startBundledDaemon: (@MainActor () async throws -> ServerConnection)? = nil,
        shellCommandRunner: WaveService.ShellCommandRunner? = nil
    ) {
        self.startBundledDaemon = startBundledDaemon
        self.shellCommandRunner = shellCommandRunner
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
        sessionStates.removeAll()
        waitingSessionIds.removeAll()
        optimisticInteractiveWaveIds.removeAll()
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
        connectionState = .connected
        let repo = currentRepo?.path ?? "/tmp/demo"
        waveStore.setAll([
            WaveViewModel(
                api: Wave(
                    id: "mock-wave-1",
                    name: "swift-falcon",
                    repo: repo,
                    flow: "build",
                    direction: [],
                    area: ["src/auth"],
                    stimuli: [Stimulus(kind: .loop)],
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
                    flow: "build",
                    direction: ["clarity"],
                    area: ["src/api"],
                    stimuli: [Stimulus(kind: .loop)],
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
                    flow: "debug",
                    direction: [],
                    area: ["."],
                    stimuli: [],
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
                    flow: "polish",
                    direction: [],
                    area: ["."],
                    stimuli: [Stimulus(kind: .cron, cron: "0 9 * * *")],
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
                    flow: "build",
                    direction: ["clarity"],
                    area: ["src/deploy"],
                    stimuli: [],
                    status: .failed,
                    iteration: 2,
                    activeRun: WaveRun(
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
        let mockRuns: [WaveRun] = [
            WaveRun(
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
            WaveRun(
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
            WaveRun(
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
    }

    public func openRepo(_ url: URL, outputBuffer: OutputBuffer, skipBackgroundRefresh: Bool = false) async {
        let startTime = CFAbsoluteTimeGetCurrent()
        let canonicalURL = canonicalRepoURL(url)

        sessionStates.removeAll()
        waitingSessionIds.removeAll()
        optimisticInteractiveWaveIds.removeAll()
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
        Task {
            await eventService?.disconnect()
        }
        eventService = nil
        waitingSessionIds.removeAll()
        optimisticInteractiveWaveIds.removeAll()
    }

    // MARK: - Event Subscription

    public func startEventSubscription(outputBuffer: OutputBuffer) {
        LoggingService.append("startEventSubscription called", category: LoggingService.Category.lfd)
        self.outputBuffer = outputBuffer
        if eventService != nil { return }
        LoggingService.append("creating EventService", category: LoggingService.Category.lfd)
        let connection = connectionStore.activeConnection
        eventService = Self.makeEventService(
            connection: connection,
            token: connectionStore.token(for: connection)
        )

        Task {
            await eventService?.subscribe(
                onEvent: { [weak self, weak outputBuffer] event in
                    Task { @MainActor in
                        guard let self, let outputBuffer else { return }
                        switch event {
                        case .connected(let connected):
                            self.updateConnectionState(.connected)
                            self.waveStore.setAll(connected.waves.map(self.makeWaveViewModel))
                            await self.refreshWorktrees()
                            await self.refreshFlowsAsync()
                        case .wave(let waveEvent):
                            await self.handleWaveEvent(waveEvent)
                        case .output(let outputEvent):
                            outputBuffer.appendOutput(
                                waveId: outputEvent.waveId,
                                text: outputEvent.text,
                                timestamp: outputEvent.timestamp
                            )
                        case .auth(let authEvent):
                            self.authProviderStore.handleEvent(authEvent)
                        case .agentStarted, .agentEnded, .worktree:
                            break
                        }
                    }
                },
                onConnectionStateChange: { [weak self] state in
                    Task { @MainActor in
                        LoggingService.lfd("onConnectionStateChange: state=\(state)")
                        self?.updateConnectionState(state)
                    }
                }
            )
        }
    }

    private func handleWaveEvent(_ event: WaveEvent) async {
        switch event.type {
        case .created, .updated, .started, .stopped, .waiting:
            var refreshedWave: Wave?
            if let wave = event.wave {
                waveStore.set(makeWaveViewModel(api: wave))
                refreshedWave = wave
            } else if let wave = try? await waveService.getWave(event.waveId) {
                waveStore.set(makeWaveViewModel(api: wave))
                refreshedWave = wave
            }

            if event.type == .waiting {
                if let sessionId = event.sessionId {
                    waitingSessionIds[event.waveId] = sessionId
                    setOptimisticInteractiveSessionStart(for: event.waveId, isStarting: false)
                    let state = sessionState(for: event.waveId, joinSessionId: sessionId)
                    if let initialUserMessage = event.initialUserMessage {
                        state.seedInitialUserMessage(initialUserMessage)
                    }
                    state.setAwaitingSession(false)
                } else if let initialUserMessage = event.initialUserMessage {
                    let state = sessionState(for: event.waveId)
                    state.seedInitialUserMessage(initialUserMessage)
                }
            } else if refreshedWave?.status != .waiting {
                waitingSessionIds.removeValue(forKey: event.waveId)
                if refreshedWave?.status == .failed || refreshedWave?.status == .idle {
                    setOptimisticInteractiveSessionStart(for: event.waveId, isStarting: false)
                }
            }

            loadWaveContent(for: event.waveId)
            // Update runs cache on run lifecycle events
            if event.type == .started || event.type == .stopped || event.type == .updated {
                loadRuns(for: event.waveId)
            }
            // New wave may adopt a worktree
            if event.type == .created {
                await refreshWorktrees()
                await refreshFlowsAsync()
            }
        case .deleted:
            waveStore.remove(event.waveId)
            runStore.clear(for: event.waveId)
            waitingSessionIds.removeValue(forKey: event.waveId)
            setOptimisticInteractiveSessionStart(for: event.waveId, isStarting: false)
            if selectedWaveId == event.waveId {
                selectedWaveId = nil
            }
            // Deleted wave may orphan a worktree
            await refreshWorktrees()
            await refreshFlowsAsync()
        }
    }

    // MARK: - Flows

    public func refreshFlowsAsync() async {
        guard let repo = repoTarget else { return }
        guard let result = try? await waveService.listFlowsAndDirections(repo: repo) else { return }
        if !result.flows.isEmpty {
            flows = result.flows
        }
        availableDirections = result.directions
    }

    // MARK: - Waves

    public func refreshWaves() async {
        guard let repo = repoTarget else {
            LoggingService.model("refreshWaves: no repoTarget")
            return
        }
        LoggingService.model("refreshWaves: starting for repo=\(repo.path)")
        do {
            let newWaves = try await waveService.listWaves(repo: repo)
            LoggingService.model("refreshWaves: got \(newWaves.count) waves")
            waveStore.setAll(newWaves.map(makeWaveViewModel))
            if let selectedWaveId {
                loadWaveContent(for: selectedWaveId)
            }
        } catch {
            LoggingService.model("refreshWaves: error=\(error.localizedDescription)")
            waveStore.removeAll()
        }
    }

    public func loadRuns(for waveId: String) {
        Task {
            guard let runs = try? await waveService.listWaveRuns(waveId: waveId) else { return }
            runStore.setRuns(for: waveId, runs)
        }
    }

    public func refreshWorktrees() async {
        guard let repo = repoTarget else { return }
        worktreeStore.setAll((try? await waveService.listWorktrees(repo: repo)) ?? [])
    }

    public func combinePRs(_ waveId: String) async throws -> CombinePRsResult {
        let result = try await waveService.combinePRs(waveId)
        loadRuns(for: waveId)
        return result
    }

    private func handleWaveStatusChange(wave: WaveViewModel, from oldStatus: WaveStatus?, to newStatus: WaveStatus) {
        loadWaveContent(for: wave.id)

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

    @discardableResult
    public func createWave(name: String) async throws -> Wave {
        try await createWaveInternal(name: name, flow: "ship-roadmap", run: false)
    }

    @discardableResult
    public func createAndRunWave(name: String) async throws -> Wave {
        try await createWaveInternal(name: name, flow: "ship", run: true)
    }

    @discardableResult
    private func createWaveInternal(name: String, flow: String, run: Bool) async throws -> Wave {
        guard let repo = repoTarget else {
            LoggingService.model("createWave: no repoTarget")
            throw WaveServiceError.commandFailed("No repository selected")
        }

        let normalizedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let waveName = normalizedName.isEmpty ? NameGenerator.generate() : normalizedName
        LoggingService.model("createWave: name=\(waveName) repo=\(repo.path)")

        let pendingId = "pending-\(UUID().uuidString)"
        let pending = WaveViewModel(
            api: Wave(id: pendingId, name: waveName.isEmpty ? "New wave" : waveName, repo: repo.path)
        )
        waveStore.insertPending(pending)
        if run {
            setOptimisticInteractiveSessionStart(for: pendingId, isStarting: true)
        } else {
            selectedWaveId = pendingId
        }

        do {
            let wave = try await waveService.createWave(
                name: waveName,
                repo: repo,
                flow: flow,
                run: run
            )
            var createdWave = WaveViewModel(api: wave)
            if run {
                createdWave.status = .waiting
            }
            waveStore.replacePending(pendingId, with: createdWave)
            if run {
                setOptimisticInteractiveSessionStart(for: pendingId, isStarting: false)
                setOptimisticInteractiveSessionStart(for: wave.id, isStarting: true)
            }
            selectedWaveId = wave.id
            await refreshFlowsAsync()
            LoggingService.model("createWave: selectedWave=\(wave.id)")
            return wave
        } catch {
            waveStore.removePending(pendingId)
            setOptimisticInteractiveSessionStart(for: pendingId, isStarting: false)
            if selectedWaveId == pendingId { selectedWaveId = nil }
            throw error
        }
    }

    public func runWave(
        wave: WaveViewModel,
        area: [String]? = nil,
        direction: [String]? = nil,
        flow: String? = nil
    ) async throws {
        try await optimisticAction(wave.id, mutation: { $0.status = .running }) {
            let overrides = RunOverrides(area: area, direction: direction, flow: flow)
            try await self.waveService.run(wave.id, overrides: overrides)
        }
    }

    public func addStimulus(wave: WaveViewModel, kind: Stimulus.Kind, cron: String? = nil) async throws {
        let stimulus = try await waveService.addStimulus(wave.id, kind: kind, cron: cron)
        _ = waveStore.applyOptimistic(wave.id) { $0.stimuli.append(stimulus) }
        waveStore.commitMutation(wave.id)
    }

    public func removeStimulus(wave: WaveViewModel, stimulusId: String) async throws {
        try await waveService.removeStimulus(wave.id, stimulusId: stimulusId)
        _ = waveStore.applyOptimistic(wave.id) { $0.stimuli.removeAll { $0.id == stimulusId } }
        waveStore.commitMutation(wave.id)
    }

    public func stopWave(_ wave: WaveViewModel) async throws {
        try await optimisticAction(wave.id, mutation: { $0.status = .idle }) {
            try await self.waveService.stop(wave.id)
        }
    }

    public func restartStep(_ wave: WaveViewModel) async throws {
        try await optimisticAction(wave.id, mutation: { _ in }) {
            try await self.waveService.restartStep(wave.id)
        }
    }

    public func cloneWave(_ wave: WaveViewModel) async throws -> WaveViewModel {
        let pendingId = "pending-\(UUID().uuidString)"
        let pendingWave = Wave(
            id: pendingId,
            name: "\(wave.name) (copy)",
            repo: wave.api.repo,
            flow: wave.api.flow,
            direction: wave.api.direction,
            area: wave.api.area,
            stimuli: wave.api.stimuli
        )
        let pending = WaveViewModel(api: pendingWave)
        waveStore.insertPending(pending)
        selectedWaveId = pendingId

        do {
            let cloned = try await waveService.cloneWave(wave.id, name: nil)
            let viewModel = WaveViewModel(api: cloned)
            waveStore.replacePending(pendingId, with: viewModel)
            selectedWaveId = viewModel.id
            return viewModel
        } catch {
            waveStore.removePending(pendingId)
            if selectedWaveId == pendingId { selectedWaveId = nil }
            throw error
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

    public func renameWave(_ wave: WaveViewModel, to newName: String) async throws {
        try await optimistic(wave.id, mutation: { $0.name = newName }) {
            _ = try await self.waveService.updateWave(wave.id, config: WaveConfigUpdate(name: newName))
        }
    }

    public func updateWave(
        _ wave: WaveViewModel,
        area: [String]? = nil,
        direction: [String]? = nil,
        flow: String? = nil,
        status: WaveStatus? = nil
    ) async throws {
        try await optimistic(wave.id, mutation: { w in
            if let area { w.area = area }
            if let direction { w.direction = direction }
            if let flow { w.flow = flow }
            if let status { w.status = status }
        }) {
            let config = WaveConfigUpdate(area: area, direction: direction, flow: flow, status: status)
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

        for phase in [ConnectionPhase.tlsTrustCheck, .authCheck] {
            try await runConnectionPhase(phase, connection: connection) {
                try await waveService.checkConnection()
            }
        }

        let repos = try await runConnectionPhase(.repoDiscovery, connection: connection) {
            try await waveService.listRepos()
        }

        if connectionStore.mode == .bundled {
            availableRemoteRepos = []
            if let currentRepo {
                repoTarget = .local(currentRepo)
            }
        } else if let remote = repos.first {
            availableRemoteRepos = repos
            selectRemoteRepo(path: remote.path)
        } else {
            availableRemoteRepos = repos
            repoTarget = nil
        }
        sessionStates.removeAll()
        waitingSessionIds.removeAll()
        optimisticInteractiveWaveIds.removeAll()

        let probeService = Self.makeEventService(
            connection: connection,
            token: connectionStore.token(for: connection)
        )
        try await runConnectionPhase(.wsProbe, connection: connection) {
            try await probeService.probe()
        }

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
        sessionStates.removeAll()
        waitingSessionIds.removeAll()
        optimisticInteractiveWaveIds.removeAll()
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

    public var isConnected: Bool {
        connectionState.isConnected
    }

    public var connectionSummary: String {
        switch connectionState {
        case .connected:
            return "Connected"
        case .connecting(let phase):
            switch phase {
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

        Task {
            await eventService?.disconnect()
        }
        eventService = nil
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

    private static func makeEventService(connection: ServerConnection, token: String?) -> EventService {
        EventService(
            connection: connection,
            tokenProvider: { token }
        )
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
    /// Used for actions (run/stop/next) where the real state arrives via WebSocket.
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
            if let wave = try? await waveService.getWave(waveId) {
                waveStore.set(makeWaveViewModel(api: wave))
            }
        }
    }

    public func loadWaveContent(for waveId: String) {
        guard let repoRoot = currentRepo,
              let wave = waveStore.wave(for: waveId),
              repoTarget?.isRemote != true else {
            return
        }

        let content = WaveContentParser.parse(repoRoot: repoRoot, waveName: wave.name)
        _ = waveStore.applyOptimistic(waveId) { $0.content = content }
        waveStore.commitMutation(waveId)
    }

    private func makeWaveViewModel(api wave: Wave) -> WaveViewModel {
        WaveViewModel(
            api: wave,
            content: waveStore.wave(for: wave.id)?.content
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
