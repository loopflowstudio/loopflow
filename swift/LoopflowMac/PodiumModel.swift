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

@MainActor
@Observable
final class PodiumModel {
    var historyWave: WaveSnapshot?
    var historyReference: String?
    @ObservationIgnored private(set) var historyLookup: Task<Void, Never>?
    var repoPath: String?
    var selection: WorkReference? { navigation.selection }
    @ObservationIgnored private var navigationByRepo: [String: WorkspaceNavigation] = [:]

    var navigation: WorkspaceNavigation {
        let key = repoPath ?? ""
        if let existing = navigationByRepo[key] { return existing }
        let state = WorkspaceNavigation()
        navigationByRepo[key] = state
        return state
    }

    var workspace: WorkspaceProjection {
        WorkspaceProjection(
            roadmaps: visibleRoadmaps, sessions: sessions.value ?? [],
            activeWorktrees: Set((processActivity.value?.nodes ?? [])
                .filter { $0.kind == .providerProcess }.compactMap(\.worktree))
        )
    }
    private(set) var roadmap: PodiumReading<RoadmapSnapshot> = .loading {
        didSet {
            // Retain the latest observed Task across temporary chapter membership
            // gaps, including selections saved in another repository.
            for navigation in navigationByRepo.values {
                guard let selection = navigation.selection, selection.kind == .task else { continue }
                for wave in roadmap.value?.waves ?? [] {
                    if let task = wave.tasks.items.first(where: { $0.id == selection.id }) {
                        navigation.selectedTaskEvidence = (wave, task)
                        break
                    }
                }
            }
        }
    }
    private(set) var waves: PodiumReading<[Wave]> = .loading
    private(set) var processActivity: PodiumReading<ActivitySnapshot> = .loading
    private(set) var activeRuns: PodiumReading<ActiveRunsSnapshot> = .loading
    private(set) var isRefreshingActiveRuns = false
    private(set) var activeRunsNeedsRetry = false
    @ObservationIgnored private var activeRunsObservation: ActiveRunsObservation?
    @ObservationIgnored private var activeRunsTask: Task<Void, Never>?
    private var activeRunsGeneration = 0
    private var activeRunsDemanded = false
    private var sessionReadings: [String: PodiumReading<[SessionRecord]>] = [:]
    private(set) var sessions: PodiumReading<[SessionRecord]> {
        get { sessionReadings[repoPath ?? ""] ?? .loading }
        set { sessionReadings[repoPath ?? ""] = newValue }
    }
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
    private var sessionsGeneration = 0
    private var roadmapGeneration = 0
    private var processActivityRefreshInFlight = false
    private var workActivityGeneration = 0

    init(query: RegistryQuery, repoPath: String? = nil) {
        self.query = query
        self.repoPath = repoPath.map(WaveOrigin.resolve)
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
            let identity = repoIdentity(repo.path)
            guard seen.insert(identity).inserted else { continue }
            result.append(PortfolioRepo(path: identity, lastOpened: repo.lastOpened))
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
        let generation = roadmapGeneration
        let previousWaves = waves.value
        if previousRoadmap == nil { roadmap = .loading }
        if previousWaves == nil { waves = .loading }

        async let roadmapResult = readRoadmap()
        async let wavesResult = readWaves()
        async let sessionRefresh: Void = refreshSessions()
        waves = reading(from: await wavesResult, lastGood: previousWaves)
        await sessionRefresh
        let result = await roadmapResult
        if generation == roadmapGeneration {
            roadmap = reading(from: result, lastGood: previousRoadmap)
        }
        selectRequestedWaveIfNeeded()
        if visibleRoadmaps.allSatisfy({ wave in
            guard case .available(_, false) = wave.tasks else { return false }
            return wave.unavailableTasks.isEmpty
        }) {
            clearSelectionIfOutsideScope()
        }
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

    /// First demand starts a window-owned reader; navigation never restarts it.
    func observeActiveRuns() {
        guard !activeRunsDemanded else { return }
        activeRunsDemanded = true
        startActiveRuns()
    }

    func refreshActiveRuns() async {
        activeRunsDemanded = true
        if activeRunsNeedsRetry || activeRunsTask == nil {
            await stopActiveRuns()
            startActiveRuns()
        } else {
            await activeRunsObservation?.request(.refresh)
        }
    }

    func rescanActiveRuns() async {
        guard let observation = activeRunsObservation, !activeRunsNeedsRetry else { return }
        activeRuns = .unavailable(lastGood: activeRuns.value, reason: "Rediscovering active Runs after wake")
        isRefreshingActiveRuns = true
        await observation.request(.rescan)
    }

    /// Attached once to the window root, independently of repository or pane visibility.
    func activeRunsLifetime() async {
        do {
            while !Task.isCancelled { try await Task.sleep(for: .seconds(3600)) }
        } catch { }
        await stopActiveRuns()
        activeRunsDemanded = false
    }

    func stopActiveRuns() async {
        activeRunsGeneration += 1
        let generation = activeRunsGeneration
        let task = activeRunsTask
        task?.cancel()
        await task?.value
        guard activeRunsGeneration == generation else { return }
        activeRunsTask = nil
        activeRunsObservation = nil
        isRefreshingActiveRuns = false
    }

    deinit { activeRunsTask?.cancel() }

    private func startActiveRuns() {
        guard activeRunsTask == nil else { return }
        activeRunsGeneration += 1
        let generation = activeRunsGeneration
        isRefreshingActiveRuns = true
        activeRunsNeedsRetry = false
        let query = query
        activeRunsTask = Task { [weak self] in
            // Only configuration replacement retries automatically. Transport failures
            // retain evidence and wait for the explicit Retry action.
            while !Task.isCancelled {
                var observation: ActiveRunsObservation?
                var replace = false
                var discoveryFailed = false
                do {
                    let opened = try await query.watchActiveRuns()
                    observation = opened
                    guard !Task.isCancelled, self?.activeRunsGeneration == generation else {
                        await opened.cancel()
                        return
                    }
                    self?.activeRunsObservation = opened
                    for try await snapshot in opened.snapshots {
                        guard !Task.isCancelled, self?.activeRunsGeneration == generation else { break }
                        self?.receiveActiveRuns(snapshot)
                        if snapshot.discovery == .unavailable {
                            discoveryFailed = true
                            break
                        }
                    }
                    if !Task.isCancelled && !discoveryFailed {
                        throw RegistryQueryError("Active Run observation ended")
                    }
                } catch ActiveRunsObservationError.configurationChanged {
                    replace = true
                    if self?.activeRunsGeneration == generation {
                        self?.activeRuns = .loading
                    }
                } catch {
                    if !Task.isCancelled, let self, self.activeRunsGeneration == generation {
                        self.activeRuns = .unavailable(lastGood: self.activeRuns.value, reason: error.localizedDescription)
                        self.activeRunsNeedsRetry = true
                    }
                }
                await observation?.cancel()
                guard !Task.isCancelled, let self, self.activeRunsGeneration == generation else { return }
                self.activeRunsObservation = nil
                if replace {
                    self.activeRuns = .loading
                    self.isRefreshingActiveRuns = true
                    continue
                }
                self.activeRunsTask = nil
                self.isRefreshingActiveRuns = false
                return
            }
        }
    }

    private func receiveActiveRuns(_ snapshot: ActiveRunsSnapshot) {
        isRefreshingActiveRuns = snapshot.discovery == .scanning
        activeRunsNeedsRetry = snapshot.discovery == .unavailable
        switch snapshot.discovery {
        case .ready:
            activeRuns = .available(snapshot)
        case .scanning, .unavailable:
            let reason = snapshot.discovery == .scanning ? "Discovering active Runs…" : "Active Run discovery unavailable"
            activeRuns = .unavailable(lastGood: activeRuns.value, reason: ([reason] + snapshot.gaps).joined(separator: "; "))
        }
    }

    func refreshPortfolio(
        initialRepoPath: String?,
        persistedRepos: [PortfolioRepo] = []
    ) async {
        guard !usesFixedFixture else { return }
        let discoveryRepoPath = initialRepoPath ?? repoPath
        let discovered = await PortfolioDiscovery.repos(
            initialRepoPath: discoveryRepoPath,
            persistedRepos: persistedRepos
        )
        await Self.resolveRepoOrigins(discovered.map(\.path))
        repos = discovered
        authoredWavesByRepo = await PortfolioDiscovery.authoredWaves(in: discovered)
        if repoPath == nil, let initialRepoPath {
            repoPath = PortfolioDiscovery.resolveLaunchRepo(initialRepoPath)
        }
        // Repository discovery cannot establish that selected Work was removed.
        // Planning refresh owns that reconciliation once its evidence is complete.
    }

    func setRepoPath(_ path: String?) {
        let path = path.map(WaveOrigin.resolve)
        if repoPath?.normalizedFilePath != path?.normalizedFilePath {
            historyLookup?.cancel()
            historyLookup = nil
            sessionsGeneration &+= 1
            workActivityGeneration &+= 1
            if !usesFixedFixture { workActivity = .loading }
        }
        // Navigation is already scoped to this repository. A partial planning
        // read cannot invalidate its saved selection merely because we return.
        repoPath = path
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

    func updateTaskDirective(task: RoadmapTask, wave: WaveSnapshot, text: String) async throws {
        try await query.updateTaskDirective(id: task.id, wave: wave.name, text: text, cwd: wave.repo)
        // Polls started before this write must not restore the old directive.
        roadmapGeneration &+= 1
        let generation = roadmapGeneration
        let result = await readRoadmap()
        if generation == roadmapGeneration {
            roadmap = reading(from: result, lastGood: roadmap.value)
        }
        switch result {
        case .success(let snapshot):
            guard snapshot.waves.contains(where: { row in
                row.wave.id == wave.id && row.tasks.items.contains(where: { $0.id == task.id })
            }) else {
                throw RegistryQueryError("Update accepted, but the Task is absent from refreshed planning. Your draft is retained.")
            }
        case .failure(let error):
            throw RegistryQueryError("Update accepted, but planning refresh failed: \(error.localizedDescription)")
        }
    }

    func select(_ requested: WorkReference?) {
        historyLookup?.cancel()
        historyLookup = nil
        let selection: WorkReference?
        if let requested, requested.kind == .project {
            if let current = waveForChapter(projectId: requested.id) {
                selection = .wave(id: current.wave.id)
            } else {
                historyLookup = Task { await openHistoricalReference(requested.id) }
                return
            }
        } else { selection = requested }
        navigation.selectedSessionId = nil
        navigation.content = selection == nil ? .overview : .details
        setSelection(selection)
        clearSelectionIfOutsideScope()
    }

    private func openHistoricalReference(_ reference: String) async {
        for wave in visibleRoadmaps {
            guard !Task.isCancelled else { return }
            let result = try? await query.chapterHistory(wave: wave.wave.name, cwd: wave.wave.repo)
            guard !Task.isCancelled else { return }
            guard let entries = result else { continue }
            if entries.contains(where: { $0.sourceProjectId == reference || $0.sourceProjectSlug == reference || $0.sourceWorkId == reference }) {
                select(.wave(id: wave.wave.id))
                historyReference = reference
                historyWave = wave.wave
                return
            }
        }
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

        let previous = workActivityScope == scope ? workActivity.value : nil
        workActivityScope = scope
        if previous == nil { workActivity = .loading }

        let result = await readWorkActivity(scope: scope)

        guard selection == requestedSelection, workActivityGeneration == generation else { return }
        workActivity = reading(from: result, lastGood: previous)
    }

    func refreshSessions() async {
        guard !usesFixedFixture || AppTestMode.current() == .sessionFixtures else { return }
        sessionsGeneration &+= 1
        let generation = sessionsGeneration
        let repoPath = repoPath
        let previous = sessions.value
        let result = await readSessions(repoPath: repoPath)
        guard sessionsGeneration == generation else { return }
        sessions = reading(from: result, lastGood: previous)
        if case .success(let records) = result,
           let selected = navigation.selectedSessionId, !records.contains(where: { $0.id == selected }) {
            navigation.selectedSessionId = nil
        }
    }

    func sessionResolved(_ id: String, repo: String) {
        // A pre-resolution read must not resurrect the completed human boundary.
        // Resolution may finish after the human has switched repositories.
        sessionsGeneration &+= 1
        if navigationByRepo[repo]?.selectedSessionId == id {
            navigationByRepo[repo]?.selectedSessionId = nil
        }
        switch sessionReadings[repo] {
        case .available(let records):
            sessionReadings[repo] = .available(records.filter { $0.id != id })
        case .unavailable(let records, let reason):
            sessionReadings[repo] = .unavailable(lastGood: records?.filter { $0.id != id }, reason: reason)
        case .loading, nil:
            break
        }
    }

    func wave(id: String) -> WaveRoadmap? {
        roadmap.value?.waves.first { $0.wave.id == id }
    }

    func rosterWave(id: String) -> WaveViewModel? {
        visibleWaves.first { $0.id == id }
    }

    func waveForChapter(projectId: String) -> WaveRoadmap? {
        roadmap.value?.waves.first { $0.chapter?.sourceProjectId == projectId || $0.chapter?.sourceProjectSlug == projectId || $0.chapter?.sourceWorkId == projectId }
    }

    func task(id: String) -> (wave: WaveRoadmap, task: RoadmapTask)? {
        for wave in visibleRoadmaps {
            if let task = wave.tasks.items.first(where: { $0.id == id }) { return (wave, task) }
        }
        if let previous = navigation.selectedTaskEvidence, previous.task.id == id,
           let current = wave(id: previous.wave.wave.id),
           current.tasks.unavailableReason != nil || current.chapter?.phase != "complete" {
            return (current, previous.task)
        }
        return nil
    }

    func waveId(for work: WorkReference) -> String? {
        switch work.kind {
        case .wave:
            work.id
        case .project:
            waveForChapter(projectId: work.id)?.wave.id
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
            setSelection(.wave(id: waveId))
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
        if fixed && AppTestMode.current() != .sessionFixtures { self.sessions = .available([]) }
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
            return nil
        case .task:
            guard let selected = task(id: selection.id) else { return nil }
            return WorkActivityScope(
                wave: selected.wave.wave.name,
                project: nil,
                task: selected.task.task.identifier
            )
        }
    }

    private func setSelection(_ selection: WorkReference?) {
        guard self.selection != selection else { return }
        workActivityGeneration &+= 1
        navigation.selectedTaskEvidence = selection.flatMap { $0.kind == .task ? task(id: $0.id) : nil }
        navigation.selection = selection
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

    private func readSessions(
        repoPath: String?
    ) async -> Result<[SessionRecord], Error> {
        guard let repoPath else { return .success([]) }
        do {
            return .success(try await query.sessions(cwd: repoPath))
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
