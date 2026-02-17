// Primary data state - waves, flows, and lfd connection.

import Foundation
import SwiftUI
import LoopflowCore

@MainActor
@Observable
final class RepoState {
    enum UITestMode: String {
        case emptyWorkspaces = "empty-workspaces"
        case sampleWorkspaces = "sample-workspaces"
        case mockWaves = "mock-waves"
    }

    struct ScreenshotMode {
        let outputPath: String
        let repoPath: String?
        let windowSize: (Int, Int)?
        let selectBranch: String?
        let mockLoops: Bool
        let mockConfig: String?
        let selectTab: String?

        static func fromArgs() -> ScreenshotMode? {
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

    var currentRepo: URL?
    var flows: [Flow] = []
    var availableDirections: [String] = []
    var waveSchemas: [WaveSchema] = []

    // Wave state — delegated to WaveStore
    let waveStore = WaveStore()
    let runStore = RunStore()
    let worktreeStore = WorktreeStore()

    var waves: [WaveViewModel] { waveStore.ordered }
    var waveGroups: WaveGroups { waveStore.groups }

    // Selection — ID-based, derived from store
    var selectedWaveId: String?

    var selectedWave: WaveViewModel? {
        get { selectedWaveId.flatMap { waveStore.wave(for: $0) } }
        set { selectedWaveId = newValue?.id }
    }

    // In-flight actions (land) — buttons disable while pending
    private(set) var inFlightActions: Set<String> = []

    func isActionInFlight(_ waveId: String) -> Bool {
        inFlightActions.contains(waveId)
    }

    // Loading
    var isLoading: Bool = false
    var errorMessage: String?

    // Daemon connection
    var lfdConnected: Bool = false

    // Services
    private let waveService = LocalWaveService()
    private var eventService: LocalEventService?
    private weak var outputBuffer: OutputBuffer?

    init() {
        waveStore.onStatusChange = { [weak self] wave, oldStatus, newStatus in
            self?.handleWaveStatusChange(wave: wave, from: oldStatus, to: newStatus)
        }
    }

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
        flows = []
        waveStore.removeAll()
        worktreeStore.removeAll()
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

    func configureMockWaves() {
        lfdConnected = true
        let repo = currentRepo?.path ?? "/tmp/demo"
        waveStore.setAll([
            WaveViewModel(
                api: Wave(
                    id: "mock-wave-1",
                    name: "swift-falcon",
                    repo: repo,
                    flow: "ship",
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
                    flow: "ship",
                    direction: ["product-engineer"],
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
                    flow: "ship",
                    direction: ["product-engineer"],
                    area: ["src/deploy"],
                    stimuli: [],
                    status: .failed,
                    iteration: 2,
                    activeRun: WaveRun(
                        id: "run-failed-1",
                        waveId: "mock-wave-5",
                        flow: "ship",
                        area: "src/deploy",
                        repo: repo,
                        direction: ["product-engineer"],
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
                flow: "ship",
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
                flow: "ship",
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
                flow: "ship",
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

    func configureMockWavesEmpty() {
        lfdConnected = true
        waveStore.removeAll()
    }

    func openRepo(_ url: URL, outputBuffer: OutputBuffer, skipBackgroundRefresh: Bool = false) async {
        let startTime = CFAbsoluteTimeGetCurrent()
        currentRepo = url
        isLoading = true
        errorMessage = nil
        isLoading = false
        LoggingService.append("openRepo.total elapsed=\(Int((CFAbsoluteTimeGetCurrent() - startTime) * 1000))ms")

        // Background operations (skip in screenshot mode to avoid overwriting mock data)
        if !skipBackgroundRefresh {
            Task {
                let setupService = SetupService()
                try? await setupService.ensureDaemonRunning()
                startEventSubscription(outputBuffer: outputBuffer)
                await refreshFlowsAsync()
            }
        }
    }

    // MARK: - Event Subscription

    func startEventSubscription(outputBuffer: OutputBuffer) {
        LoggingService.append("startEventSubscription called", category: LoggingService.Category.lfd)
        self.outputBuffer = outputBuffer
        if eventService != nil { return }
        LoggingService.append("creating LocalEventService", category: LoggingService.Category.lfd)
        eventService = LocalEventService()

        Task {
            await eventService?.subscribe(
                onEvent: { [weak self, weak outputBuffer] event in
                    Task { @MainActor in
                        guard let self, let outputBuffer else { return }
                        switch event {
                        case .connected(let connected):
                            self.lfdConnected = true
                            self.waveStore.setAll(connected.waves.map { WaveViewModel(api: $0) })
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
                        case .agentStarted, .agentEnded, .worktree:
                            break
                        }
                    }
                },
                onConnectionChange: { [weak self] connected in
                    Task { @MainActor in
                        LoggingService.lfd("onConnectionChange: connected=\(connected)")
                        self?.lfdConnected = connected
                    }
                }
            )
        }
    }

    private func handleWaveEvent(_ event: WaveEvent) async {
        switch event.type {
        case .created, .updated, .started, .stopped, .waiting:
            if let wave = event.wave {
                waveStore.set(WaveViewModel(api: wave))
            } else if let wave = try? await waveService.getWave(event.waveId) {
                waveStore.set(WaveViewModel(api: wave))
            }
            // Update runs cache on run lifecycle events
            if event.type == .started || event.type == .stopped || event.type == .updated {
                loadRuns(for: event.waveId)
            }
            // New wave may adopt a worktree
            if event.type == .created {
                await refreshWorktrees()
            }
        case .deleted:
            waveStore.remove(event.waveId)
            runStore.clear(for: event.waveId)
            if selectedWaveId == event.waveId {
                selectedWaveId = nil
            }
            // Deleted wave may orphan a worktree
            await refreshWorktrees()
        }
    }

    // MARK: - Flows

    func refreshFlowsAsync() async {
        guard let repo = currentRepo else { return }
        guard let result = try? await waveService.listFlowsAndDirections(repo: repo) else { return }
        if !result.flows.isEmpty {
            flows = result.flows
        }
        availableDirections = result.directions
        waveSchemas = (try? await waveService.listWaveSchemas(repo: repo)) ?? []
    }

    // MARK: - Waves

    func refreshWaves() async {
        guard let repo = currentRepo else {
            LoggingService.model("refreshWaves: no currentRepo")
            return
        }
        LoggingService.model("refreshWaves: starting for repo=\(repo.path)")
        do {
            let newWaves = try await waveService.listWaves(repo: repo)
            LoggingService.model("refreshWaves: got \(newWaves.count) waves")
            waveStore.setAll(newWaves.map { WaveViewModel(api: $0) })
        } catch {
            LoggingService.model("refreshWaves: error=\(error.localizedDescription)")
            waveStore.removeAll()
        }
    }

    func loadRuns(for waveId: String) {
        Task {
            guard let runs = try? await waveService.listWaveRuns(waveId: waveId) else { return }
            runStore.setRuns(for: waveId, runs)
        }
    }

    func refreshWorktrees() async {
        guard let repo = currentRepo else { return }
        worktreeStore.setAll((try? await waveService.listWorktrees(repo: repo)) ?? [])
    }

    func combinePRs(_ waveId: String) async throws -> CombinePRsResult {
        let result = try await waveService.combinePRs(waveId)
        loadRuns(for: waveId)
        return result
    }

    private func handleWaveStatusChange(wave: WaveViewModel, from oldStatus: WaveStatus?, to newStatus: WaveStatus) {
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
    func createWave(name: String, schemaRef: String? = nil) async throws -> Wave {
        guard let repo = currentRepo else {
            LoggingService.model("createWave: no currentRepo")
            throw WaveServiceError.commandFailed("No repository selected")
        }

        let normalizedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let waveName: String
        if normalizedName.isEmpty && schemaRef == nil {
            waveName = NameGenerator.generate()
        } else {
            waveName = normalizedName
        }
        LoggingService.model("createWave: name=\(waveName) repo=\(repo.path)")

        let pendingId = "pending-\(UUID().uuidString)"
        let pending = WaveViewModel(
            api: Wave(id: pendingId, name: waveName.isEmpty ? "New wave" : waveName, repo: repo.path)
        )
        waveStore.insertPending(pending)
        selectedWaveId = pendingId

        do {
            let wave = try await waveService.createWave(name: waveName, repo: repo, schema: schemaRef)
            waveStore.replacePending(pendingId, with: WaveViewModel(api: wave))
            selectedWaveId = wave.id
            await refreshFlowsAsync()
            LoggingService.model("createWave: selectedWave=\(wave.id)")
            return wave
        } catch {
            waveStore.removePending(pendingId)
            if selectedWaveId == pendingId { selectedWaveId = nil }
            throw error
        }
    }

    func instantiateSchema(_ schema: WaveSchema, startImmediately: Bool = true) async throws {
        let wave = try await createWave(name: schema.name, schemaRef: schema.schemaRef)
        if startImmediately {
            try await waveService.run(wave.id, overrides: nil)
            _ = waveStore.applyOptimistic(wave.id) { $0.status = .running }
            waveStore.commitMutation(wave.id)
            scheduleRefresh(for: wave.id)
        }
    }

    func runWave(
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

    func addStimulus(wave: WaveViewModel, kind: Stimulus.Kind, cron: String? = nil) async throws {
        let stimulus = try await waveService.addStimulus(wave.id, kind: kind, cron: cron)
        _ = waveStore.applyOptimistic(wave.id) { $0.stimuli.append(stimulus) }
        waveStore.commitMutation(wave.id)
    }

    func removeStimulus(wave: WaveViewModel, stimulusId: String) async throws {
        try await waveService.removeStimulus(wave.id, stimulusId: stimulusId)
        _ = waveStore.applyOptimistic(wave.id) { $0.stimuli.removeAll { $0.id == stimulusId } }
        waveStore.commitMutation(wave.id)
    }

    func stopWave(_ wave: WaveViewModel) async throws {
        try await optimisticAction(wave.id, mutation: { $0.status = .idle }) {
            try await self.waveService.stop(wave.id)
        }
    }

    func restartStep(_ wave: WaveViewModel) async throws {
        try await optimisticAction(wave.id, mutation: { _ in }) {
            try await self.waveService.restartStep(wave.id)
        }
    }

    func cloneWave(_ wave: WaveViewModel) async throws -> WaveViewModel {
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

    func deleteWave(_ wave: WaveViewModel) async throws {
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

    func renameWave(_ wave: WaveViewModel, to newName: String) async throws {
        try await optimistic(wave.id, mutation: { $0.name = newName }) {
            _ = try await self.waveService.updateWave(wave.id, config: WaveConfigUpdate(name: newName))
        }
    }

    func updateWave(
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

    func landWave(_ wave: WaveViewModel) async throws {
        inFlightActions.insert(wave.id)
        defer { inFlightActions.remove(wave.id) }
        try await waveService.landWave(wave.id)
    }

    func nextWave(_ wave: WaveViewModel) async throws {
        try await optimisticAction(wave.id, mutation: { $0.status = .idle }) {
            _ = try await self.waveService.nextWave(wave.id)
        }
    }

    func connectLfd(outputBuffer: OutputBuffer) async throws {
        LoggingService.lfd("connectLfd: starting")
        try? await waveService.connectLfd()
        LoggingService.lfd("connectLfd: waveService.connectLfd completed")
        startEventSubscription(outputBuffer: outputBuffer)
        LoggingService.lfd("connectLfd: event subscription started")
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
                waveStore.set(WaveViewModel(api: wave))
            }
        }
    }
}
