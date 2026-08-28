import Foundation
import Loopflow
import Observation

struct WorkActivityScope: Equatable, Sendable {
    let wave: String?
    let project: String?
    let task: String?
}

enum PodiumReading<Value> {
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

struct WaveSummary: Equatable {
    let waves: Int
}

@MainActor
@Observable
final class PodiumModel {
    var repoPath: String?
    var selection: WorkReference?
    private(set) var roadmap: PodiumReading<RoadmapSnapshot> = .loading
    private(set) var waves: PodiumReading<[Wave]> = .loading
    private(set) var processActivity: PodiumReading<ActivitySnapshot> = .loading
    private(set) var userAskAttention: PodiumReading<[AskAttentionRecord]> = .loading
    private(set) var workActivity: PodiumReading<WorkActivitySnapshot> = .loading
    private(set) var workActivityScope = WorkActivityScope(
        wave: nil,
        project: nil,
        task: nil
    )
    private(set) var repos: [PortfolioRepo] = []
    private(set) var authoredWavesByRepo: [String: [String]] = [:]
    private(set) var isRefreshing = false

    private let query: RegistryQuery
    private var usesFixedFixture = false
    private var askAttentionGeneration = 0
    private var processActivityRefreshInFlight = false
    private var workActivityGeneration = 0

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

    var waveSummary: WaveSummary? {
        guard waves.value != nil else { return nil }
        return WaveSummary(waves: visibleWaves.count)
    }

    var visibleRepos: [PortfolioRepo] {
        guard let repoPath else { return allRepos }
        let target = repoIdentity(repoPath)
        return allRepos.filter { repoIdentity($0.path) == target }
    }

    var allRepos: [PortfolioRepo] {
        var result: [PortfolioRepo] = []
        var seen = Set<String>()
        let selectedPath = repoPath?.normalizedFilePath
        let orderedRepos = repos.sorted { left, right in
            let leftIsSelected = left.path.normalizedFilePath == selectedPath
            let rightIsSelected = right.path.normalizedFilePath == selectedPath
            return leftIsSelected && !rightIsSelected
        }
        for repo in orderedRepos {
            guard seen.insert(repoIdentity(repo.path)).inserted else { continue }
            result.append(repo)
        }
        for wave in waves.value ?? [] {
            let identity = repoIdentity(wave.repo)
            guard seen.insert(identity).inserted else { continue }
            result.append(PortfolioRepo(path: identity, lastOpened: .distantPast))
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
        let previousProcessActivity = processActivity.value
        let previousUserAskAttention = userAskAttention.value
        let userAskAttentionGeneration = askAttentionGeneration
        let userAskAttentionRepoPath = repoPath
        if previousRoadmap == nil { roadmap = .loading }
        if previousWaves == nil { waves = .loading }
        if previousProcessActivity == nil { processActivity = .loading }
        if previousUserAskAttention == nil { userAskAttention = .loading }

        async let roadmapResult = readRoadmap()
        async let wavesResult = readWaves()
        async let userAskAttentionResult = readUserAskAttention(
            repoPath: userAskAttentionRepoPath
        )
        if !processActivityRefreshInFlight {
            processActivityRefreshInFlight = true
            processActivity = reading(
                from: await readProcessActivity(),
                lastGood: previousProcessActivity
            )
            processActivityRefreshInFlight = false
        }
        roadmap = reading(from: await roadmapResult, lastGood: previousRoadmap)
        waves = reading(from: await wavesResult, lastGood: previousWaves)
        let nextUserAskAttention = await userAskAttentionResult
        if askAttentionGeneration == userAskAttentionGeneration {
            userAskAttention = reading(
                from: nextUserAskAttention,
                lastGood: previousUserAskAttention
            )
        }
        selectRequestedWaveIfNeeded()
        clearSelectionIfOutsideScope()
        await refreshWorkActivity()
    }

    func refreshProcessActivity() async {
        guard !usesFixedFixture, !isRefreshing, !processActivityRefreshInFlight else { return }
        processActivityRefreshInFlight = true
        defer { processActivityRefreshInFlight = false }

        let previous = processActivity.value
        if previous == nil { processActivity = .loading }
        processActivity = reading(
            from: await readProcessActivity(),
            lastGood: previous
        )
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
        await Self.resolveRepoOrigins(discovered.map(\.path))
        repos = discovered
        authoredWavesByRepo = await PortfolioDiscovery.authoredWaves(in: discovered)
        if repoPath == nil, let initialRepoPath {
            repoPath = PortfolioDiscovery.resolveLaunchRepo(initialRepoPath).path
        }
        clearSelectionIfOutsideScope()
    }

    func setRepoPath(_ path: String?) {
        if repoPath?.normalizedFilePath != path?.normalizedFilePath {
            askAttentionGeneration &+= 1
            userAskAttention = .loading
        }
        repoPath = path
        clearSelectionIfOutsideScope()
    }

    func setWavePaused(waveId: String, paused: Bool) async throws {
        let target: (name: String, repo: String)? = if let roadmap = wave(id: waveId) {
            (roadmap.wave.name, roadmap.wave.repo)
        } else if let wave = rosterWave(id: waveId) {
            (wave.name, wave.repo)
        } else {
            nil
        }
        guard let target else {
            throw RegistryQueryError("Wave is absent from the latest Podium evidence")
        }
        _ = try await query.setWavePaused(
            wave: target.name,
            paused: paused,
            cwd: target.repo
        )
        await refresh()
    }

    func select(_ selection: WorkReference?) {
        setSelection(selection)
        clearSelectionIfOutsideScope()
    }

    func refreshWorkActivity() async {
        guard !usesFixedFixture else { return }
        workActivityGeneration &+= 1
        let generation = workActivityGeneration
        let requestedSelection = selection
        guard let scope = activityScope(for: requestedSelection) else {
            workActivity = .unavailable(
                lastGood: nil,
                reason: "Selected Work is absent from the latest Podium evidence"
            )
            return
        }

        let sameScope = workActivityScope == scope
        let previous = sameScope ? workActivity.value : nil
        if !sameScope { workActivity = .loading }
        workActivityScope = scope

        let result = await readWorkActivity(scope: scope)

        guard selection == requestedSelection, workActivityGeneration == generation else { return }
        workActivity = reading(from: result, lastGood: previous)
    }

    func refreshUserAskAttention() async {
        guard !usesFixedFixture else { return }
        let generation = askAttentionGeneration
        let repoPath = repoPath
        let previous = userAskAttention.value
        let result = await readUserAskAttention(repoPath: repoPath)
        guard askAttentionGeneration == generation else { return }
        userAskAttention = reading(from: result, lastGood: previous)
    }

    func wave(id: String) -> WaveRoadmap? {
        roadmap.value?.waves.first { $0.wave.id == id }
    }

    func rosterWave(id: String) -> WaveViewModel? {
        visibleWaves.first { $0.id == id }
    }

    func project(id: String) -> (wave: WaveRoadmap, project: RoadmapProject)? {
        for wave in roadmap.value?.waves ?? [] {
            if let project = wave.projects.items.first(where: { $0.id == id }) {
                return (wave, project)
            }
        }
        return nil
    }

    func task(id: String) -> (
        wave: WaveRoadmap,
        project: RoadmapProject,
        task: RoadmapTask
    )? {
        for wave in roadmap.value?.waves ?? [] {
            for project in wave.projects.items {
                if let task = project.tasks.first(where: { $0.id == id }) {
                    return (wave, project, task)
                }
            }
        }
        return nil
    }

    func waveId(for work: WorkReference) -> String? {
        switch work.kind {
        case .wave:
            work.id
        case .project:
            project(id: work.id)?.wave.wave.id
        case .task:
            task(id: work.id)?.wave.wave.id
        }
    }

    func clearSelectionIfOutsideScope() {
        guard let selection else { return }
        let visibleIds = Set(visibleWaves.map(\.id) + visibleRoadmaps.map { $0.wave.id })
        guard let waveId = waveId(for: selection), visibleIds.contains(waveId) else {
            setSelection(nil)
            return
        }

        switch selection.kind {
        case .wave:
            break
        case .project:
            if project(id: selection.id) == nil {
                setSelection(.wave(id: waveId))
            }
        case .task:
            if task(id: selection.id) == nil {
                setSelection(.wave(id: waveId))
            }
        }
    }

    func applyFixture(
        roadmap: PodiumReading<RoadmapSnapshot>,
        waves: PodiumReading<[Wave]>,
        processActivity: PodiumReading<ActivitySnapshot>,
        workActivity: PodiumReading<WorkActivitySnapshot>,
        repos: [PortfolioRepo],
        fixed: Bool = false
    ) {
        self.roadmap = roadmap
        self.waves = waves
        self.processActivity = processActivity
        self.workActivity = workActivity
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
        let target = repoIdentity(repoPath)
        return values.filter { repoIdentity(repo($0)) == target }
    }

    private func waveIdentity(repo: String, name: String) -> String {
        "\(repoIdentity(repo))#\(name)"
    }

    func repoIdentity(_ path: String) -> String {
        let path = path.normalizedFilePath
        return WaveOrigin.cached(path).normalizedFilePath
    }

    private nonisolated static func resolveRepoOrigins(_ paths: [String]) async {
        await Task.detached {
            for path in Set(paths.map(\.normalizedFilePath)) {
                _ = WaveOrigin.resolve(path)
            }
        }.value
    }

    private func activityScope(for selection: WorkReference?) -> WorkActivityScope? {
        guard let selection else {
            return WorkActivityScope(wave: nil, project: nil, task: nil)
        }
        switch selection.kind {
        case .wave:
            let name: String
            if let wave = wave(id: selection.id)?.wave {
                name = wave.name
            } else if let wave = rosterWave(id: selection.id) {
                name = wave.api.name
            } else {
                return nil
            }
            return WorkActivityScope(
                wave: name,
                project: nil,
                task: nil
            )
        case .project:
            guard let selected = project(id: selection.id) else { return nil }
            return WorkActivityScope(
                wave: selected.wave.wave.name,
                project: selected.project.project.slug,
                task: nil
            )
        case .task:
            guard let selected = task(id: selection.id) else { return nil }
            return WorkActivityScope(
                wave: selected.wave.wave.name,
                project: selected.project.project.slug,
                task: selected.task.task.identifier
            )
        }
    }

    private func setSelection(_ selection: WorkReference?) {
        guard self.selection != selection else { return }
        workActivityGeneration &+= 1
        self.selection = selection
        if !usesFixedFixture { workActivity = .loading }
    }

    private func selectRequestedWaveIfNeeded() {
        guard selection == nil, let requested = AppTestMode.selectBranch else { return }
        guard let wave = visibleRoadmaps.first(where: { $0.wave.name == requested }) else { return }
        setSelection(.wave(id: wave.wave.id))
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
            let waves = try await query.allWaves()
            await Self.resolveRepoOrigins(waves.map(\.repo))
            return .success(waves)
        } catch {
            return .failure(error)
        }
    }

    private func readProcessActivity() async -> Result<ActivitySnapshot, Error> {
        do {
            let snapshot = try await query.processActivity()
            await Self.resolveRepoOrigins(snapshot.nodes.compactMap(\.repo))
            return .success(snapshot)
        } catch {
            return .failure(error)
        }
    }

    private func readUserAskAttention(
        repoPath: String?
    ) async -> Result<[AskAttentionRecord], Error> {
        guard let repoPath else { return .success([]) }
        do {
            return .success(try await query.userAskAttention(cwd: repoPath))
        } catch {
            return .failure(error)
        }
    }

    private func readWorkActivity(
        scope: WorkActivityScope
    ) async -> Result<WorkActivitySnapshot, Error> {
        do {
            return .success(try await query.workActivity(
                wave: scope.wave,
                project: scope.project,
                task: scope.task
            ))
        } catch {
            return .failure(error)
        }
    }

    private func reading<Value>(
        from result: Result<Value, Error>,
        lastGood: Value?
    ) -> PodiumReading<Value> {
        switch result {
        case .success(let value):
            .available(value)
        case .failure(let error):
            .unavailable(lastGood: lastGood, reason: error.localizedDescription)
        }
    }
}
