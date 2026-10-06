import Foundation
import Loopflow
import Observation

/// One Wave's streamed detail. `sequence` moves with each new reading.
struct StreamedWaveDetail: Sendable {
    let wave: String
    let sequence: Int
    let snapshot: WaveDetailSnapshot?
    let reason: String?
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

/// What the workspace is waiting on: the one loading vocabulary every surface uses.
enum WorkspaceStatus: Equatable {
    /// Nothing to show yet; only a first launch without a saved workspace.
    case loading
    /// The saved workspace is shown while this launch's first reads finish.
    case updating
    case current
    /// The last refresh failed; what is shown is the last good reading.
    case failed(String)

    var message: String? {
        switch self {
        case .loading: "Loading workspace…"
        case .updating: "Updating…"
        case .current: nil
        case .failed(let reason): reason
        }
    }
}

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
    var linkedSession: SessionRecord?
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
            if matches.count == 1, link.repo != nil || !unavailable, let match = matches.first {
                try await openLinkedTask(wave: match.0, task: match.1, link: link, generation: generation)
            }
        } catch {
            guard destinationGeneration == generation else { return }
            taskLinkReading = .unavailable(lastGood: nil, reason: error.localizedDescription)
        }
    }

    func dismissTaskLink() {
        destinationGeneration &+= 1
        showsTaskLink = false
        linkedSession = nil
    }

    func chooseLinkedTask(wave: WaveRoadmap, task: RoadmapTask) async {
        guard let taskLinkURL else { return }
        let generation = destinationGeneration
        do {
            try await openLinkedTask(wave: wave, task: task, link: TaskLink(url: taskLinkURL), generation: generation)
        } catch {
            guard destinationGeneration == generation else { return }
            taskLinkReading = .unavailable(lastGood: nil, reason: error.localizedDescription)
        }
    }

    private func openLinkedTask(wave: WaveRoadmap, task: RoadmapTask, link: TaskLink, generation: Int) async throws {
        guard let sessionID = link.session else {
            openTaskDestination(wave: wave, task: task)
            return
        }
        var after: String?
        var records: [SessionRecord] = []
        repeat {
            let page = try await query.sessionPage(includingHeadless: true, after: after, cwd: wave.wave.repo)
            guard destinationGeneration == generation else { return }
            records += page.entries
            after = page.next
        } while after != nil
        guard let taskID = task.runtime?.workId,
              let record = records.first(where: { $0.id == sessionID }),
              record.taskIds.contains(taskID) || record.workspace?.taskId == taskID else {
            throw RegistryQueryError("Session \(sessionID) was not found in Task \(task.task.identifier). Retry after its Session is available.")
        }
        openTaskDestination(wave: wave, task: task)
        supersedeSessions()
        sessions = .available(records)
        navigation.selectedSessionId = record.id
        navigation.content = .terminals
        linkedSession = record
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

    var taskHistoryFilters: [String: TaskHistoryFilter] = [:]
    var taskHistoryNow = Date()
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

    /// Visibility never changes Task membership or retained native surfaces.
    var visibleSessions: [SessionRecord] {
        (sessions.value ?? []).filter(isSessionVisible)
    }

    func isSessionVisible(_ session: SessionRecord) -> Bool {
        session.state != .closed && (navigation.showsHeadlessSessions || session.interactive)
    }

    var visibleWorkspace: WorkspaceProjection {
        WorkspaceProjection(roadmaps: visibleRoadmaps, sessions: visibleSessions)
    }

    private(set) var roadmap: PodiumReading<RoadmapSnapshot> = .loading {
        didSet {
            taskHistoryNow = Date()
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
    private(set) var sessionHistory = TaskReadings<[SessionHistory]>()
    private(set) var taskContext = TaskReadings<ContextReport>()
    private(set) var workActivityScope = WorkActivityScope(
        wave: nil,
        project: nil,
        task: nil
    )
    private(set) var repos: [PortfolioRepo] = []
    private(set) var authoredWavesByRepo: [String: [String]] = [:]
    private(set) var isRefreshing = false

    private var sessionSkillSelections = UserDefaults.standard.dictionary(forKey: "sessionSkillsByRepository") as? [String: String] ?? [:]

    var selectedSessionSkill: String {
        guard let repoPath else { return "capture-tasks" }
        return sessionSkillSelections[repoIdentity(repoPath)] ?? "capture-tasks"
    }

    func selectSessionSkill(_ name: String, repo: String) {
        var saved = UserDefaults.standard.dictionary(forKey: "sessionSkillsByRepository") as? [String: String] ?? [:]
        saved[repoIdentity(repo)] = name
        sessionSkillSelections = saved
        UserDefaults.standard.set(saved, forKey: "sessionSkillsByRepository")
    }

    func sessionSkills(repo: String) async throws -> [DiscoveryEntry] {
        try await query.sessionSkills(cwd: repo)
    }

    private let query: RegistryQuery
    @ObservationIgnored private let cache: WorkspaceCache?
    /// The Home the saved workspace was read from, until `confirmHome` checks it.
    @ObservationIgnored private(set) var savedHomeId: String?
    /// Parts still showing saved text instead of a read from this launch.
    private var showsSavedPlanning = false
    private var savedSessionRepos: Set<String> = []
    private var usesFixedFixture = false
    private var sessionsGeneration = 0
    private var roadmapGeneration = 0
    /// The Wave whose detail a view is showing, if any.
    var detailWaveId: String?
    /// The scoped Wave's `lf wave status` reading, as last streamed.
    private(set) var waveDetail: StreamedWaveDetail?
    /// Moves with every planning frame, for views that derive from planning.
    private(set) var planningSequence = 0
    @ObservationIgnored private var workspaceObservation: WorkspaceObservation?
    @ObservationIgnored private var workspaceHome: String?
    @ObservationIgnored private var workspaceOpened = ContinuousClock.now
    @ObservationIgnored private var nextRequestId = 0
    @ObservationIgnored private var sentScope: WorkspaceScope?
    @ObservationIgnored private var appliedSequence: [String: Int] = [:]
    /// Frames read before these requests predate a local change and are ignored.
    @ObservationIgnored private var planningFloor = 0
    @ObservationIgnored private var sessionsFloor = 0
    @ObservationIgnored private var scopeFloor = 0
    @ObservationIgnored private var planningWaiters: [(id: Int, resume: CheckedContinuation<Void, Never>)] = []
    private var processActivityRefreshInFlight = false
    private var workActivityGeneration = 0

    /// A launch repository taken from the saved workspace without running
    /// `git`; `refreshPortfolio` checks it off the main thread.
    @ObservationIgnored private var unverifiedRepoPath: String?

    init(query: RegistryQuery, repoPath: String? = nil, cache: WorkspaceCache? = nil) {
        self.query = query
        self.cache = cache
        self.repoPath = repoPath.map(WaveOrigin.resolve)
        if let saved = cache?.load() { restore(saved) }
    }

    /// A window's model, scoped to the first candidate that names a repository.
    /// One the saved workspace was last scoped to opens without running `git`.
    init(query: RegistryQuery, launchCandidates: [String], cache: WorkspaceCache?) {
        self.query = query
        self.cache = cache
        let saved = cache?.load()
        for candidate in launchCandidates {
            let path = candidate.normalizedFilePath
            if saved?.repositories[path] != nil {
                repoPath = path
                unverifiedRepoPath = path
                WaveOrigin.remember(origin: path)
            } else {
                repoPath = PortfolioDiscovery.resolveLaunchRepo(candidate)
            }
            if repoPath != nil { break }
        }
        if let saved { restore(saved) }
    }

    /// The model every window builds: saved workspace first, except in fixture
    /// and proof runs, which render only what they read.
    static func window(query: RegistryQuery, launchCandidates: [String] = []) -> PodiumModel {
        let model = PodiumModel(query: query, launchCandidates: launchCandidates,
                                cache: AppTestMode.current() == nil ? .home : nil)
        LaunchJournal.home.mark(.restored, ["cache": model.showsSavedWorkspace ? "hit" : "miss"])
        PodiumFixture.applyIfRequested(to: model)
        return model
    }

    var workspaceStatus: WorkspaceStatus {
        let sessionsError = repoPath == nil ? nil : sessions.errorMessage
        switch (roadmap.errorMessage, sessionsError) {
        case (let planning?, let sessions?):
            return .failed(planning == sessions
                ? "Couldn't update: \(planning)"
                : "Couldn't update planning: \(planning); Sessions: \(sessions)")
        case (let planning?, nil): return .failed("Couldn't update planning: \(planning)")
        case (nil, let sessions?): return .failed("Couldn't update Sessions: \(sessions)")
        case (nil, nil): break
        }
        if roadmap.isLoading || (repoPath != nil && sessions.isLoading) { return .loading }
        return showsSavedWorkspace ? .updating : .current
    }

    /// Whether any part shown is saved text instead of a read from this launch.
    var showsSavedWorkspace: Bool {
        showsSavedPlanning || savedSessionRepos.contains(repoPath ?? "")
    }

    /// Show the saved workspace before any read. Text that no longer decodes is skipped.
    private func restore(_ saved: WorkspaceSnapshot) {
        savedHomeId = saved.homeId
        if let text = saved.roadmap, let value = try? RegistryQuery.decode(RoadmapSnapshot.self, from: text) {
            roadmap = .available(value)
            showsSavedPlanning = true
        }
        if let text = saved.waves, let value = try? RegistryQuery.decode([WaveSnapshot].self, from: text) {
            waves = .available(value.map { $0.toWave() })
        }
        for (repo, entry) in saved.repositories {
            if let pages = entry.sessionPages,
               let records = try? pages.flatMap({ try RegistryQuery.decode(SessionPage.self, from: $0).entries }) {
                sessionReadings[repo] = .available(records)
                savedSessionRepos.insert(repo)
            }
            guard let selection = entry.selection, let waves = roadmap.value?.waves else { continue }
            let evidence = waves.lazy.compactMap { wave in
                wave.tasks.items.first { $0.id == selection.id }.map { (wave, $0) }
            }.first
            guard selection.kind == .task ? evidence != nil : waves.contains(where: { $0.wave.id == selection.id }) else { continue }
            let navigation = navigationByRepo[repo] ?? WorkspaceNavigation()
            navigationByRepo[repo] = navigation
            navigation.selection = selection
            navigation.selectedTaskEvidence = evidence
            navigation.content = .details
        }
    }

    private func endLaunchWhenCurrent() {
        guard workspaceStatus == .current else { return }
        Perf.end(Perf.workspaceCurrent, id: "launch")
        LaunchJournal.home.mark(.fresh)
    }

    /// Reads now come from `id`. A workspace saved under another Home is dropped
    /// unless this launch has already replaced it.
    func confirmHome(_ id: String) {
        if let savedHomeId, savedHomeId != id {
            if showsSavedPlanning {
                roadmap = .loading
                waves = .loading
                showsSavedPlanning = false
                // Work chosen from the other Home's rows names nothing here.
                for navigation in navigationByRepo.values {
                    navigation.selection = nil
                    navigation.selectedTaskEvidence = nil
                    navigation.content = .overview
                }
            }
            for repo in savedSessionRepos { sessionReadings[repo] = nil }
            savedSessionRepos = []
        }
        savedHomeId = id
        cache?.confirmHome(id)
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

    /// The window's one refresh owner: one reader process whose frames say
    /// what changed. Runs until cancelled, reopening the reader when it ends.
    func keepWorkspaceCurrent() async {
        if usesFixedFixture {
            if AppTestMode.current() == .sessionFixtures { await refreshSessions() }
            return
        }
        guard query.streamsWorkspace else {
            // This transport has no reader: one reading, shown until asked again.
            await refresh()
            await refreshProcessActivity()
            return
        }
        var delay = Duration.seconds(1)
        while !Task.isCancelled {
            var observation: WorkspaceObservation?
            do {
                let opened = try await query.watchWorkspace()
                observation = opened
                guard !Task.isCancelled else { await opened.cancel(); return }
                // A new reader numbers its frames from one and reads the store
                // as it is now, which includes every local write so far.
                workspaceObservation = opened
                workspaceOpened = .now
                appliedSequence = [:]
                (planningFloor, sessionsFloor, scopeFloor, sentScope) = (0, 0, 0, nil)
                syncWorkspaceScope()
                for try await frame in opened.frames {
                    guard !Task.isCancelled else { break }
                    await apply(frame)
                    if frame.content.part != "heartbeat" { delay = .seconds(1) }
                }
                if !Task.isCancelled { throw RegistryQueryError("Workspace observation ended") }
            } catch ActiveSessionsObservationError.configurationChanged {
                // The installed `lf` or its Home selection was replaced.
                delay = .milliseconds(100)
            } catch {
                if !Task.isCancelled { workspaceUnavailable(error.localizedDescription) }
            }
            workspaceObservation = nil
            await observation?.cancel()
            resumePlanningWaiters(through: .max)
            do { try await Task.sleep(for: delay) } catch { return }
            delay = min(delay * 2, .seconds(30))
        }
    }

    var workspaceScope: WorkspaceScope {
        let shown = selection.flatMap { $0.kind == .task ? task(id: $0.id) : nil }
        return WorkspaceScope(
            repo: repoPath,
            headless: navigation.showsHeadlessSessions,
            task: shown?.task.task.identifier,
            wave: detailWaveId,
            activity: activityScope(for: selection))
    }

    /// Tell the reader what this window shows now. Frames for the previous
    /// scope are ignored from here on.
    func syncWorkspaceScope() {
        guard workspaceObservation != nil else { return }
        let scope = workspaceScope
        guard scope != sentScope else { return }
        sentScope = scope
        scopeFloor = send { .scope(id: $0, scope) }
    }

    /// After sleep, events may have been missed.
    func rescanWorkspace() {
        guard workspaceObservation != nil else { return }
        _ = send { .refresh(id: $0) }
    }

    private func send(_ request: (Int) -> WorkspaceRequest) -> Int {
        nextRequestId += 1
        workspaceObservation?.request(request(nextRequestId))
        return nextRequestId
    }

    /// A local write changed Session rows; a frame read before it is stale.
    private func supersedeSessions() {
        sessionsGeneration &+= 1
        guard workspaceObservation != nil else { return }
        sessionsFloor = send { .refresh(id: $0) }
    }

    private func resumePlanningWaiters(through answered: Int) {
        let ready = planningWaiters.filter { $0.id <= answered }
        planningWaiters.removeAll { $0.id <= answered }
        for waiter in ready { waiter.resume.resume() }
    }

    /// The reader ended. What is shown stays, marked as no longer current.
    private func workspaceUnavailable(_ reason: String) {
        if let value = roadmap.value { roadmap = .unavailable(lastGood: value, reason: reason) }
        if let value = waves.value { waves = .unavailable(lastGood: value, reason: reason) }
        if repoPath != nil, let value = sessions.value { sessions = .unavailable(lastGood: value, reason: reason) }
        if let value = processActivity.value { processActivity = .unavailable(lastGood: value, reason: reason) }
    }

    private func apply(_ frame: WorkspaceFrame) async {
        let part = frame.content.part
        if case .heartbeat = frame.content { return }
        guard frame.sequence > appliedSequence[part] ?? 0 else { return }
        if let workspaceHome, workspaceHome != frame.home {
            // Another Home answers now; nothing shown from the previous one names anything here.
            dropHomeContent()
        }
        workspaceHome = frame.home
        let answers = frame.answers ?? 0
        let reason = frame.unavailable ?? "Workspace reader returned no \(part) reading"
        switch frame.content {
        case .planning(let body):
            guard answers >= planningFloor else { return }
            await applyPlanning(body, wire: frame.wire, reason: reason)
            planningSequence = frame.sequence
            resumePlanningWaiters(through: answers)
        case .sessions(let body):
            guard answers >= max(sessionsFloor, scopeFloor), let repoPath else { return }
            guard let body else {
                sessions = .unavailable(lastGood: sessions.value, reason: reason)
                break
            }
            guard body.repo.normalizedFilePath == repoPath.normalizedFilePath,
                  body.includesHeadless == navigation.showsHeadlessSessions else { return }
            let next = PodiumReading.available(body.entries)
            if sessions != next { sessions = next }
            if savedSessionRepos.remove(repoPath) != nil {
                LaunchJournal.home.refreshed("sessions", ms: workspaceOpened.elapsedMs, ok: true)
            }
            endLaunchWhenCurrent()
            // Explicit history is read on request, never restored at launch.
            if !body.includesHeadless, let page = frame.wire?.sessionPage {
                cache?.saveSessions([page], repo: repoPath)
            }
        case .task(let body):
            guard answers >= scopeFloor, let shown = selection, shown.kind == .task,
                  let task = task(id: shown.id)?.task else { return }
            guard let body else {
                taskWork.values[task.id] = .unavailable(lastGood: taskWork[task.id].value, reason: reason)
                break
            }
            guard body.task == task.task.identifier else { return }
            let next = PodiumReading.available(body.work)
            if taskWork[task.id] != next { taskWork.values[task.id] = next }
        case .wave(let body):
            guard answers >= scopeFloor, let detailWaveId else { return }
            guard body == nil || body?.wave == detailWaveId else { return }
            waveDetail = StreamedWaveDetail(wave: detailWaveId, sequence: frame.sequence,
                                           snapshot: body?.detail, reason: body == nil ? reason : nil)
        case .workActivity(let body):
            guard answers >= scopeFloor, let scope = activityScope(for: selection) else { return }
            guard let body else {
                workActivity = .unavailable(lastGood: workActivity.value, reason: reason)
                break
            }
            guard body.scope == scope else { return }
            workActivityScope = scope
            let next = PodiumReading.available(body.snapshot)
            if workActivity != next { workActivity = next }
        case .activity(let body):
            guard let body else {
                processActivity = .unavailable(lastGood: processActivity.value, reason: reason)
                break
            }
            await Self.resolveRepoOrigins(body.nodes.compactMap(\.repo))
            let next = PodiumReading.available(body)
            if processActivity != next { processActivity = next }
        case .heartbeat:
            break
        }
        appliedSequence[part] = frame.sequence
    }

    private func applyPlanning(_ body: WorkspaceFrame.Planning?, wire: WorkspaceFrame.Wire?, reason: String) async {
        taskHistoryNow = Date()
        guard let body else {
            roadmap = .unavailable(lastGood: roadmap.value, reason: reason)
            return
        }
        await Self.resolveRepoOrigins(body.waves.map(\.repo))
        let nextWaves = PodiumReading.available(body.waves.map { $0.toWave() })
        if waves != nextWaves { waves = nextWaves }
        let next = PodiumReading.available(body.roadmap)
        if roadmap != next { roadmap = next }
        if showsSavedPlanning || roadmapGeneration == 0 {
            LaunchJournal.home.refreshed("planning", ms: workspaceOpened.elapsedMs, ok: true)
        }
        // A one-shot read still in flight is older than this frame.
        roadmapGeneration &+= 1
        showsSavedPlanning = false
        if let text = wire?.roadmap { cache?.saveRoadmap(text) }
        if let text = wire?.waves { cache?.saveWaves(text) }
        endLaunchWhenCurrent()
        selectRequestedWaveIfNeeded()
        if visibleRoadmaps.allSatisfy({ wave in
            guard case .available(_, false) = wave.tasks else { return false }
            return wave.unavailableTasks.isEmpty
        }) {
            clearSelectionIfOutsideScope()
        }
        // Names in the scope come from planning.
        syncWorkspaceScope()
    }

    private func dropHomeContent() {
        roadmap = .loading
        waves = .loading
        showsSavedPlanning = false
        for navigation in navigationByRepo.values {
            navigation.selection = nil
            navigation.selectedTaskEvidence = nil
            navigation.content = .overview
        }
        sessionReadings = [:]
        savedSessionRepos = []
        waveDetail = nil
        taskWork = TaskReadings<TaskWork>()
    }

    /// One explicit read of everything, for a change the cadence should not wait on.
    func refresh() async {
        if workspaceObservation != nil {
            // Frames read before this request may predate the caller's write.
            let id = send { .refresh(id: $0) }
            (planningFloor, sessionsFloor) = (id, id)
            await withCheckedContinuation { planningWaiters.append((id, $0)) }
            return
        }
        async let sessions: Void = refreshSessions()
        await refreshPlanning()
        await sessions
    }

    func refreshPlanning() async {
        taskHistoryNow = Date()
        guard !usesFixedFixture else { return }
        guard !isRefreshing else { return }
        isRefreshing = true
        defer { isRefreshing = false }

        let started = ContinuousClock.now
        let previousRoadmap = roadmap.value
        let generation = roadmapGeneration
        let previousWaves = waves.value
        if previousRoadmap == nil { roadmap = .loading }
        if previousWaves == nil { waves = .loading }

        async let roadmapResult = readRoadmap()
        async let wavesResult = readWaves()
        let nextWaves = reading(from: await wavesResult, lastGood: previousWaves)
        if waves != nextWaves { waves = nextWaves }
        let result = await roadmapResult
        if generation == roadmapGeneration { publishRoadmap(result, lastGood: previousRoadmap) }
        LaunchJournal.home.refreshed("planning", ms: started.elapsedMs, ok: roadmap.errorMessage == nil)
        endLaunchWhenCurrent()
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
        guard workspaceObservation == nil else { return }
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
        if let trusted = unverifiedRepoPath {
            unverifiedRepoPath = nil
            let checked = await Task.detached { PortfolioDiscovery.resolveLaunchRepo(trusted) }.value
            if repoPath == trusted, checked != trusted { setRepoPath(checked) }
        }
        repos = discovered
        authoredWavesByRepo = await PortfolioDiscovery.authoredWaves(in: discovered)
        if repoPath == nil, let initialRepoPath {
            repoPath = PortfolioDiscovery.resolveLaunchRepo(initialRepoPath)
        }
        // Repository discovery cannot establish that selected Work was removed.
        // Planning refresh owns that reconciliation once its evidence is complete.
    }

    // Command ownership survives navigation. These are transport handles and feedback;
    // selected Project, readiness and execution outcomes arrive in workspace frames.
    private var projectCommands: [String: Task<Void, Never>] = [:]
    private(set) var projectCommandErrors: [String: String] = [:]

    @discardableResult
    func activateProject(id: String, name: String, repo: String) -> Task<Void, Never>? {
        guard !usesFixedFixture else { return nil }
        if let command = projectCommands[id] { return command }
        projectCommandErrors[id] = nil
        let command = Task {
            defer { projectCommands[id] = nil }
            do { try await query.ensureProject(wave: name, cwd: repo) }
            catch { projectCommandErrors[id] = error.localizedDescription }
        }
        projectCommands[id] = command
        return command
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
        if workspaceObservation != nil {
            await refresh()
            if let reason = roadmap.errorMessage {
                throw RegistryQueryError("Update accepted, but planning refresh failed: \(reason)")
            }
            guard roadmap.value?.waves.contains(where: { row in
                row.wave.id == wave.id && row.tasks.items.contains(where: { $0.id == task.id })
            }) == true else {
                throw RegistryQueryError("Update accepted, but the Task is absent from refreshed planning. Your draft is retained.")
            }
            return
        }
        // Polls started before this write must not restore the old directive.
        roadmapGeneration &+= 1
        let generation = roadmapGeneration
        let result = await readRoadmap()
        if generation == roadmapGeneration { publishRoadmap(result, lastGood: roadmap.value) }
        switch result {
        case .success(let (snapshot, _)):
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
        navigation.showsRetainedTerminals = false
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
        if workspaceObservation != nil {
            if activityScope(for: selection) == nil {
                workActivity = .unavailable(lastGood: nil, reason: "Selected Work is absent from the latest Podium evidence")
            }
            syncWorkspaceScope()
            return
        }
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
        if workspaceObservation != nil {
            syncWorkspaceScope()
            return
        }
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
        let includingHeadless = navigation.showsHeadlessSessions
        let wire = WireCapture()
        let query = query.recording(wire.record)
        let started = ContinuousClock.now
        do {
            repeat {
                let page = try await query.sessionPage(includingHeadless: includingHeadless, after: after, cwd: repoPath)
                guard sessionsGeneration == generation, self.repoPath == repoPath,
                      navigation.showsHeadlessSessions == includingHeadless,
                      !Task.isCancelled else { return }
                records += page.entries
                let seen = Set(records.map(\.id))
                // Keep prior rows until enumeration finishes, and preserve records
                // added locally during this read. Native panes have their own lifetime.
                let retained = (sessions.value ?? []).filter {
                    !seen.contains($0.id) && (page.next != nil || !initialIDs.contains($0.id))
                }
                let next = PodiumReading.available(records + retained)
                if sessions != next { sessions = next }
                after = page.next
            } while after != nil
            savedSessionRepos.remove(repoPath)
            LaunchJournal.home.refreshed("sessions", ms: started.elapsedMs, ok: true)
            endLaunchWhenCurrent()
            // Explicit history is read on request, never restored at launch.
            if !includingHeadless { cache?.saveSessions(wire.texts, repo: repoPath) }
        } catch {
            guard sessionsGeneration == generation, self.repoPath == repoPath,
                  !Task.isCancelled else { return }
            LaunchJournal.home.refreshed("sessions", ms: started.elapsedMs, ok: false)
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

    func loadSessionHistory(task: RoadmapTask, wave: WaveSnapshot) async {
        await loadTaskReading(\.sessionHistory, task: task.id) { [query] in
            try await query.taskHistory(task: task.task.identifier, cwd: WaveOrigin.resolve(wave.repo))
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
            supersedeSessions()
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
            supersedeSessions()
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
        supersedeSessions()
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
            if let current = wave(id: retained.wave.wave.id) {
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

    /// Every repository filter asks this once per Wave on each render, and one
    /// answer standardizes three file URLs. Answers hold until another origin
    /// is recorded.
    @ObservationIgnored private var repoIdentities: (revision: Int, byPath: [String: String]) = (0, [:])

    func repoIdentity(_ path: String) -> String {
        let revision = WaveOrigin.revision
        if repoIdentities.revision != revision { repoIdentities = (revision, [:]) }
        if let known = repoIdentities.byPath[path] { return known }
        let identity = WaveOrigin.cached(path.normalizedFilePath).normalizedFilePath
        repoIdentities.byPath[path] = identity
        return identity
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
        cache?.saveSelection(selection, repo: repoPath ?? "")
        if !usesFixedFixture { workActivity = .loading }
    }

    private func selectRequestedWaveIfNeeded() {
        guard selection == nil, let requested = AppTestMode.selectBranch else { return }
        guard let wave = visibleRoadmaps.first(where: { $0.wave.name == requested }) else { return }
        select(.wave(id: wave.wave.id))
    }

    /// The snapshot with the wire text it was decoded from.
    private func readRoadmap() async -> Result<(RoadmapSnapshot, String), Error> {
        let wire = WireCapture()
        do {
            let snapshot = try await query.recording(wire.record).roadmap()
            return .success((snapshot, wire.texts.last ?? ""))
        } catch {
            return .failure(error)
        }
    }

    /// A successful read replaces the saved workspace, on screen and on disk.
    private func publishRoadmap(_ result: Result<(RoadmapSnapshot, String), Error>, lastGood: RoadmapSnapshot?) {
        let next = reading(from: result.map(\.0), lastGood: lastGood)
        if roadmap != next { roadmap = next }
        guard case .success(let (_, text)) = result else { return }
        showsSavedPlanning = false
        cache?.saveRoadmap(text)
    }

    private func readWaves() async -> Result<[Wave], Error> {
        let wire = WireCapture()
        do {
            let waves = try await query.recording(wire.record).allWaves()
            await Self.resolveRepoOrigins(waves.map(\.repo))
            if let text = wire.texts.last { cache?.saveWaves(text) }
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
