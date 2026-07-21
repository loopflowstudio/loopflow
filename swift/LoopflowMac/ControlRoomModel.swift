import Foundation
import Loopflow
import Observation

enum ControlRoomSelection: Equatable, Hashable {
    case wave(waveId: String)
    case project(waveId: String, projectId: String)
    case task(waveId: String, taskId: String)

    var waveId: String {
        switch self {
        case .wave(let waveId), .project(let waveId, _), .task(let waveId, _):
            waveId
        }
    }
}

enum ControlRoomReading<Value> {
    case loading
    case available(Value)
    case unavailable(lastGood: Value?, reason: String)

    var value: Value? {
        switch self {
        case .loading:
            nil
        case .available(let value):
            value
        case .unavailable(let lastGood, _):
            lastGood
        }
    }

    var errorMessage: String? {
        guard case .unavailable(_, let reason) = self else { return nil }
        return reason
    }

    var isLoading: Bool {
        if case .loading = self { return true }
        return false
    }
}

struct ControlRoomActivitySummary: Equatable {
    let activeAgents: Int
    let working: Int
    let waiting: Int
    let stalled: Int
    let orphaned: Int
    let unclaimed: Int
    let outputTokensPerSecond5m: Double
    let measuredOutputTokens: UInt64
}

extension ActivitySnapshot {
    var controlRoomSummary: ControlRoomActivitySummary {
        let providers = nodes.filter { $0.kind == .providerLaunch }
        return ControlRoomActivitySummary(
            activeAgents: providers.count + providerProcesses.count,
            working: providers.count { $0.state == .working },
            waiting: providers.count { $0.state == .waiting },
            stalled: providers.count { $0.state == .stalled },
            orphaned: providerProcesses.count { $0.claim == .orphaned },
            unclaimed: providerProcesses.count { $0.claim == .unclaimed },
            outputTokensPerSecond5m: aggregate.outputTokensPerSecondFast,
            measuredOutputTokens: aggregate.measuredOutputTokens
        )
    }
}

@MainActor
@Observable
final class ControlRoomModel {
    var repoPath: String?
    var selection: ControlRoomSelection?
    private(set) var roadmap: ControlRoomReading<RoadmapSnapshot> = .loading
    private(set) var waves: ControlRoomReading<[Wave]> = .loading
    private(set) var activity: ControlRoomReading<ActivitySnapshot> = .loading
    private(set) var repos: [PortfolioRepo] = []
    private(set) var authoredWavesByRepo: [String: [String]] = [:]
    private(set) var isRefreshing = false

    private let query: RegistryQuery
    private var usesFixedFixture = false

    init(query: RegistryQuery, repoPath: String? = nil) {
        self.query = query
        self.repoPath = repoPath
    }

    var visibleRoadmaps: [WaveRoadmap] {
        filterByRepo(roadmap.value?.waves ?? [], repo: { $0.wave.repo })
    }

    var visibleWaves: [WaveViewModel] {
        let registered = filterByRepo(waves.value ?? [], repo: { $0.repo })
        let registeredNames = Set(registered.map { waveIdentity(repo: $0.repo, name: $0.name) })
        var result = registered.map { wave in
            let objective = roadmap.value?.waves.first(where: { $0.wave.id == wave.id })?.wave.goal ?? ""
            let plan = objective.isEmpty ? nil : WavePlan(objective: objective)
            return WaveViewModel(api: wave, plan: plan)
        }

        for repo in visibleRepos {
            for name in authoredWavesByRepo[repo.path] ?? [] {
                let identity = waveIdentity(repo: repo.path, name: name)
                guard !registeredNames.contains(identity) else { continue }
                result.append(WaveViewModel(
                    api: Wave(
                        id: identity,
                        name: name,
                        repo: repo.path,
                        status: .ready
                    ),
                    isRegistered: false
                ))
            }
        }
        return result.sorted {
            $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending
        }
    }

    var visibleRepos: [PortfolioRepo] {
        guard let repoPath else { return allRepos }
        let target = WaveOrigin.resolve(repoPath).normalizedFilePath
        return allRepos.filter { WaveOrigin.resolve($0.path).normalizedFilePath == target }
    }

    var allRepos: [PortfolioRepo] {
        var result = repos
        var seen = Set(result.map { WaveOrigin.resolve($0.path).normalizedFilePath })
        for wave in waves.value ?? [] {
            let path = WaveOrigin.resolve(wave.repo).normalizedFilePath
            guard seen.insert(path).inserted else { continue }
            result.append(PortfolioRepo(path: path, lastOpened: .distantPast))
        }
        return result.sorted {
            $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending
        }
    }

    func refresh() async {
        guard !usesFixedFixture else { return }
        guard !isRefreshing else { return }
        isRefreshing = true
        defer { isRefreshing = false }

        let previousRoadmap = roadmap.value
        let previousWaves = waves.value
        let previousActivity = activity.value
        if previousRoadmap == nil { roadmap = .loading }
        if previousWaves == nil { waves = .loading }
        if previousActivity == nil { activity = .loading }

        async let roadmapResult = readRoadmap()
        async let wavesResult = readWaves()
        async let activityResult = readActivity()
        roadmap = reading(from: await roadmapResult, lastGood: previousRoadmap)
        waves = reading(from: await wavesResult, lastGood: previousWaves)
        activity = reading(from: await activityResult, lastGood: previousActivity)
        selectRequestedWaveIfNeeded()
        clearSelectionIfOutsideScope()
    }

    func refreshPortfolio(
        initialRepoPath: String?,
        persistedRepos: [PortfolioRepo] = []
    ) async {
        guard !usesFixedFixture else { return }
        let discovered = await PortfolioDiscovery.repos(
            initialRepoPath: initialRepoPath,
            persistedRepos: persistedRepos
        )
        repos = discovered
        authoredWavesByRepo = await PortfolioDiscovery.authoredWaves(in: discovered)
        if repoPath == nil, let initialRepoPath {
            repoPath = PortfolioDiscovery.resolveLaunchRepo(initialRepoPath).path
        }
        clearSelectionIfOutsideScope()
    }

    func setRepoPath(_ path: String?) {
        repoPath = path
        clearSelectionIfOutsideScope()
    }

    func select(_ selection: ControlRoomSelection?) {
        self.selection = selection
        clearSelectionIfOutsideScope()
    }

    func wave(id: String) -> WaveRoadmap? {
        roadmap.value?.waves.first { $0.wave.id == id }
    }

    func rosterWave(id: String) -> WaveViewModel? {
        visibleWaves.first { $0.id == id }
    }

    func project(waveId: String, projectId: String) -> RoadmapProject? {
        wave(id: waveId)?.projects.items.first { $0.id == projectId }
    }

    func task(waveId: String, taskId: String) -> (project: RoadmapProject, task: RoadmapTask)? {
        guard let projects = wave(id: waveId)?.projects.items else { return nil }
        for project in projects {
            if let task = project.tasks.first(where: { $0.id == taskId }) {
                return (project, task)
            }
        }
        return nil
    }

    func clearSelectionIfOutsideScope() {
        guard let selection else { return }
        let visibleIds = Set(visibleWaves.map(\.id) + visibleRoadmaps.map { $0.wave.id })
        guard visibleIds.contains(selection.waveId) else {
            self.selection = nil
            return
        }

        switch selection {
        case .wave:
            break
        case .project(let waveId, let projectId):
            if project(waveId: waveId, projectId: projectId) == nil {
                self.selection = .wave(waveId: waveId)
            }
        case .task(let waveId, let taskId):
            if task(waveId: waveId, taskId: taskId) == nil {
                self.selection = .wave(waveId: waveId)
            }
        }
    }

    func applyFixture(
        roadmap: ControlRoomReading<RoadmapSnapshot>,
        waves: ControlRoomReading<[Wave]>,
        activity: ControlRoomReading<ActivitySnapshot>,
        repos: [PortfolioRepo],
        fixed: Bool = false
    ) {
        self.roadmap = roadmap
        self.waves = waves
        self.activity = activity
        self.repos = repos
        authoredWavesByRepo = [:]
        usesFixedFixture = fixed
        clearSelectionIfOutsideScope()
    }

    private func filterByRepo<Value>(
        _ values: [Value],
        repo: (Value) -> String
    ) -> [Value] {
        guard let repoPath else { return values }
        let target = WaveOrigin.resolve(repoPath).normalizedFilePath
        return values.filter { WaveOrigin.resolve(repo($0)).normalizedFilePath == target }
    }

    private func waveIdentity(repo: String, name: String) -> String {
        "\(WaveOrigin.resolve(repo).normalizedFilePath)#\(name)"
    }

    private func selectRequestedWaveIfNeeded() {
        guard selection == nil, let requested = AppTestMode.selectBranch else { return }
        guard let wave = visibleRoadmaps.first(where: { $0.wave.name == requested }) else { return }
        selection = .wave(waveId: wave.wave.id)
    }

    private func readRoadmap() async -> Result<RoadmapSnapshot, Error> {
        do {
            return .success(try await query.roadmap())
        } catch {
            return .failure(error)
        }
    }

    private func readWaves() async -> Result<[Wave], Error> {
        do {
            return .success(try await query.allWaves())
        } catch {
            return .failure(error)
        }
    }

    private func readActivity() async -> Result<ActivitySnapshot, Error> {
        do {
            return .success(try await query.activity())
        } catch {
            return .failure(error)
        }
    }

    private func reading<Value>(
        from result: Result<Value, Error>,
        lastGood: Value?
    ) -> ControlRoomReading<Value> {
        switch result {
        case .success(let value):
            .available(value)
        case .failure(let error):
            .unavailable(lastGood: lastGood, reason: error.localizedDescription)
        }
    }
}
