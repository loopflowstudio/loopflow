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

    // Wave state — delegated to WaveStore
    let waveStore = WaveStore()

    var waves: [WaveViewModel] { waveStore.ordered }
    var waveGroups: WaveGroups { waveStore.groups }

    // Selection — ID-based, derived from store
    var selectedWaveId: String?

    var selectedWave: WaveViewModel? {
        get { selectedWaveId.flatMap { waveStore.wave(for: $0) } }
        set { selectedWaveId = newValue?.id }
    }

    // Loading
    var isLoading: Bool = false
    var errorMessage: String?

    // Daemon connection
    var lfdConnected: Bool = false

    // Services
    private let waveService = LocalWaveService()
    private var eventService: LocalEventService?

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
        waveStore.setAll([
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
        ])
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
                            self.waveStore.setAll(connected.waves.map { WaveViewModel(api: $0) })
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
        case .deleted:
            waveStore.remove(event.waveId)
            if selectedWaveId == event.waveId {
                selectedWaveId = nil
            }
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
            waveStore.setAll(newWaves.map { WaveViewModel(api: $0) })
        } catch {
            LoggingService.model("refreshWaves: error=\(error.localizedDescription)")
            waveStore.removeAll()
        }
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

    func createWave(name: String) async throws {
        guard let repo = currentRepo else {
            LoggingService.model("createWave: no currentRepo")
            return
        }

        let waveName = name.isEmpty ? NameGenerator.generate() : name
        LoggingService.model("createWave: name=\(waveName) repo=\(repo.path)")

        let pendingId = "pending-\(UUID().uuidString)"
        let pending = WaveViewModel(
            api: Wave(
                id: pendingId,
                name: waveName,
                repo: repo.path,
                flow: "design",
                direction: [],
                area: [],
                stimulus: Stimulus(kind: .once),
                status: .idle,
                iteration: 0
            )
        )
        waveStore.set(pending)
        selectedWaveId = pendingId

        do {
            let wave = try await waveService.createWave(name: waveName, repo: repo)
            let viewModel = WaveViewModel(api: wave)
            waveStore.remove(pendingId)
            waveStore.set(viewModel)
            selectedWaveId = viewModel.id
            LoggingService.model("createWave: selectedWave=\(wave.id)")
        } catch {
            waveStore.remove(pendingId)
            if selectedWaveId == pendingId {
                selectedWaveId = nil
            }
            throw error
        }
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
    }

    func stopWave(_ wave: WaveViewModel) async throws {
        try await waveService.stop(wave.id)
    }

    func cloneWave(_ wave: WaveViewModel) async throws -> WaveViewModel {
        let cloned = try await waveService.cloneWave(wave.id, name: nil)
        let viewModel = WaveViewModel(api: cloned)
        waveStore.set(viewModel)
        selectedWave = viewModel
        return viewModel
    }

    func deleteWave(_ wave: WaveViewModel) async throws {
        try await waveService.deleteWave(wave.id)
        waveStore.remove(wave.id)
        if selectedWaveId == wave.id {
            selectedWaveId = nil
        }
    }

    func renameWave(_ wave: WaveViewModel, to newName: String) async throws {
        let snapshot = waveStore.applyOptimistic(wave.id) { $0.name = newName }
        do {
            _ = try await waveService.updateWave(wave.id, config: WaveConfigUpdate(name: newName))
            waveStore.commitMutation(wave.id)
        } catch {
            if let snapshot { waveStore.rollback(snapshot) }
            throw error
        }
    }

    func updateWave(
        _ wave: WaveViewModel,
        area: [String]? = nil,
        direction: [String]? = nil,
        flow: String? = nil,
        stimulus: Stimulus? = nil,
        status: WaveStatus? = nil
    ) async throws {
        let snapshot = waveStore.applyOptimistic(wave.id) { w in
            if let area { w.area = area }
            if let direction { w.direction = direction }
            if let flow { w.flow = flow }
            if let stimulus { w.stimulus = stimulus }
            if let status { w.status = status }
        }
        do {
            let config = WaveConfigUpdate(
                area: area,
                direction: direction,
                flow: flow,
                stimulus: stimulus,
                status: status
            )
            _ = try await waveService.updateWave(wave.id, config: config)
            waveStore.commitMutation(wave.id)
        } catch {
            if let snapshot { waveStore.rollback(snapshot) }
            throw error
        }
    }

    func landWave(_ wave: WaveViewModel) async throws {
        try await waveService.landWave(wave.id)
    }

    func nextWave(_ wave: WaveViewModel) async throws {
        _ = try await waveService.nextWave(wave.id)
    }

    func connectLfd(outputBuffer: OutputBuffer) async throws {
        LoggingService.lfd("connectLfd: starting")
        try? await waveService.connectLfd()
        LoggingService.lfd("connectLfd: waveService.connectLfd completed")
        startEventSubscription(outputBuffer: outputBuffer)
        LoggingService.lfd("connectLfd: event subscription started")
    }
}
