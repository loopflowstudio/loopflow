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

        static func fromArgs() -> ScreenshotMode? {
            let args = ProcessInfo.processInfo.arguments
            guard let snapshotIndex = args.firstIndex(of: "--snapshot"),
                  args.count > snapshotIndex + 1 else {
                return nil
            }

            let outputPath = args[snapshotIndex + 1]
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
    var flows: [Flow] = []
    var availableDirections: [String] = []
    var waves: [WaveViewModel] = [] {
        didSet {
            waveGroups = buildWaveGroups(from: waves)
        }
    }

    // Selection
    var selectedWave: WaveViewModel?

    // Loading
    var isLoading: Bool = false
    var errorMessage: String?

    // Daemon connection
    var lfdConnected: Bool = false

    // Services
    private let waveService = LocalWaveService()
    private var eventService: LocalEventService?

    // Track previous wave statuses for notification transitions
    private var previousWaveStatuses: [String: WaveStatus] = [:]

    struct WaveGroups {
        let blocked: [WaveViewModel]
        let pr: [WaveViewModel]
        let recentActivity: [WaveViewModel]
        let active: [WaveViewModel]
        let idle: [WaveViewModel]

        var attentionCount: Int { blocked.count + pr.count }
        var allInOrder: [WaveViewModel] { blocked + pr + recentActivity + active + idle }
    }

    var waveGroups: WaveGroups = WaveGroups(
        blocked: [],
        pr: [],
        recentActivity: [],
        active: [],
        idle: []
    )

    private func buildWaveGroups(from waves: [WaveViewModel]) -> WaveGroups {
        let blocked = waves.filter { $0.status == .failed }
        let pr = waves.filter { wave in
            wave.status != .failed && pendingPR(for: wave) != nil
        }

        let hourAgo = Date().addingTimeInterval(-3600)
        let recentActivity = Array(waves
            .filter { wave in
                guard let lastActivity = wave.lastActivityAt else { return false }
                return lastActivity > hourAgo && wave.status != .failed && pendingPR(for: wave) == nil
            }
            .sorted { ($0.lastActivityAt ?? .distantPast) > ($1.lastActivityAt ?? .distantPast) }
            .prefix(5))

        let recentIds = Set(recentActivity.map(\.id))

        let active = waves.filter { wave in
            (wave.status == .running || wave.status == .waiting)
                && pendingPR(for: wave) == nil
                && !recentIds.contains(wave.id)
        }

        let idle = waves.filter { wave in
            wave.status == .idle
                && pendingPR(for: wave) == nil
                && !recentIds.contains(wave.id)
        }

        return WaveGroups(
            blocked: blocked,
            pr: pr,
            recentActivity: recentActivity,
            active: active,
            idle: idle
        )
    }

    private func pendingPR(for wave: WaveViewModel) -> (number: Int, url: URL?)? {
        guard let prNumber = wave.prNumber, wave.prState == .open else {
            return nil
        }
        return (number: prNumber, url: wave.prURL)
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
        waves = []
        selectedWave = nil
        isLoading = false
        errorMessage = nil
        let selectBranch = ProcessInfo.processInfo.environment["CONCERTO_UI_TEST_SELECT_BRANCH"]

        switch mode {
        case .emptyWorkspaces:
            waves = []
        case .sampleWorkspaces:
            configureMockWaves()
        case .mockWaves:
            configureMockWaves()
            if let selectBranch {
                selectedWave = waves.first { $0.branch == selectBranch }
            }
        }
    }

    func configureMockWaves() {
        lfdConnected = true
        waves = [
            WaveViewModel(
                api: Wave(
                    id: "mock-wave-1",
                    name: "swift-falcon",
                    repo: currentRepo?.path ?? "/tmp/demo",
                    flow: "ship",
                    direction: [],
                    area: ["src/auth"],
                    stimulus: Stimulus(kind: .loop),
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
                    repo: currentRepo?.path ?? "/tmp/demo",
                    flow: "ship",
                    direction: ["product-engineer"],
                    area: ["src/api"],
                    stimulus: Stimulus(kind: .loop),
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
                    repo: currentRepo?.path ?? "/tmp/demo",
                    flow: "debug",
                    direction: [],
                    area: ["."],
                    stimulus: Stimulus(kind: .manual),
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
                    repo: currentRepo?.path ?? "/tmp/demo",
                    flow: "polish",
                    direction: [],
                    area: ["."],
                    stimulus: Stimulus(kind: .cron, cron: "0 9 * * *"),
                    status: .idle,
                    iteration: 12
                ),
                prLimit: 5,
                mergeMode: .pr
            )
        ]
        lfdConnected = true
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
                            self.waves = connected.waves.map { WaveViewModel(api: $0) }
                            for wave in self.waves {
                                self.previousWaveStatuses[wave.id] = wave.status
                            }
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
            if let wave = try? await waveService.getWave(event.waveId) {
                upsertWave(WaveViewModel(api: wave))
            }
        case .deleted:
            waves.removeAll { $0.id == event.waveId }
            if selectedWave?.id == event.waveId {
                selectedWave = nil
            }
        }
    }

    private func upsertWave(_ wave: WaveViewModel) {
        let oldStatus = previousWaveStatuses[wave.id]
        if oldStatus != wave.status {
            handleWaveStatusChange(wave: wave, from: oldStatus, to: wave.status)
        }
        previousWaveStatuses[wave.id] = wave.status

        if let index = waves.firstIndex(where: { $0.id == wave.id }) {
            waves[index] = wave
        } else {
            waves.insert(wave, at: 0)
        }
        if selectedWave?.id == wave.id {
            selectedWave = wave
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
            let newViewModels = newWaves.map { WaveViewModel(api: $0) }

            // Detect status transitions and notify
            for wave in newViewModels {
                let oldStatus = previousWaveStatuses[wave.id]
                let newStatus = wave.status

                if oldStatus != newStatus {
                    LoggingService.model("refreshWaves: wave \(wave.id) status changed \(String(describing: oldStatus)) -> \(newStatus)")
                    handleWaveStatusChange(wave: wave, from: oldStatus, to: newStatus)
                }
                previousWaveStatuses[wave.id] = newStatus
            }

            waves = newViewModels
            if let selected = selectedWave,
               let updated = waves.first(where: { $0.id == selected.id }) {
                selectedWave = updated
            }
        } catch {
            LoggingService.model("refreshWaves: error=\(error.localizedDescription)")
            waves = []
        }
    }

    private func handleWaveStatusChange(wave: WaveViewModel, from oldStatus: WaveStatus?, to newStatus: WaveStatus) {
        switch newStatus {
        case .waiting:
            // Get current step from recentSteps if available
            let step = wave.recentSteps.first?.step ?? "step"
            NotificationService.shared.notifyNeedsInteractive(
                waveId: wave.id,
                waveName: wave.displayName,
                step: step
            )

        case .failed:
            // Use step name for error message
            let step = wave.recentSteps.first?.step ?? "unknown step"
            let message = "Error in \(step)"
            NotificationService.shared.notifyError(
                waveId: wave.id,
                waveName: wave.displayName,
                message: message
            )

        case .idle:
            // Check if PR was just created (prNumber set, wasn't before or status changed)
            if let prNumber = wave.prNumber, wave.prState == .open {
                // Only notify if this is a new PR (not just a refresh)
                if oldStatus == .running {
                    NotificationService.shared.notifyPRReady(
                        waveId: wave.id,
                        waveName: wave.displayName,
                        prNumber: prNumber
                    )
                }
            }

        case .running, .completed, .paused:
            break  // No notification needed
        }
    }

    func createWave(name: String) async throws {
        guard let repo = currentRepo else {
            LoggingService.model("createWave: no currentRepo")
            return
        }

        let waveName = name.isEmpty ? NameGenerator.generate() : name
        LoggingService.model("createWave: name=\(waveName) repo=\(repo.path)")

        let wave = try await waveService.createWave(name: waveName, repo: repo)
        let viewModel = WaveViewModel(api: wave)

        LoggingService.model("createWave: success, inserting wave id=\(viewModel.id)")
        waves.insert(viewModel, at: 0)
        selectedWave = viewModel
        LoggingService.model("createWave: selectedWave=\(wave.id)")
    }

    func runWave(
        wave: WaveViewModel,
        area: [String]? = nil,
        direction: [String]? = nil,
        flow: String? = nil,
        stimulus: Stimulus? = nil
    ) async throws {
        let overrides = RunOverrides(
            area: area,
            direction: direction,
            flow: flow,
            stimulus: stimulus
        )
        try await waveService.run(wave.id, overrides: overrides)

        await refreshWaves()
    }

    func stopWave(_ wave: WaveViewModel) async throws {
        try await waveService.stop(wave.id)
        await refreshWaves()
    }

    func cloneWave(_ wave: WaveViewModel) async throws -> WaveViewModel {
        let cloned = try await waveService.cloneWave(wave.id, name: nil)
        let viewModel = WaveViewModel(api: cloned)
        waves.insert(viewModel, at: 0)
        selectedWave = viewModel
        return viewModel
    }

    func deleteWave(_ wave: WaveViewModel) async throws {
        try await waveService.deleteWave(wave.id)
        waves.removeAll { $0.id == wave.id }
        if selectedWave?.id == wave.id {
            selectedWave = nil
        }
    }

    func renameWave(_ wave: WaveViewModel, to newName: String) async throws {
        _ = try await waveService.updateWave(wave.id, config: WaveConfigUpdate(name: newName))
        await refreshWaves()
    }

    func updateWave(
        _ wave: WaveViewModel,
        area: [String]? = nil,
        direction: [String]? = nil,
        flow: String? = nil,
        stimulus: Stimulus? = nil,
        status: WaveStatus? = nil
    ) async throws {
        let config = WaveConfigUpdate(
            area: area,
            direction: direction,
            flow: flow,
            stimulus: stimulus,
            status: status
        )
        _ = try await waveService.updateWave(wave.id, config: config)
        await refreshWaves()
    }

    func connectLfd(outputBuffer: OutputBuffer) async throws {
        LoggingService.lfd("connectLfd: starting")
        try? await waveService.connectLfd()
        LoggingService.lfd("connectLfd: waveService.connectLfd completed")
        startEventSubscription(outputBuffer: outputBuffer)
        LoggingService.lfd("connectLfd: event subscription started")
    }
}
