import Foundation
import Loopflow
import Observation

struct WorkActivityScope: Equatable, Sendable {
    let wave: String?
    let project: String?
    let task: String?
}

enum TaskFlowControlRequest: Equatable, Sendable {
    case start(flow: String)
    case restart(flow: String)
    case resume
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

extension PodiumReading: Equatable where Value: Equatable {}

/// Readings keyed by planning Task id, so a late response can only settle
/// its own Task.
struct TaskReadings<Value> {
    fileprivate var values: [String: PodiumReading<Value>] = [:]
    private var generations: [String: Int] = [:]
    private(set) var inFlight: Set<String> = []

    subscript(taskId: String) -> PodiumReading<Value> { values[taskId] ?? .loading }

    fileprivate mutating func begin(_ taskId: String) -> Int {
        let generation = (generations[taskId] ?? 0) &+ 1
        generations[taskId] = generation
        inFlight.insert(taskId)
        return generation
    }

    /// Whether `generation` is still the newest read for its Task.
    fileprivate mutating func finish(_ taskId: String, generation: Int) -> Bool {
        guard generations[taskId] == generation else { return false }
        inFlight.remove(taskId)
        return true
    }
}

@MainActor
@Observable
final class PodiumModel {
    private(set) var taskLinkURL: URL?
    private(set) var taskLinkReading: PodiumReading<RoadmapSnapshot> = .loading
    var showsTaskLink = false
    @ObservationIgnored private var destinationGeneration = 0

    @ObservationIgnored private var taskLinkExpectedID: String?

    func openTaskLink(_ url: URL, expectedTaskID: String? = nil) async {
        destinationGeneration &+= 1
        let generation = destinationGeneration
        taskLinkURL = url
        taskLinkExpectedID = expectedTaskID
        taskLinkReading = .loading
        showsTaskLink = true
        do {
            let link = try TaskLink(url: url)
            let result = try await query.taskDestination(issue: link.issue, repo: link.repo)
            guard destinationGeneration == generation else { return }
            let matches = result.waves.flatMap { wave in wave.tasks.items.map { (wave, $0) } }
            let unavailable = result.waves.contains { $0.tasks.unavailableReason != nil }
            if let expectedTaskID {
                guard matches.allSatisfy({ $0.1.id == expectedTaskID && $0.0.wave.repo.normalizedFilePath == link.repo?.normalizedFilePath }) else {
                    throw RegistryQueryError("The recent Task no longer resolves to its recorded identity.")
                }
                if matches.isEmpty, !unavailable {
                    navigation.recentDestinations.removeAll { $0.id == .task(expectedTaskID) }
                }
            }
            taskLinkReading = .available(result)
            if matches.count == 1, !unavailable, let match = matches.first {
                openTaskDestination(wave: match.0, task: match.1)
            }
        } catch {
            guard destinationGeneration == generation else { return }
            taskLinkReading = .unavailable(lastGood: nil, reason: error.localizedDescription)
        }
    }

    func dismissTaskLink() {
        destinationGeneration &+= 1
        showsTaskLink = false
    }

    func openTaskDestination(wave: WaveRoadmap, task: RoadmapTask) {
        setRepoPath(wave.wave.repo)
        navigation.selectedTaskEvidence = (wave, task)
        select(.task(id: task.id))
        remember(.task(task.id))
    }

    func remember(_ destination: WorkspaceDestination) {
        guard let row = paletteRows.first(where: { $0.id == destination }) else { return }
        navigation.recentDestinations.removeAll { $0.id == destination }
        navigation.recentDestinations.insert(row, at: 0)
        navigation.recentDestinations = Array(navigation.recentDestinations.prefix(20))
    }

    func openPaletteTask(_ id: String) async {
        if let found = task(id: id) {
            openTaskDestination(wave: found.wave, task: found.task)
            return
        }
        guard let recent = navigation.recentDestinations.first(where: { $0.id == .task(id) }),
              let repoPath else { return }
        var components = URLComponents()
        components.scheme = "loopflow"
        components.host = "task"
        components.path = "/" + recent.key
        components.queryItems = [URLQueryItem(name: "repo", value: repoPath)]
        guard let url = components.url else { return }
        await openTaskLink(url, expectedTaskID: id)
    }

    func retryTaskLink() async {
        guard let taskLinkURL else { return }
        await openTaskLink(taskLinkURL, expectedTaskID: taskLinkExpectedID)
    }

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
        WorkspaceProjection(roadmaps: visibleRoadmaps, sessions: sessions.value ?? [])
    }
    var breadcrumb: WorkspaceBreadcrumb? {
        if let current = workspace.breadcrumb(selection: selection, sessionId: navigation.selectedSessionId) {
            return current
        }
        guard let retained = navigation.selectedTaskEvidence, selection == .task(id: retained.task.id) else { return nil }
        // Exact lookup evidence supports inspection without joining the current plan.
        return WorkspaceProjection(roadmaps: [retained.wave], sessions: sessions.value ?? [])
            .breadcrumb(selection: selection, sessionId: navigation.selectedSessionId)
    }

    /// Visibility never changes the inventory used by panes and selection.
    var visibleWorkspace: WorkspaceProjection {
        WorkspaceProjection(roadmaps: visibleRoadmaps, sessions: (sessions.value ?? []).filter {
            navigation.showsHeadlessSessions || $0.interactive
        })
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
    private(set) var activeSessions: PodiumReading<ActiveSessionsSnapshot> = .loading
    private(set) var isRefreshingActiveSessions = false
    private(set) var activeSessionsNeedsRetry = false
    @ObservationIgnored private var activeSessionsObservation: ActiveSessionsObservation?
    @ObservationIgnored private var activeSessionsTask: Task<Void, Never>?
    private var activeSessionsGeneration = 0
    private var activeSessionsDemanded = false
    private var sessionReadings: [String: PodiumReading<[SessionRecord]>] = [:]
    private(set) var sessions: PodiumReading<[SessionRecord]> {
        get { sessionReadings[repoPath ?? ""] ?? .loading }
        set { sessionReadings[repoPath ?? ""] = newValue }
    }
    private(set) var workActivity: PodiumReading<WorkActivitySnapshot> = .loading
    /// Selectable Flows per repository, read once on demand; never per repaint.
    private var flowCatalogReadings: [String: PodiumReading<[FlowCatalogEntry]>] = [:]
    var flowCatalog: PodiumReading<[FlowCatalogEntry]> {
        flowCatalogReadings[repoPath ?? ""] ?? .loading
    }
    /// Comment threads, read on demand for the shown Task.
    private(set) var comments = TaskReadings<TaskComments>()
    private(set) var taskWork = TaskReadings<TaskWork>()
    /// Conversation history, read only on disclosure.
    private(set) var recentRuns = TaskReadings<[SessionHistory]>()
    private(set) var taskContext = TaskReadings<ContextReport>()
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
        let nextWaves = reading(from: await wavesResult, lastGood: previousWaves)
        if waves != nextWaves { waves = nextWaves }
        await sessionRefresh
        let result = await roadmapResult
        if generation == roadmapGeneration {
            let nextRoadmap = reading(from: result, lastGood: previousRoadmap)
            if roadmap != nextRoadmap { roadmap = nextRoadmap }
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
        let next = reading(from: await readProcessActivity(), lastGood: previous)
        if processActivity != next { processActivity = next }
    }

    /// First demand starts a window-owned reader; navigation never restarts it.
    func observeActiveSessions() {
        guard !activeSessionsDemanded else { return }
        activeSessionsDemanded = true
        startActiveSessions()
    }

    func refreshActiveSessions() async {
        activeSessionsDemanded = true
        if activeSessionsNeedsRetry || activeSessionsTask == nil {
            await stopActiveSessions()
            startActiveSessions()
        } else {
            await activeSessionsObservation?.request(.refresh)
        }
    }

    func rescanActiveSessions() async {
        guard let observation = activeSessionsObservation, !activeSessionsNeedsRetry else { return }
        activeSessions = .unavailable(lastGood: activeSessions.value, reason: "Rediscovering active Sessions after wake")
        isRefreshingActiveSessions = true
        await observation.request(.rescan)
    }

    /// Attached once to the window root, independently of repository or pane visibility.
    func activeSessionsLifetime() async {
        do {
            while !Task.isCancelled { try await Task.sleep(for: .seconds(3600)) }
        } catch { }
        await stopActiveSessions()
        activeSessionsDemanded = false
    }

    func stopActiveSessions() async {
        activeSessionsGeneration += 1
        let generation = activeSessionsGeneration
        let task = activeSessionsTask
        task?.cancel()
        await task?.value
        guard activeSessionsGeneration == generation else { return }
        activeSessionsTask = nil
        activeSessionsObservation = nil
        isRefreshingActiveSessions = false
    }

    deinit { activeSessionsTask?.cancel() }

    private func startActiveSessions() {
        guard activeSessionsTask == nil else { return }
        activeSessionsGeneration += 1
        let generation = activeSessionsGeneration
        isRefreshingActiveSessions = true
        activeSessionsNeedsRetry = false
        let query = query
        activeSessionsTask = Task { [weak self] in
            // Only configuration replacement retries automatically. Transport failures
            // retain evidence and wait for the explicit Retry action.
            while !Task.isCancelled {
                var observation: ActiveSessionsObservation?
                var replace = false
                var discoveryFailed = false
                do {
                    let opened = try await query.watchActiveSessions()
                    observation = opened
                    guard !Task.isCancelled, self?.activeSessionsGeneration == generation else {
                        await opened.cancel()
                        return
                    }
                    self?.activeSessionsObservation = opened
                    for try await snapshot in opened.snapshots {
                        guard !Task.isCancelled, self?.activeSessionsGeneration == generation else { break }
                        self?.receiveActiveSessions(snapshot)
                        if snapshot.discovery == .unavailable {
                            discoveryFailed = true
                            break
                        }
                    }
                    if !Task.isCancelled && !discoveryFailed {
                        throw RegistryQueryError("Active Session observation ended")
                    }
                } catch ActiveSessionsObservationError.configurationChanged {
                    replace = true
                    if self?.activeSessionsGeneration == generation {
                        self?.activeSessions = .loading
                    }
                } catch {
                    if !Task.isCancelled, let self, self.activeSessionsGeneration == generation {
                        self.activeSessions = .unavailable(lastGood: self.activeSessions.value, reason: error.localizedDescription)
                        self.activeSessionsNeedsRetry = true
                    }
                }
                await observation?.cancel()
                guard !Task.isCancelled, let self, self.activeSessionsGeneration == generation else { return }
                self.activeSessionsObservation = nil
                if replace {
                    self.activeSessions = .loading
                    self.isRefreshingActiveSessions = true
                    continue
                }
                self.activeSessionsTask = nil
                self.isRefreshingActiveSessions = false
                return
            }
        }
    }

    private func receiveActiveSessions(_ snapshot: ActiveSessionsSnapshot) {
        isRefreshingActiveSessions = snapshot.discovery == .scanning
        activeSessionsNeedsRetry = snapshot.discovery == .unavailable
        switch snapshot.discovery {
        case .ready:
            activeSessions = .available(snapshot)
        case .scanning, .unavailable:
            let reason = snapshot.discovery == .scanning ? "Discovering active Sessions…" : "Active Session discovery unavailable"
            activeSessions = .unavailable(lastGood: activeSessions.value, reason: ([reason] + snapshot.gaps).joined(separator: "; "))
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
        dismissTaskLink()
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
        dismissTaskLink()
        historyLookup?.cancel()
        historyLookup = nil
        if let requested, requested.kind != .project {
            Perf.begin(Perf.taskWorkspaceReady, requested.kind == .task ? "task" : "wave", id: requested.id)
        }
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
            let result = try? await query.status(wave: wave.wave.name, cwd: wave.wave.repo)
            guard !Task.isCancelled else { return }
            guard let entries = result?.projects.items else { continue }
            if entries.contains(where: { $0.id == reference || $0.slug == reference || $0.workId == reference }) {
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
        guard let repoPath else {
            sessions = .available([])
            return
        }
        let initialIDs = Set((sessions.value ?? []).map(\.id))
        var records: [SessionRecord] = []
        var after: String?
        do {
            repeat {
                let page = try await query.sessionPage(includingHeadless: true, after: after, cwd: repoPath)
                guard sessionsGeneration == generation, self.repoPath == repoPath,
                      !Task.isCancelled else { return }
                records += page.entries
                let seen = Set(records.map(\.id))
                // Partial enumeration cannot establish absence. Keep existing panes
                // until the last page, including records added locally during this read.
                let retained = (sessions.value ?? []).filter {
                    !seen.contains($0.id) && (page.next != nil || !initialIDs.contains($0.id))
                }
                let next = PodiumReading.available(records + retained)
                if sessions != next { sessions = next }
                after = page.next
            } while after != nil
            if let selected = navigation.selectedSessionId,
               !(sessions.value ?? []).contains(where: { $0.id == selected }) {
                navigation.selectedSessionId = nil
            }
        } catch {
            guard sessionsGeneration == generation, self.repoPath == repoPath,
                  !Task.isCancelled else { return }
            sessions = .unavailable(lastGood: sessions.value, reason: error.localizedDescription)
        }
    }

    /// Read one Task's thread.
    func loadComments(task: RoadmapTask, wave: WaveSnapshot) async {
        await loadTaskReading(\.comments, task: task.id) { [query] in
            try await query.taskComments(id: task.id, wave: wave.name, cwd: WaveOrigin.resolve(wave.repo))
        }
    }

    func loadTaskWork(task: RoadmapTask, wave: WaveSnapshot) async {
        await loadTaskReading(\.taskWork, task: task.id) { [query] in
            try await query.taskWork(task: task.task.identifier, cwd: WaveOrigin.resolve(wave.repo))
        }
    }

    func loadRecentRuns(task: RoadmapTask, wave: WaveSnapshot) async {
        await loadTaskReading(\.recentRuns, task: task.id) { [query] in
            try await query.taskRuns(task: task.task.identifier, cwd: WaveOrigin.resolve(wave.repo))
        }
    }

    func loadTaskContext(task: RoadmapTask, wave: WaveSnapshot) async {
        await loadTaskReading(\.taskContext, task: task.id) { [query] in
            try await query.taskContext(task: task.task.identifier, cwd: WaveOrigin.resolve(wave.repo))
        }
    }

    /// A failure keeps the last good value, marked unavailable; an older
    /// response never replaces a newer one. Leaving the Task cancels its read,
    /// which is not evidence the read failed.
    private func loadTaskReading<Value: Sendable>(
        _ readings: ReferenceWritableKeyPath<PodiumModel, TaskReadings<Value>>,
        task taskId: String,
        read: () async throws -> Value
    ) async {
        let generation = self[keyPath: readings].begin(taskId)
        let result: Result<Value, Error>
        do { result = .success(try await read()) } catch { result = .failure(error) }
        guard self[keyPath: readings].finish(taskId, generation: generation) else { return }
        if case .failure = result, Task.isCancelled { return }
        self[keyPath: readings].values[taskId] = reading(from: result, lastGood: self[keyPath: readings][taskId].value)
    }

    /// Read the Flow catalogue for the current repository the first time a
    /// Task needs it, or again when the picker is opened (`force`).
    func loadFlowCatalog(force: Bool = false) async {
        let key = repoPath ?? ""
        if !force, flowCatalogReadings[key]?.value != nil { return }
        let previous = flowCatalogReadings[key]?.value
        let result: Result<[FlowCatalogEntry], Error>
        do { result = .success(try await query.flowCatalog(cwd: repoPath)) }
        catch { result = .failure(error) }
        flowCatalogReadings[key] = reading(from: result, lastGood: previous)
    }

    /// Run one Rust-owned Task Flow control, then refresh the shared reading.
    /// The outcome settles only the Task and repository that started it; a
    /// refusal is kept on that Task's draft and changes nothing else.
    func performFlowControl(_ control: TaskFlowControlRequest, task: RoadmapTask, wave: WaveSnapshot) async {
        let owner = navigation
        let taskId = task.id
        guard owner.flowDrafts[taskId]?.acting != true else { return }
        owner.flowDrafts[taskId, default: TaskFlowDraft()].acting = true
        owner.flowDrafts[taskId]?.error = nil
        let issue = task.task.identifier
        let cwd = WaveOrigin.resolve(wave.repo)
        do {
            switch control {
            case .start(let flow):
                try await query.runTaskFlow(issue: issue, flow: flow, cwd: cwd)
            case .restart(let flow):
                try await query.restartTaskFlow(issue: issue, flow: flow, cwd: cwd)
            case .resume:
                try await query.runTaskFlow(issue: issue, flow: nil, cwd: cwd)
            }
            owner.flowDrafts[taskId] = nil
            await refresh()
        } catch {
            owner.flowDrafts[taskId]?.acting = false
            owner.flowDrafts[taskId]?.error = error.localizedDescription
        }
    }

    func beginSessionRename(_ record: SessionRecord) {
        guard navigation.renaming?.sessionId != record.id else { return }
        navigation.renaming = SessionRenameDraft(sessionId: record.id, text: record.title)
    }

    /// Submit the current rename. Success publishes Rust's authoritative
    /// record; rejection keeps the typed name and its error. Either outcome
    /// settles only the Session and repository that started it.
    func commitSessionRename() async {
        guard let repo = repoPath, let draft = navigation.renaming, !draft.submitting else { return }
        let owner = navigation
        let target = draft.sessionId
        owner.renaming?.submitting = true
        owner.renaming?.error = nil
        do {
            let record = try await query.renameSession(id: target, name: draft.text, cwd: repo)
            // A read started before the rename must not restore the old name.
            sessionsGeneration &+= 1
            replaceSession(record, repo: repo)
            if owner.renaming?.sessionId == target { owner.renaming = nil }
        } catch {
            guard owner.renaming?.sessionId == target else { return }
            owner.renaming?.submitting = false
            owner.renaming?.error = error.localizedDescription
        }
    }

    func cancelSessionRename() {
        guard navigation.renaming?.submitting == false else { return }
        navigation.renaming = nil
    }

    func beginSessionBinding(_ record: SessionRecord) {
        guard navigation.binding == nil else { return }
        navigation.binding = SessionBindingDraft(sessionId: record.id, title: record.title)
    }

    func previewSessionBinding() async {
        guard let repo = repoPath, let draft = navigation.binding, !draft.submitting else { return }
        let owner = navigation
        owner.binding?.submitting = true
        owner.binding?.error = nil
        do {
            let preview = try await query.previewSessionBinding(
                id: draft.sessionId, task: draft.selector.trimmingCharacters(in: .whitespacesAndNewlines), cwd: repo)
            guard owner.binding?.id == draft.id else { return }
            owner.binding?.preview = preview
            owner.binding?.submitting = false
        } catch {
            guard owner.binding?.id == draft.id else { return }
            owner.binding?.submitting = false
            owner.binding?.error = error.localizedDescription
        }
    }

    func commitSessionBinding() async {
        guard let repo = repoPath, let draft = navigation.binding, !draft.submitting,
              let preview = draft.preview, preview.sessionId == draft.sessionId else { return }
        let owner = navigation
        owner.binding?.submitting = true
        owner.binding?.error = nil
        do {
            let record = try await query.bindSession(id: preview.sessionId, taskId: preview.taskId, cwd: repo)
            sessionsGeneration &+= 1
            replaceSession(record, repo: repo)
            if owner.selectedSessionId == record.id { owner.selection = record.work }
            if owner.binding?.id == draft.id { owner.binding = nil }
        } catch {
            guard owner.binding?.id == draft.id else { return }
            owner.binding?.submitting = false
            owner.binding?.error = error.localizedDescription
        }
    }

    func cancelSessionBinding() {
        guard navigation.binding?.submitting == false else { return }
        navigation.binding = nil
    }

    private func replaceSession(_ record: SessionRecord, repo: String) {
        let replace = { (records: [SessionRecord]) in
            records.map { $0.id == record.id ? record : $0 }
        }
        switch sessionReadings[repo] {
        case .available(let records):
            sessionReadings[repo] = .available(replace(records))
        case .unavailable(let records, let reason):
            sessionReadings[repo] = .unavailable(lastGood: records.map(replace), reason: reason)
        case .loading, nil:
            break
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
        roadmap.value?.waves.first { $0.currentProject?.id == projectId || $0.currentProject?.slug == projectId || $0.currentProject?.workId == projectId }
    }

    func task(id: String) -> (wave: WaveRoadmap, task: RoadmapTask)? {
        for wave in visibleRoadmaps {
            if let task = wave.tasks.items.first(where: { $0.id == id || $0.runtime?.workId == id }) {
                return (wave, task)
            }
        }
        if let retained = navigation.selectedTaskEvidence, retained.task.id == id {
            if let current = wave(id: retained.wave.wave.id),
               current.tasks.unavailableReason != nil || current.projects.unavailableReason != nil || current.projects.items.filter({ $0.status == .started }).count != 1 {
                return (current, retained.task)
            }
            return retained
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
        if selection.kind == .task, navigation.selectedTaskEvidence?.task.id == selection.id { return }
        // The repository's Session reading retains Work even when current
        // planning no longer contains it. Keep that conversation's selection.
        if let records = sessions.value {
            if records.contains(where: { $0.work == selection }) { return }
            if let selected = navigation.selectedSessionId,
               let session = records.first(where: { $0.id == selected }),
               session.work?.kind == .task,
               navigation.selectedTaskEvidence?.task.runtime?.workId == session.work?.id {
                return
            }
        }
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
        select(.wave(id: wave.wave.id))
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
