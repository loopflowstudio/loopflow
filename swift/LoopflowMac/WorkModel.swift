import Foundation
import Loopflow
import Observation

/// One Wave's share of the planning reading. `sequence` moves with each
/// planning frame; a nil `snapshot` says why the Wave has no live reading.
struct StreamedWaveDetail: Sendable {
    let wave: String
    let sequence: Int
    let snapshot: WaveRoadmap?
    let reason: String?
}

enum WorkReading<Value> {
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

extension WorkReading: Equatable where Value: Equatable {}

/// What the workspace is waiting on: the one loading vocabulary every surface uses.
enum WorkReadingStatus: Equatable {
    /// Nothing to show yet; only a first launch without a saved workspace.
    case loading
    /// The saved workspace is shown while this launch's first reads finish.
    case updating
    case current
    /// The last refresh failed; what is shown is the last good reading.
    case failed(String)

    var message: String? {
        switch self {
        case .loading: "Loading work…"
        case .updating: "Updating…"
        case .current: nil
        case .failed(let reason): reason
        }
    }
}

/// Readings keyed by planning Task id, so a late response can only settle
/// its own Task.
struct TaskReadings<Value> {
    fileprivate var values: [String: WorkReading<Value>] = [:]
    private var generations: [String: Int] = [:]
    private(set) var inFlight: Set<String> = []

    subscript(taskId: String) -> WorkReading<Value> { values[taskId] ?? .loading }

    fileprivate mutating func begin(_ taskId: String) -> Int {
        let generation = (generations[taskId] ?? 0) &+ 1
        generations[taskId] = generation
        inFlight.insert(taskId)
        return generation
    }

    fileprivate mutating func receive(_ value: Value, for taskId: String) {
        generations[taskId] = (generations[taskId] ?? 0) &+ 1
        inFlight.remove(taskId)
        values[taskId] = .available(value)
    }

    /// Whether `generation` is still the newest read for its Task.
    fileprivate mutating func finish(_ taskId: String, generation: Int) -> Bool {
        guard generations[taskId] == generation else { return false }
        inFlight.remove(taskId)
        return true
    }
}

/// One resolved opening: its companion must never outlive or drift from the Session.
struct LinkedSession {
    let generation: Int
    let record: SessionRecord
    let task: RoadmapTask
    let showsChanges: Bool
    let location: TaskLocationObservation?
    var prepared = false
}

struct LinkedTaskPage: Equatable {
    let generation: Int
    let taskID: String
    let repo: String
}

@MainActor
@Observable
final class WorkModel {
    @ObservationIgnored private var workspaceRegistry: SessionsWorkspaceRegistry?
    @ObservationIgnored private var workspaceRepository: String?

    private var viewOpening: UUID?
    private var selectionOpening: DesktopOpening?
    var dashboardError: String?
    private(set) var dashboardSaving = false

    private var selectionReadings: [RepositoryReadingKey: WorkReading<[WorkViewPlan]>] = [:]
    private(set) var selectionPlans: WorkReading<[WorkViewPlan]> {
        get { selectionReadings[repositoryReadingKey(repoPath ?? "")] ?? .loading }
        set { selectionReadings[repositoryReadingKey(repoPath ?? "")] = newValue }
    }

    func applySelectionPlans(_ plans: [WorkViewPlan]?, reason: String) {
        guard let plans else {
            selectionPlans = .unavailable(lastGood: selectionPlans.value, reason: reason)
            return
        }
        let previous = selectionPlans.value ?? []
        let next = WorkReading.available(plans.map { plan in
            guard let id = plan.dashboardID, let old = previous.first(where: { $0.dashboardID == id }),
                  (old.savedRevision ?? 0) > (plan.savedRevision ?? 0) else { return plan }
            return old
        })
        // A write notifies even when equal. The sidebar, Dashboards and title
        // read these plans, so an unchanged frame must not republish them.
        if selectionPlans != next { selectionPlans = next }
        reconcileSelectionRepository()
        switch navigation.destination {
        case .dashboard, .workingTasks: if let plan = selectedViewPlan { presentView(plan) }
        case .work: presentWaveTiles()
        case .view, .entered: break
        }
    }

    private func reconcileSelectionRepository() {
        guard let registry = workspaceRegistry else { return }
        if let repository = selectionPlans.value?.first?.repository,
           let previous = workspaceRepository, previous != repository {
            registry.reassociateRepository(from: previous, to: repository)
            workspaceRepository = repository
        }
    }

    func workspaceKey(for reference: WorkReference?, repository requestedRepository: String) -> WorkWorkspaceKey {
        let repository = workspaceRepository ?? requestedRepository
        guard let reference else { return WorkWorkspaceKey(repository: repository, kind: .repository, id: repository) }
        if reference.kind == .task {
            let task = task(id: reference.id)?.task
            let key = WorkWorkspaceKey(repository: repository, kind: .task, id: task?.runtime?.workId ?? task?.id ?? reference.id)
            if let task {
                workspaceRegistry?.canonicalize(WorkWorkspaceKey(repository: repository, kind: .task, id: task.id), as: key)
            }
            return key
        }
        return WorkWorkspaceKey(repository: repository, kind: .wave, id: reference.id)
    }

    func workspaceTitle(_ key: WorkWorkspaceKey) -> String {
        let memberTitle = selectionPlans.value?.lazy.flatMap(\.contents).first { $0.work == key }?.title
        switch key.kind {
        case .repository: return repoPath.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Repository unavailable"
        case .wave: return visibleRoadmaps.first { $0.wave.id == key.id }?.wave.name ?? memberTitle ?? workspaceRegistry?.retained(key)?.title ?? "Wave unavailable"
        case .task: return task(id: key.id)?.task.task.name ?? memberTitle ?? workspaceRegistry?.retained(key)?.title ?? "Task unavailable"
        }
    }

    func bindWorkspaces(_ registry: SessionsWorkspaceRegistry, repository: String) {
        guard workspaceRegistry !== registry else { return }
        workspaceRegistry = registry
        let retainedRepository = registry.keys.first?.repository
        workspaceRepository = retainedRepository ?? repository
        reconcileSelectionRepository()
        guard retainedRepository != nil, taskOpening?.status != .opening else {
            openSelectedWorkspace()
            return
        }
        // Retained Work may all be hidden; nil selection must not reopen it.
        setSelection(registry.selectedWork?.reference)
        if let key = registry.selectedWork, let workspace = registry.retained(key) {
            navigation.workspacePresentation = workspace.presentation
            navigation.selectedSessionId = workspace.presentation.selectedSessionId
        }
    }

    func openSelectedWorkspace() {
        if let plan = selectedViewPlan { presentView(plan); return }
        guard let registry = workspaceRegistry, let repository = workspaceRepository else { return }
        let key = workspaceKey(for: selection, repository: repository)
        let location: WorkspaceIdentity?
        if key.kind == .task, let task = task(id: key.id)?.task {
            location = navigation.preparedTaskWorktrees[task.id] ?? task.reference.workspace?.identity
        } else if key.kind != .task, let machine = registry.localMachineId, let repoPath {
            location = WorkspaceIdentity(machineId: machine, worktree: repoPath)
        } else { location = nil }
        let workspace = registry.open(key, title: workspaceTitle(key), location: location)
        workspace.startPersistence(query: query, repoPath: repoPath)
        navigation.workspacePresentation = workspace.presentation
        navigation.selectedSessionId = workspace.presentation.selectedSessionId
        revealPrimaryConversation()
        presentWaveTiles()
    }

    /// Observation can display an existing conversation without acquiring its provider.
    func revealPrimaryConversation() {
        guard let registry = workspaceRegistry, let key = registry.selectedWork,
              key.kind != .task, let workspace = registry.retained(key),
              workspace.multiplexer.sessionPane?.sessionId == nil,
              let record = sessions.value?.first(where: {
                  key.kind == .repository ? $0.primaryScope == "repository"
                      : $0.primaryScope == "wave" && $0.waveId == key.id
              }) else { return }
        registry.revealSession(record, in: workspace)
        workspace.presentation.selectedSessionId = record.id
        workspace.presentation.content = .terminals
    }

    @ObservationIgnored var openRepository: ((String, LinkedWork?) -> Void)?

    private(set) var taskLink: TaskLink?
    private(set) var taskLinkReading: WorkReading<RoadmapSnapshot> = .loading
    var showsTaskLink = false
    private(set) var linkedTaskPage: LinkedTaskPage?
    private(set) var taskOpening: DesktopOpening?
    @ObservationIgnored private var destinationGeneration = 0

    @ObservationIgnored private var taskLinkExpectedID: String?
    @ObservationIgnored private var taskLinkConversation = false

    func openTaskLink(_ link: TaskLink, expectedTaskID: String? = nil) async {
        guard !Task.isCancelled else { return }
        dismissTaskLink()
        let generation = destinationGeneration
        taskOpening = DesktopOpening(url: link.address, status: .opening, reason: nil)
        defer {
            if Task.isCancelled, destinationGeneration == generation { dismissTaskLink() }
        }
        // Observation invalidates on every write, changed or not. Reopening
        // what is already open must not redraw the window.
        if taskLink != link { taskLink = link }
        taskLinkExpectedID = expectedTaskID
        taskLinkConversation = link.conversation
        if showsTaskLink { showsTaskLink = false }
        do {
            // Already observed planning opens directly, like the outline and palette.
            // Unscoped links and identity mismatches still resolve through a read.
            if let loaded = loadedTask(link), expectedTaskID == nil || loaded.task.id == expectedTaskID {
                try await openLinkedTask(wave: loaded.wave, task: loaded.task, link: link, generation: generation)
                return
            }
            taskLinkReading = .loading
            let result = try await query.taskDestination(issue: link.issue, repo: link.repo)
            guard !Task.isCancelled, destinationGeneration == generation else { return }
            let matches = result.tasks.items.map { task in (result.waves.first { $0.wave.id == task.task.waveId }, task) }
            let unavailable = result.tasks.unavailableReason != nil
            if let expectedTaskID {
                guard matches.allSatisfy({ $0.1.id == expectedTaskID && $0.1.task.repo.normalizedFilePath == link.repo?.normalizedFilePath }) else {
                    throw RegistryQueryError("The recent Task no longer resolves to its recorded identity.")
                }
                if matches.isEmpty, !unavailable {
                    navigation.recentDestinations.removeAll { $0.id == .task(expectedTaskID) }
                }
            }
            taskLinkReading = .available(result)
            if matches.count == 1, link.repo != nil || !unavailable, let match = matches.first {
                try await openLinkedTask(wave: match.0, task: match.1, link: link, generation: generation)
            } else {
                taskOpening = DesktopOpening(url: link.address, status: .failed, reason: "Task lookup is missing, ambiguous or unavailable; choose a repository-qualified Task.")
                showsTaskLink = true
            }
        } catch {
            Perf.agentInput("opening_failed", task: expectedTaskID ?? "")
            failTaskLink(error, link: link, generation: generation)
        }
    }

    func dismissTaskLink() {
        if let opening = taskOpening, opening.status == .opening {
            taskOpening = DesktopOpening(url: opening.url, status: .failed, reason: "Opening canceled by navigation or a newer request.")
        }
        destinationGeneration &+= 1
        if showsTaskLink { showsTaskLink = false }
        if linkedTaskPage != nil { linkedTaskPage = nil }
    }

    func chooseLinkedTask(wave: WaveRoadmap?, task: RoadmapTask) async {
        guard let taskLink else { return }
        dismissTaskLink()
        let generation = destinationGeneration
        taskOpening = DesktopOpening(url: taskLink.address, status: .opening, reason: nil)
        defer {
            if Task.isCancelled, destinationGeneration == generation { dismissTaskLink() }
        }
        do {
            try await openLinkedTask(wave: wave, task: task, link: taskLink, generation: generation)
        } catch {
            failTaskLink(error, link: taskLink, generation: generation)
        }
    }

    private func failTaskLink(_ error: Error, link: TaskLink, generation: Int) {
        guard !Task.isCancelled, destinationGeneration == generation else { return }
        taskOpening = DesktopOpening(url: link.address, status: .failed, reason: error.localizedDescription)
        taskLinkReading = .unavailable(lastGood: nil, reason: error.localizedDescription)
        // A sidebar opening already has its retained Task error and retry.
        // Only unresolved external links need the lookup sheet.
        let hasTaskPage = taskLinkConversation
            && taskLinkExpectedID.map { selection == .task(id: $0) } == true
        showsTaskLink = !hasTaskPage
    }

    /// The one Task a repository-qualified link names in planning this window holds.
    private func loadedTask(_ link: TaskLink) -> (wave: WaveRoadmap?, task: RoadmapTask)? {
        guard let repo = link.repo?.normalizedFilePath else { return nil }
        guard case .available(let snapshot) = roadmap else { return nil }
        let matches = snapshot.tasks.items.filter {
            $0.task.repo.normalizedFilePath == repo && ($0.task.identifier == link.issue || $0.id == link.issue || $0.runtime?.workId == link.issue)
        }.map { task in (snapshot.waves.first { $0.wave.id == task.task.waveId }, task) }
        return matches.count == 1 ? matches[0] : nil
    }

    private func openLinkedTask(wave: WaveRoadmap?, task: RoadmapTask, link: TaskLink, generation: Int) async throws {
        let opensConversation = link.session != nil || link.diff || taskLinkConversation
        let isLocalRepository = repoPath?.normalizedFilePath == task.task.repo.normalizedFilePath
        if opensConversation, isLocalRepository {
            openTaskDestination(wave: wave, task: task, preservingOpening: true)
            navigation.content = .terminals
        }
        let key = workspaceKey(for: .task(id: task.id), repository: workspaceRepository ?? "")
        // The Task's workspace carries the opening's spinner and error, retained or not.
        let workspace = opensConversation && isLocalRepository
            ? workspaceRegistry?.workspace(for: key, title: task.task.name, location: nil) : nil
        let request = UUID()
        workspace?.conversationRequest = request
        workspace?.conversationError = nil
        defer { if workspace?.conversationRequest == request { workspace?.conversationRequest = nil } }
        do {
            try await prepareLinkedTask(wave: wave, task: task, link: link, generation: generation)
        } catch {
            if destinationGeneration == generation { workspace?.conversationError = error.localizedDescription }
            throw error
        }
    }

    private func prepareLinkedTask(wave: WaveRoadmap?, task: RoadmapTask, link: TaskLink, generation: Int) async throws {
        try Task.checkCancellation()
        guard destinationGeneration == generation else { return }
        var task = task
        if let openRepository, repoPath?.normalizedFilePath != task.task.repo.normalizedFilePath {
            openRepository(task.task.repo, taskLink.map(LinkedWork.task))
            // The receiving window owns the opening now.
            taskOpening = nil
            return
        }
        var location: TaskLocationObservation?
        var executionQuery = query
        var executionTask = task.runtime?.workId ?? task.id
        var executionIdentity = navigation.preparedTaskWorktrees[task.id] ?? task.reference.workspace?.identity
        if let machine = link.machine, let repository = link.repository {
            let identity = try await query.repositoryIdentity(cwd: task.task.repo)
            guard identity.locators.contains(repository) else {
                throw RegistryQueryError("The repository identity changed; nothing was opened.")
            }
            executionQuery = query.onMachine(machine, repository: repository)
            let reading = try await executionQuery.taskLocation(task: executionTask, repository: repository, machine: machine)
            guard !Task.isCancelled, destinationGeneration == generation else { return }
            guard case .recorded(let remoteTask, let checkout) = reading.location, let checkout else {
                throw RegistryQueryError("The execution owner has no available recorded checkout; nothing was prepared or opened.")
            }
            location = reading
            executionTask = remoteTask
            executionIdentity = WorkspaceIdentity(machineId: machine, worktree: checkout)
        }
        guard link.session != nil || link.diff || taskLinkConversation else {
            if location != nil, let executionIdentity {
                navigation.preparedTaskWorktrees[task.id] = executionIdentity
            }
            openTaskDestination(wave: wave, task: task, preservingOpening: true)
            linkedTaskPage = LinkedTaskPage(generation: generation, taskID: task.id, repo: task.task.repo.normalizedFilePath)
            return
        }
        let sameRepo = repoPath?.normalizedFilePath == task.task.repo.normalizedFilePath
        let retainedRecords = sameRepo && location == nil ? sessions.value ?? [] : []
        let sessionID: String
        var preparation: SessionRecord?
        if let selected = link.session {
            sessionID = selected
        } else {
            // Reuse the primary-Session owner rather than creating another
            // conversation or replaying a retained terminal's launch command.
            Perf.agentInput("primary_begin", task: task.id)
            let record = try await executionQuery.prepareTaskSession(issue: executionTask, cwd: task.task.repo)
            Perf.agentInput("primary_returned", task: task.id, session: record.id, detail: "provider=\(record.provider ?? "unknown")")
            guard !Task.isCancelled, destinationGeneration == generation else { return }
            sessionID = record.id
            preparation = record
            // Preparation may have supplied a checkout/runtime absent from the
            // planning read. Refresh this exact Task, never the current selection.
            if location == nil {
                let refreshed = try await query.taskDestination(issue: task.task.identifier, repo: task.task.repo)
                guard !Task.isCancelled, destinationGeneration == generation else { return }
                guard let current = refreshed.tasks.items.first(where: { $0.id == task.id }) else {
                    throw RegistryQueryError("The prepared Task is no longer available; its Session was not retargeted.")
                }
                task = current
                executionTask = current.runtime?.workId ?? current.id
                executionIdentity = navigation.preparedTaskWorktrees[task.id] ?? task.reference.workspace?.identity
            }
        }
        var records: [SessionRecord] = []
        if let preparation {
            // Preparation already returned this Session's authoritative reading.
            // Opening one Task must not wait for every repository Session.
            records = [preparation]
        } else if link.session != nil, retainedRecords.contains(where: { $0.id == sessionID }) {
            records = retainedRecords
        } else {
            // An explicit Session link not held by this window still needs
            // lookup evidence; publish its inventory only after the last page.
            var after: String?
            repeat {
                let page = try await executionQuery.sessionPage(includingHeadless: true, after: after, cwd: task.task.repo)
                guard !Task.isCancelled, destinationGeneration == generation else { return }
                records += page.entries
                after = page.next
            } while after != nil
        }
        guard let record = records.first(where: { $0.id == sessionID }),
              record.taskIds.contains(executionTask) || record.workspace?.taskId == executionTask else {
            throw RegistryQueryError("Session \(sessionID) was not found in Task \(task.task.identifier). Retry after its Session is available.")
        }
        if location != nil {
            guard record.workspace?.identity == executionIdentity, record.workspace?.unavailable == nil else {
                throw RegistryQueryError("The Session no longer belongs to the observed Machine/checkout; nothing was opened.")
            }
        }
        if link.diff {
            guard let identity = record.workspace?.identity,
                  identity == executionIdentity else {
                throw RegistryQueryError("The Session and Task do not share a recorded checkout; no Changes pane was opened.")
            }
        }
        if location != nil, let executionIdentity { navigation.preparedTaskWorktrees[task.id] = executionIdentity }
        openTaskDestination(wave: wave, task: task, preservingOpening: true)
        supersedeSessions()
        if location != nil || preparation != nil {
            // One explicitly opened Session updates its own reading. Merge the
            // latest inventory: it may have refreshed during preparation.
            records = [record] + (sessions.value ?? []).filter { $0.id != record.id }
        }
        if preparation != nil, let reason = sessions.errorMessage {
            // One successful acquisition does not repair an unrelated inventory failure.
            sessions = .unavailable(lastGood: records, reason: reason)
        } else {
            sessions = .available(records)
        }
        let destination = LinkedSession(generation: generation, record: record, task: task, showsChanges: link.diff, location: location, prepared: preparation != nil)
        try await installConversation(destination, repository: task.task.repo)
    }

    /// Install the record and pane synchronously, before preparation can end.
    /// Native readiness is observed separately and never chooses navigation.
    private func installConversation(_ destination: LinkedSession, repository: String) async throws {
        guard let registry = workspaceRegistry, let repositoryID = workspaceRepository else {
            throw RegistryQueryError("The repository workspace is not available; retry opening it.")
        }
        guard registry.localMachineId != nil else {
            throw RegistryQueryError(registry.machineError ?? "Workspace Machine is unavailable; retry opening it.")
        }
        let store = registry.sessionStore(repository: repositoryID, repoPath: repository, query: query)
        let record = destination.record
        if let location = destination.location { store.retainLocation(location, for: record) }
        store.reconcile(sessions.value ?? [])
        let key = workspaceKey(for: .task(id: destination.task.id), repository: repositoryID)
        let target = registry.workspace(for: key, title: destination.task.task.name, location: record.workspace?.identity)
        let workspace = placeSession(record, in: target, store: store, registry: registry)
        let files = try registry.openLinkedFiles(destination, in: workspace.key, query: store.query)
        if files != nil, let zoomed = workspace.multiplexer.zoomedPaneId {
            workspace.multiplexer.setZoom(zoomed, enabled: false)
        }
        await connectLinkedOpening(destination, store: store, workspace: workspace, files: files)
    }

    /// Exact contextual links share the same synchronous pane placement as Task entry.
    func openSession(_ record: SessionRecord, store: SessionsStore) {
        guard let registry = workspaceRegistry, let repository = workspaceRepository else { return }
        if let retained = registry.key(containingSession: record.id) {
            select(retained.reference)
        } else {
            let selectedKey = workspaceKey(for: selection, repository: repository)
            if selection?.kind == .task,
               record.taskIds.contains(selectedKey.id) || record.workspace?.taskId == selectedKey.id {
                select(selection)
            } else if selection?.kind == .wave, record.waveId == selectedKey.id {
                select(selection)
            } else {
                select(store.taskSubject(for: record) ?? projection.subject(for: record.id))
            }
        }
        let key = workspaceKey(for: selection, repository: repository)
        let target = registry.workspace(for: key, title: workspaceTitle(key), location: record.workspace?.identity)
        store.reconcile([record])
        _ = placeSession(record, in: target, store: store, registry: registry)
        Task { await store.select(record.id) }
    }

    private func placeSession(_ record: SessionRecord, in target: SessionsWorkspace,
                              store: SessionsStore, registry: SessionsWorkspaceRegistry,
                              navigating: Bool = true) -> SessionsWorkspace {
        Perf.begin(Perf.taskWorkspaceReady, "session", id: record.id)
        let workspace: SessionsWorkspace
        if case .shell(let id, let machine) = store.localTerminal(for: record),
           let key = registry.key(containingShell: id, machineId: machine),
           let retained = registry.retained(key) {
            workspace = retained
            if navigating { select(key.reference, preservingOpening: true) }
            workspace.multiplexer.selectShell(id)
            store.surfaces.focus(.shell(id, machineId: machine))
        } else {
            workspace = target
            registry.revealSession(record, in: workspace)
            if let machine = record.workspace?.machineId {
                store.surfaces.focus(.session(record.id, machineId: machine))
            }
        }
        workspace.multiplexer.updateSessionLocation(record.id, location: record.workspace?.identity,
            execution: store.executionLocation(for: record.id))
        workspace.prepareTaskPanes(query: store.query)
        workspace.presentation.selectedSessionId = record.id
        workspace.presentation.content = .terminals
        store.beginPaneLoad(record.id)
        return workspace
    }

    /// Called by the requested Task's mounted page (or its unavailable surface).
    /// A callback from an older rendering of the same Task is not this request.
    func finishTaskPage(_ request: LinkedTaskPage, error: String? = nil) {
        guard !Task.isCancelled, linkedTaskPage == request, request.generation == destinationGeneration,
              repoPath?.normalizedFilePath == request.repo,
              let opening = taskOpening, opening.status == .opening else { return }
        taskOpening = DesktopOpening(url: opening.url, status: error == nil ? .usable : .failed, reason: error)
        linkedTaskPage = nil
    }

    /// Observe the existing Session and Files owners, never a second readiness store.
    /// Each callback retains the request generation, including repeated identical URLs.
    private func connectLinkedOpening(_ destination: LinkedSession, store: SessionsStore, workspace: SessionsWorkspace, files: TaskFilesStore?) async {
        guard destination.generation == destinationGeneration, taskOpening?.status == .opening else { return }
        let layout = workspace.multiplexer.layout
        let sessionPane: PaneState?
        if case .shell(let id, _) = store.localTerminal(for: destination.record) {
            sessionPane = workspace.multiplexer.shell(id) == nil ? nil : workspace.multiplexer.shellPane
        } else { sessionPane = layout.allPanes.first { $0.sessionId == destination.record.id } }
        var panes = [PaneState]()
        if let sessionPane { panes.append(sessionPane) }
        if destination.showsChanges, let pane = layout.pane(of: .browser),
           pane.content.selectedBrowserTab == .files { panes.append(pane) }
        guard panes.count == (destination.showsChanges ? 2 : 1) else {
            taskOpening = taskOpening.map { DesktopOpening(url: $0.url, status: .failed, reason: "Opening panes are no longer available.") }
            return
        }
        if destination.prepared { store.adoptPreparation(destination.record) }
        await store.select(destination.record.id)
        guard !Task.isCancelled, destination.generation == destinationGeneration, taskOpening?.status == .opening else { return }
        if let files { await files.refreshChanges() }
        guard !Task.isCancelled, destination.generation == destinationGeneration, taskOpening?.status == .opening else { return }
        observeOpeningOwners(destination, store: store, workspace: workspace, panes: panes, files: files)
    }

    private func observeOpeningOwners(_ destination: LinkedSession, store: SessionsStore, workspace: SessionsWorkspace, panes: [PaneState], files: TaskFilesStore?) {
        guard destination.generation == destinationGeneration, taskOpening?.status == .opening else { return }
        withObservationTracking {
            updateLinkedOpening(destination, store: store, workspace: workspace, panes: panes, files: files)
        } onChange: { [weak self, weak store, weak workspace, weak files] in
            Task { @MainActor in
                guard let self, let store, let workspace else { return }
                self.observeOpeningOwners(destination, store: store, workspace: workspace, panes: panes, files: files)
            }
        }
    }

    private func updateLinkedOpening(_ destination: LinkedSession, store: SessionsStore, workspace: SessionsWorkspace, panes: [PaneState], files: TaskFilesStore?) {
        guard destination.generation == destinationGeneration,
              let opening = taskOpening, opening.status == .opening else { return }
        // Observe the panes even before the Session reading arrives. Its absence
        // cannot hide a definitive close, replacement or visibility change.
        guard (workspaceRegistry == nil || workspaceRegistry?.selectedWork == workspace.key),
              panes.allSatisfy(workspace.multiplexer.isVisible) else {
            taskOpening = DesktopOpening(url: opening.url, status: .failed, reason: "Opening panes were hidden, closed or replaced.")
            return
        }
        guard let item = store.sessions.first(where: { $0.id == destination.record.id }) else { return }
        let reason: String?
        if item.record.workspace?.machineId == nil {
            reason = "Session Machine unavailable."
        } else if item.record.workspace?.identity != destination.record.workspace?.identity {
            reason = "The Session checkout changed while opening; retry its current location."
        } else {
            switch item.state {
            case .failed(let message), .ended(let message): reason = message
            case .pending where item.record.action(.open) == nil:
                reason = "Session has no available opening action."
            default: reason = files?.isRefreshingChanges == true ? nil : files?.changesError
            }
        }
        if let reason {
            taskOpening = DesktopOpening(url: opening.url, status: .failed, reason: reason)
        } else if item.state == .live || item.state == .elsewhere,
                  !destination.showsChanges || (files?.changes != nil && files?.isRefreshingChanges == false) {
            taskOpening = DesktopOpening(url: opening.url, status: .usable, reason: nil)
        }
    }

    func openTaskDestination(wave: WaveRoadmap?, task: RoadmapTask) {
        openTaskDestination(wave: wave, task: task, preservingOpening: false)
    }

    private func openTaskDestination(wave: WaveRoadmap?, task: RoadmapTask, preservingOpening: Bool) {
        if let openRepository, repoPath?.normalizedFilePath != task.task.repo.normalizedFilePath {
            openRepository(task.task.repo, .task(TaskLink(issue: task.task.identifier, repo: task.task.repo)))
            taskOpening = nil
            return
        }
        setRepoPath(task.task.repo, preservingOpening: preservingOpening)
        if navigation.selectedTaskEvidence.map({ $0.wave != wave || $0.task != task }) ?? true {
            navigation.selectedTaskEvidence = (wave, task)
        }
        // Reopening a Task must not clear its focused Session or retrigger entry.
        let isSelected = selection?.kind == .task
            && (selection?.id == task.id || selection?.id == task.runtime?.workId)
        if !isSelected { select(.task(id: task.id), preservingOpening: preservingOpening) }
        else { openSelectedWorkspace() }
        remember(.task(task.id))
    }

    func remember(_ destination: WorkDestination) {
        guard let row = paletteRows.first(where: { $0.id == destination }) else { return }
        let recent = Array(([row] + navigation.recentDestinations.filter { $0.id != destination }).prefix(20))
        if navigation.recentDestinations != recent { navigation.recentDestinations = recent }
    }

    func openTaskConversation(_ id: String) async {
        guard let found = task(id: id) else {
            guard let repoPath else { return }
            await openTaskLink(TaskLink(issue: id, repo: repoPath, conversation: true))
            return
        }
        let link = TaskLink(issue: found.task.task.identifier, repo: found.task.task.repo, conversation: true)
        let retainedConversation = showsTaskConversation(found.task)
        Perf.agentInput("workspace_selected", task: id, detail: "retained=\(retainedConversation)")
        openTaskDestination(wave: found.wave, task: found.task)
        navigation.content = .terminals
        if !retainedConversation {
            await openTaskLink(link, expectedTaskID: found.task.id)
        }
    }

    /// Whether the Task's retained workspace shows a Session, as its pane would.
    private func showsTaskConversation(_ task: RoadmapTask) -> Bool {
        guard let registry = workspaceRegistry, let repository = workspaceRepository, let repoPath,
              repoPath.normalizedFilePath == task.task.repo.normalizedFilePath,
              let workspace = registry.retained(workspaceKey(for: .task(id: task.id), repository: repository))
        else { return false }
        let held = Set(registry.sessionStore(repository: repository, repoPath: repoPath, query: query).sessions.map(\.id))
        return workspace.showsConversation(held: held)
    }

    /// Attach a Task's primary conversation in its retained workspace without
    /// navigating: a Dashboard tile shows it in place and the Dashboard stays.
    func attachTaskConversation(_ id: String, in workspace: SessionsWorkspace) async {
        guard let found = task(id: id), let registry = workspaceRegistry, let repository = workspaceRepository else { return }
        await workspace.openConversation {
            let record = try await query.prepareTaskSession(issue: found.task.runtime?.workId ?? found.task.id,
                                                            cwd: found.task.task.repo)
            // One opened Session updates its own reading; an unrelated inventory failure stays.
            let records = [record] + (sessions.value ?? []).filter { $0.id != record.id }
            sessions = sessions.errorMessage.map { .unavailable(lastGood: records, reason: $0) } ?? .available(records)
            let store = registry.sessionStore(repository: repository, repoPath: found.task.task.repo, query: query)
            store.reconcile([record])
            store.adoptPreparation(record)
            _ = placeSession(record, in: workspace, store: store, registry: registry, navigating: false)
        }
    }

    func openPaletteWave(_ id: String) {
        guard let wave = wave(id: id)?.wave else { return }
        if let openRepository, repoPath?.normalizedFilePath != wave.repo.normalizedFilePath {
            openRepository(wave.repo, .wave(id: id, address: nil))
        } else { select(.wave(id: id)) }
    }

    func receiveWorkLink(_ link: LinkedWork) async {
        switch link {
        case .task(let link): await openTaskLink(link)
        case .wave(let id, _):
            if wave(id: id) == nil { await refresh() }
            select(.wave(id: id))
        case .view(let expression, let address): await openView(expression: expression, url: URL(string: address))
        }
    }

    func openPaletteTask(_ id: String) async {
        if let task = roadmap.value?.tasks.items.first(where: { $0.id == id }),
           repoPath?.normalizedFilePath != task.task.repo.normalizedFilePath {
            let wave = roadmap.value?.waves.first { $0.wave.id == task.task.waveId }
            openTaskDestination(wave: wave, task: task)
            return
        }
        if let found = task(id: id) {
            openTaskDestination(wave: found.wave, task: found.task)
            return
        }
        guard let recent = navigation.recentDestinations.first(where: { $0.id == .task(id) }),
              let repoPath else { return }
        await openTaskLink(TaskLink(issue: recent.key, repo: repoPath), expectedTaskID: id)
    }

    func retryTaskLink() async {
        guard let taskLink else { return }
        await openTaskLink(taskLink, expectedTaskID: taskLinkExpectedID)
    }

    func inspectDesktop(repository: String, window: UUID, workspaces: SessionsWorkspaceRegistry) -> DesktopWindowInspection {
        if let previous = workspaceRepository, previous != repository {
            workspaces.reassociateRepository(from: previous, to: repository)
            workspaceRepository = repository
        }
        let reading: String
        switch workStatus {
        case .loading: reading = "loading"
        case .updating: reading = "updating"
        case .current: reading = "current"
        case .failed: reading = "unavailable"
        }
        return DesktopWindowInspection(repository: repository, window: window.uuidString,
            path: repoPath, destination: desktopDestination(repository: repository), history: desktopHistory,
            reading: reading, reason: workStatus.message,
            task: inspectSelectedTask(), session: inspectSelectedSession(),
            list: desktopList(repository: workspaceRepository ?? repository),
            actions: DesktopAction.all.map(\.descriptor), workspaces: workspaces.inspect(), openWork: workspaces.openWork,
            view: selectedViewPlan ?? navigation.waveView.flatMap { boardPlan($0) }, viewParts: desktopViewParts(repository: repository),
            opening: selectionOpening ?? taskOpening)
    }

    /// A Wave view's board, conversation and strip; other views have no named parts yet.
    func desktopViewParts(repository: String) -> [DesktopViewPart] {
        guard let view = navigation.waveView else { return [] }
        let doneDays = WaveStandupDefaults.doneDays
        let tiles = boardTiles(view)
        let pages = boardPlan(view)?.slots.pages(tiles) ?? [tiles]
        let page = min(navigation.wavePages[view] ?? 0, pages.count - 1)
        let ratios = navigation.waveRatios[view] ?? WaveViewRatios()
        let zoomed = zoomedPart(in: view)
        return [
            DesktopViewPart(name: "board", kind: .board, work: nil, section: boardSection(view).rawValue,
                            doneDays: doneDays, text: standupText(view, doneDays: doneDays),
                            page: nil, pages: nil, visible: [], hidden: isHidden(.board, in: view),
                            ratio: 1 - ratios.session, zoomed: zoomed == .board,
                            actions: ["section", "filter", "hide", "show", "zoom", "resize"]),
            DesktopViewPart(name: "session", kind: .session, work: viewWork(view, repository: workspaceRepository ?? repository),
                            section: nil, doneDays: nil, text: nil, page: nil, pages: nil, visible: [],
                            hidden: isHidden(.session, in: view), ratio: ratios.session, zoomed: zoomed == .session,
                            actions: []),
            DesktopViewPart(name: "tiles", kind: .strip, work: nil, section: nil, doneDays: nil, text: nil,
                            page: page + 1, pages: pages.count, visible: pages[page].map(\.work),
                            hidden: isHidden(.tiles, in: view), ratio: 1 - ratios.band, zoomed: zoomed == .tiles,
                            actions: ["page", "hide", "show", "zoom", "resize"]),
        ]
    }

    /// The Work whose conversation a Wave view shows.
    private func viewWork(_ view: WorkNavigation.Board, repository: String) -> WorkWorkspaceKey {
        if case .wave(let id) = view { WorkWorkspaceKey(repository: repository, kind: .wave, id: id) }
        else { WorkWorkspaceKey(repository: repository, kind: .repository, id: repository) }
    }

    /// The retained workspace of the Work this window shows.
    var shownWorkspace: SessionsWorkspace? {
        workspaceRegistry?.selectedWork.flatMap { workspaceRegistry?.retained($0) }
    }

    /// The Wave view's conversation workspace, once it is retained.
    private func viewWorkspace(_ view: WorkNavigation.Board) -> SessionsWorkspace? {
        guard let repository = workspaceRepository else { return nil }
        return workspaceRegistry?.retained(viewWork(view, repository: repository))
    }

    /// The board and strip hide in navigation; the conversation hides as its session
    /// pane does, so `lf desktop hide -p session` and the hover button agree.
    func isHidden(_ part: WaveViewPart, in view: WorkNavigation.Board) -> Bool {
        guard part == .session else { return navigation.hiddenViewParts[view, default: WaveViewPart.hiddenByDefault].contains(part) }
        guard let store = viewWorkspace(view)?.multiplexer, let pane = store.sessionPane else { return false }
        return store.collapsedPaneIds.contains(pane.id)
    }

    func setHidden(_ part: WaveViewPart, _ hidden: Bool, in view: WorkNavigation.Board) {
        // A hidden part is no longer zoomed; a collapsed session pane unzooms itself.
        guard part == .session else {
            if hidden {
                navigation.hiddenViewParts[view, default: WaveViewPart.hiddenByDefault].insert(part)
                if navigation.zoomedViewParts[view] == part { navigation.zoomedViewParts[view] = nil }
            } else { navigation.hiddenViewParts[view, default: WaveViewPart.hiddenByDefault].remove(part) }
            return
        }
        guard let workspace = viewWorkspace(view), let pane = workspace.multiplexer.sessionPane else { return }
        workspace.setCollapsed(paneId: pane.id, collapsed: hidden)
    }

    /// The part filling a Wave view. The board and strip zoom in navigation; the
    /// conversation zooms as its session pane, so `lf desktop zoom -p session` agrees.
    func zoomedPart(in view: WorkNavigation.Board) -> WaveViewPart? {
        if let part = navigation.zoomedViewParts[view] { return part }
        guard let store = viewWorkspace(view)?.multiplexer, let pane = store.sessionPane,
              store.zoomedPaneId == pane.id else { return nil }
        return .session
    }

    /// Zoom one part to fill the view, or return the view to its layout with nil.
    func setZoomed(_ part: WaveViewPart?, in view: WorkNavigation.Board) {
        navigation.zoomedViewParts[view] = part == .session ? nil : part
        guard let workspace = viewWorkspace(view), let pane = workspace.multiplexer.sessionPane else { return }
        let zoomed = part == .session
        if (workspace.multiplexer.zoomedPaneId == pane.id) != zoomed {
            workspace.multiplexer.setZoom(pane.id, enabled: zoomed)
        }
    }

    /// A Wave view's Tasks bucketed for its board, with owners' GitHub users and Sessions.
    func standup(_ tasks: [RoadmapTask], doneDays: Int) -> WaveStandup {
        var sessions: [String: WaveStandup.SessionAttention] = [:]
        for task in tasks {
            let open = taskSessions(task.id).filter { $0.state != .closed }
            if open.contains(where: { $0.availability == .available }) { sessions[task.id] = .available }
            else if !open.isEmpty { sessions[task.id] = .busy }
        }
        return WaveStandup(tasks: tasks, closed: tasks, doneDays: doneDays, now: .now, github: githubPeople,
                           sessions: sessions)
    }

    func boardSection(_ view: WorkNavigation.Board) -> WaveBoardSection {
        navigation.boardSections[view] ?? .backlog
    }

    /// The board as `lf desktop capture -p board` reads it.
    private func standupText(_ view: WorkNavigation.Board, doneDays: Int) -> String? {
        let scope = view == .repository ? "Repository" : "Wave"
        switch standupTasks(view) {
        case .available(let tasks, _):
            let heading = if case .wave(let id) = view, let roadmap = wave(id: id) { WaveStandup.heading(roadmap) }
                else { [repositoryName] }
            return standup(tasks, doneDays: doneDays).text(heading: heading, section: boardSection(view))
        case .unavailable(let reason): return "\(scope) planning unavailable · \(reason)"
        case nil: return nil
        }
    }

    /// The Tasks a Wave view's board buckets: the Wave's, or every Task in the repository.
    func standupTasks(_ view: WorkNavigation.Board) -> WorkEvidence<RoadmapTask>? {
        if case .wave(let id) = view { return wave(id: id)?.tasks }
        guard view == .repository, let tasks = roadmap.value?.tasks else { return nil }
        guard case .available(_, let truncated) = tasks else { return tasks }
        return .available(items: visibleTasks, truncated: truncated)
    }

    var repositoryName: String { repoPath.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Repository" }

    /// The Working Tasks ⌘1–9 select, in that order.
    func desktopList(repository: String) -> [DesktopListEntry] {
        workingTaskIDs.enumerated().map { index, id in
            let task = task(id: id)?.task
            return DesktopListEntry(position: index + 1,
                work: WorkWorkspaceKey(repository: repository, kind: .task, id: task?.id ?? id),
                identifier: task?.task.identifier, title: task?.task.name ?? id)
        }
    }

    /// The window's destination with durable Work keys, as `lf desktop state` reports it.
    func desktopDestination(repository: String) -> DesktopDestination {
        func key(_ work: WorkReference?) -> WorkWorkspaceKey {
            let repository = workspaceRepository ?? repository
            guard let work else { return WorkWorkspaceKey(repository: repository, kind: .repository, id: repository) }
            return work.kind == .task
                ? WorkWorkspaceKey(repository: repository, kind: .task, id: task(id: work.id)?.task.id ?? work.id)
                : WorkWorkspaceKey(repository: repository, kind: .wave, id: work.id)
        }
        switch navigation.destination {
        case .work(let work): return .work(key(work))
        case .dashboard(let id): return .dashboard(id: id)
        case .view(let plan): return .view(selection: plan.selection)
        case .workingTasks: return .workingTasks
        case .entered(.saved(let id), let work): return .entered(from: .dashboard(id), work: key(work))
        case .entered(.workingTasks, let work): return .entered(from: .workingTasks, work: key(work))
        case .entered(.wave(let id), let work): return .entered(from: .wave(id), work: key(work))
        case .entered(.repository, let work): return .entered(from: .repository, work: key(work))
        }
    }

    var desktopHistory: DesktopHistory {
        DesktopHistory(back: navigation.backStack.count, forward: navigation.forwardStack.count)
    }

    private func inspectSelectedTask() -> DesktopTaskInspection? {
        guard let selection, selection.kind == .task else { return nil }
        // Never use task(id:): its historical fallback remains useful for
        // display after removal, but is not a current action reading.
        let task = currentTask(id: selection.id)?.task
        let state: String
        let reason: String?
        switch roadmap {
        case .loading: (state, reason) = ("loading", "Planning has not been read")
        case .unavailable(_, let error): (state, reason) = ("unavailable", error)
        case .available where showsSavedPlanning:
            (state, reason) = ("saved", "Saved planning is not a current action reading")
        case .available where task == nil:
            (state, reason) = ("unavailable", "Selected Task is not available in the current planning reading")
        case .available: (state, reason) = ("current", nil)
        }
        return DesktopTaskInspection(id: selection.id, reading: state, reason: reason,
            roadmapGeneratedAt: roadmap.value?.generatedAt, conditionObservedAt: task?.condition.observedAt,
            actions: state == "current" ? task?.actions : nil,
            runControl: state == "current" ? task?.runControl : nil)
    }

    private func inspectSelectedSession() -> DesktopSessionInspection? {
        guard let id = navigation.selectedSessionId else { return nil }
        let record = sessions.value?.first { $0.id == id }
        let state: String
        let reason: String?
        switch sessions {
        case .loading: (state, reason) = ("loading", "Sessions have not been read")
        case .unavailable(_, let error): (state, reason) = ("unavailable", error)
        case .available where savedSessionRepos.contains(repositoryReadingKey(repoPath ?? "")):
            (state, reason) = ("saved", "Saved Sessions are not a current action reading")
        case .available where sessionsRefresh?.repo == repoPath:
            (state, reason) = ("updating", "Session enumeration is not complete")
        case .available where record == nil:
            (state, reason) = ("unavailable", "Selected Session is absent from the current Session reading")
        case .available: (state, reason) = ("current", nil)
        }
        return DesktopSessionInspection(id: id, machineId: record?.workspace?.machineId,
            reading: state, reason: reason, observedAt: nil, actions: state == "current" ? record?.actions : nil)
    }

    var historyWave: WaveSnapshot?
    var historyReference: String?
    @ObservationIgnored private(set) var historyLookup: Task<Void, Never>?
    var repoPath: String?
    var selection: WorkReference? { navigation.selection }
    @ObservationIgnored private var navigationByRepo: [String: WorkNavigation] = [:]

    var navigation: WorkNavigation {
        let key = repoPath ?? ""
        if let existing = navigationByRepo[key] { return existing }
        let state = WorkNavigation()
        navigationByRepo[key] = state
        return state
    }

    var projection: WorkProjection {
        WorkProjection(roadmaps: visibleRoadmaps, inventory: visibleTasks, sessions: sessions.value ?? [])
    }
    /// Built from the whole projection, so it is computed once per change to
    /// what it reads, not on each of the several reads per window render.
    var breadcrumb: WorkBreadcrumb? {
        _ = breadcrumbGeneration
        if let memo = breadcrumbMemo { return memo.value }
        let value = withObservationTracking { currentBreadcrumb() } onChange: { [weak self] in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.breadcrumbMemo = nil
                self.breadcrumbGeneration &+= 1
            }
        }
        breadcrumbMemo = BreadcrumbMemo(value: value)
        return value
    }
    private var breadcrumbGeneration = 0
    @ObservationIgnored private var breadcrumbMemo: BreadcrumbMemo?
    private struct BreadcrumbMemo { let value: WorkBreadcrumb? }

    private func currentBreadcrumb() -> WorkBreadcrumb? {
        // A Dashboard heads its own trail; no member Task or Session does.
        if navigation.showsSelection { return nil }
        if let current = projection.breadcrumb(selection: selection, sessionId: navigation.selectedSessionId) {
            return current
        }
        guard let retained = navigation.selectedTaskEvidence, selection == .task(id: retained.task.id) else { return nil }
        // Exact lookup evidence supports inspection without joining the current plan.
        return WorkProjection(roadmaps: [retained.wave].compactMap { $0 }, inventory: [retained.task], sessions: sessions.value ?? [])
            .breadcrumb(selection: selection, sessionId: navigation.selectedSessionId)
    }

    /// Visibility never changes Task membership or retained native surfaces.
    var visibleSessions: [SessionRecord] {
        (sessions.value ?? []).filter(isSessionVisible)
    }

    func isSessionVisible(_ session: SessionRecord) -> Bool {
        session.state != .closed && session.interactive
    }

    var visibleWork: WorkProjection {
        WorkProjection(roadmaps: visibleRoadmaps, inventory: visibleTasks, sessions: visibleSessions)
    }

    private(set) var roadmap: WorkReading<RoadmapSnapshot> = .loading {
        didSet {
            taskIndexCache = nil
            // Retain the latest observed Task across temporary chapter membership
            // gaps, including selections saved in another repository.
            for navigation in navigationByRepo.values {
                guard let selection = navigation.selection, selection.kind == .task else { continue }
                if let task = roadmap.value?.tasks.items.first(where: { $0.id == selection.id || $0.runtime?.workId == selection.id }) {
                    let wave = roadmap.value?.waves.first { $0.wave.id == task.task.waveId }
                    // A tuple is not Equatable, so an unchanged write would still notify.
                    if navigation.selectedTaskEvidence?.task != task || navigation.selectedTaskEvidence?.wave != wave {
                        navigation.selectedTaskEvidence = (wave, task)
                    }
                }
            }
        }
    }
    /// The latest row from a Task's own watch, current after the Task leaves
    /// the plan (completed, canceled).
    private var watchedTaskRow: RoadmapTask?
    private(set) var waves: WorkReading<[Wave]> = .loading
    private(set) var processActivity: WorkReading<ActivitySnapshot> = .loading
    private struct RepositoryReadingKey: Hashable {
        let machineId: String?
        let repository: String
    }
    private func repositoryReadingKey(_ repo: String) -> RepositoryReadingKey {
        RepositoryReadingKey(machineId: workMachine ?? savedMachineId, repository: repo)
    }
    private var sessionReadings: [RepositoryReadingKey: WorkReading<[SessionRecord]>] = [:]
    private var peerPlanningReadings: [RepositoryReadingKey: WorkReading<[PeerPlanningStatus]>] = [:]
    private(set) var peerPlanning: WorkReading<[PeerPlanningStatus]> {
        get { peerPlanningReadings[repositoryReadingKey(repoPath ?? "")] ?? .loading }
        set { peerPlanningReadings[repositoryReadingKey(repoPath ?? "")] = newValue }
    }
    private var peerPlanningGeneration = 0
    private(set) var sessions: WorkReading<[SessionRecord]> {
        get { sessionReadings[repositoryReadingKey(repoPath ?? "")] ?? .loading }
        set { sessionReadings[repositoryReadingKey(repoPath ?? "")] = newValue }
    }
    private(set) var workActivity: WorkReading<WorkActivitySnapshot> = .loading
    /// Selectable Graphs per repository, read once on demand; never per repaint.
    private var graphCatalogReadings: [String: WorkReading<[GraphCatalogEntry]>] = [:]
    @ObservationIgnored private var definitionRevision = 0
    @ObservationIgnored private var graphCatalogRevisions: [String: Int] = [:]
    @ObservationIgnored private var graphCatalogRequests: [String: UUID] = [:]
    private var graphCatalogKey: String { repoPath ?? "" }
    var graphCatalog: WorkReading<[GraphCatalogEntry]> {
        graphCatalogReadings[graphCatalogKey] ?? .loading
    }
    /// The same read, each graph seen as a Workflow.
    var workflowCatalog: WorkReading<[WorkflowCatalogEntry]> {
        switch graphCatalog {
        case .loading: .loading
        case .available(let entries): .available(entries.map(WorkflowCatalogEntry.init))
        case .unavailable(let lastGood, let reason):
            .unavailable(lastGood: lastGood?.map(WorkflowCatalogEntry.init), reason: reason)
        }
    }
    /// Comment threads, read on demand for the shown Task.
    private(set) var comments = TaskReadings<TaskComments>()
    /// The shown Task's work, from the workspace reader's `task` part.
    private(set) var taskWork = TaskReadings<TaskOverview>()
    /// Its recorded Sessions, Processes and Auto-Ship marks, from the same part.
    private(set) var taskExecution = TaskReadings<TaskExecution>()
    /// That Task's Graph processes keyed by Graph process, from the same part.
    private(set) var graphProcesses: [String: GraphProcessDetail] = [:]
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

    private let query: RegistryQuery
    @ObservationIgnored private let cache: WorkCache?
    /// The Machine the saved workspace was read from, until `confirmMachine` checks it.
    @ObservationIgnored private(set) var savedMachineId: String?
    var observingMachineId: String? { workMachine ?? savedMachineId }
    /// Parts still showing saved text instead of a read from this launch.
    private(set) var showsSavedPlanning = false
    private var savedSessionRepos: Set<RepositoryReadingKey> = []
    private var usesFixedFixture = false
    private var sessionsGeneration = 0
    @ObservationIgnored private var sessionsRefresh: (repo: String, generation: Int, task: Task<Void, Never>)?
    private var roadmapGeneration = 0
    /// Moves with every planning frame, for views that derive from planning.
    private(set) var planningSequence = 0
    @ObservationIgnored private var workObservation: WorkObservation?
    private var workMachine: String?
    private var workDataDirectory: String?
    @ObservationIgnored private var workOpened = ContinuousClock.now
    @ObservationIgnored private var nextRequestId = 0
    @ObservationIgnored private var sentScope: WorkScope?
    @ObservationIgnored private var appliedSequence: [String: Int] = [:]
    /// Frames read before these requests predate a local change and are ignored.
    @ObservationIgnored private var planningFloor = 0
    @ObservationIgnored private var sessionsFloor = 0
    @ObservationIgnored private var scopeFloor = 0
    @ObservationIgnored private var taskFloor = 0
    @ObservationIgnored private var planningWaiters: [(id: Int, resume: CheckedContinuation<Void, Never>)] = []
    @ObservationIgnored private var checkpointWaiters: [(id: Int, resume: CheckedContinuation<Void, any Error>, deadline: Task<Void, Never>)] = []
    private var workActivityGeneration = 0

    /// A launch repository taken from the saved workspace without running
    /// `git`; `applyPortfolio` checks it off the main thread.
    @ObservationIgnored private var unverifiedRepoPath: String?

    init(query: RegistryQuery, repoPath: String? = nil, cache: WorkCache? = nil) {
        self.query = query
        self.cache = cache
        self.repoPath = repoPath.map(WaveOrigin.resolve)
        if let saved = cache?.load() { restore(saved) }
    }

    /// A window's model, scoped to the first candidate that names a repository.
    /// One the saved workspace was last scoped to opens without running `git`.
    init(query: RegistryQuery, launchCandidates: [String], cache: WorkCache?) {
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
    static func window(query: RegistryQuery, launchCandidates: [String] = []) -> WorkModel {
        let model = WorkModel(query: query, launchCandidates: launchCandidates,
                                cache: AppTestMode.current() == nil ? .home : nil)
        LaunchJournal.home.mark(.restored, ["cache": model.showsSavedWork ? "hit" : "miss"])
        WorkFixture.applyIfRequested(to: model)
        return model
    }

    var workStatus: WorkReadingStatus {
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
        return showsSavedWork ? .updating : .current
    }

    /// Whether any part shown is saved text instead of a read from this launch.
    var showsSavedWork: Bool {
        showsSavedPlanning || savedSessionRepos.contains(repositoryReadingKey(repoPath ?? ""))
    }

    /// Show the saved workspace before any read. Text that no longer decodes is skipped.
    private func restore(_ saved: WorkSnapshot) {
        savedMachineId = saved.machineId
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
                sessionReadings[RepositoryReadingKey(machineId: saved.machineId, repository: repo)] = .available(records)
                savedSessionRepos.insert(RepositoryReadingKey(machineId: saved.machineId, repository: repo))
            }
            // Restore readings, not the previous launch's destination. Ordinary
            // launches start at the repository; explicit links select their Work.
        }
    }

    private func endLaunchWhenCurrent() {
        guard workStatus == .current else { return }
        Perf.end(Perf.workspaceCurrent, id: "launch")
        LaunchJournal.home.mark(.fresh)
    }

    /// Reads now come from `id`. A workspace saved under another Machine is dropped
    /// unless this launch has already replaced it.
    func confirmMachine(_ id: String) {
        if workMachine != id {
            sessionsGeneration &+= 1
            if workMachine != nil { dropMachineContent() }
        }
        workMachine = id
        if let savedMachineId, savedMachineId != id {
            if showsSavedPlanning {
                roadmap = .loading
                waves = .loading
                showsSavedPlanning = false
                // Work chosen from the other Machine's rows names nothing here.
                for navigation in navigationByRepo.values {
                    navigation.clearSelection()
                }
            }
            for repo in savedSessionRepos { sessionReadings[repo] = nil }
            savedSessionRepos = []
        }
        savedMachineId = id
        cache?.confirmMachine(id)
    }

    var visibleRoadmaps: [WaveRoadmap] {
        filterByRepo(roadmap.value?.waves ?? [], repo: { $0.wave.repo })
    }

    /// One Wave's detail, from the planning reading the window already watches.
    func waveDetail(id: String) -> StreamedWaveDetail? {
        guard !roadmap.isLoading else { return nil }
        let wave = roadmap.value?.waves.first { $0.wave.id == id }
        let reason = roadmap.errorMessage ?? (wave == nil ? "This Wave is not in the current plan" : nil)
        return StreamedWaveDetail(wave: id, sequence: planningSequence,
                                  snapshot: reason == nil ? wave : nil, reason: reason)
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
            result.append(repo)
        }
        return result.sorted {
            $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending
        }
    }

    /// The window's one refresh owner: one reader process whose frames say
    /// what changed. Runs until cancelled, reopening the reader when it ends.
    func keepWorkCurrent() async {
        if usesFixedFixture { return }
        guard query.streamsWork else {
            // This transport has no reader: one reading, shown until asked again.
            await refresh()
            return
        }
        var delay = Duration.seconds(1)
        while !Task.isCancelled {
            var observation: WorkObservation?
            do {
                let opened = try await query.watchWork()
                observation = opened
                guard !Task.isCancelled else { await opened.cancel(); return }
                // A new reader numbers its frames from one and reads the store
                // as it is now, which includes every local write so far.
                // Every reader reads the Machine's one store.
                workDataDirectory = ""
                workObservation = opened
                workOpened = .now
                appliedSequence = [:]
                definitionRevision = 0
                graphCatalogRevisions = [:]
                graphCatalogRequests = [:]
                (planningFloor, sessionsFloor, scopeFloor, taskFloor, sentScope) = (0, 0, 0, 0, nil)
                syncWorkScope()
                for try await frame in opened.frames {
                    guard !Task.isCancelled else { break }
                    Perf.begin(Perf.workFrameApply, frame.content.part, id: String(frame.sequence))
                    await apply(frame)
                    Perf.end(Perf.workFrameApply, id: String(frame.sequence))
                    delay = .seconds(1)
                }
                if !Task.isCancelled { throw RegistryQueryError("Workspace observation ended") }
            } catch WorkObservationError.configurationChanged {
                // The installed `lf` or its Machine selection was replaced.
                delay = .milliseconds(100)
            } catch {
                if !Task.isCancelled { workUnavailable(error.localizedDescription) }
            }
            workObservation = nil
            await observation?.cancel()
            resumePlanningWaiters(through: .max)
            finishCheckpoints(through: .max, reason: "Work observation ended before the update was confirmed")
            do { try await Task.sleep(for: delay) } catch { return }
            delay = min(delay * 2, .seconds(30))
        }
    }

    var workScope: WorkScope {
        let shown = selection.flatMap { $0.kind == .task ? task(id: $0.id) : nil }
        return WorkScope(
            repo: repoPath,
            headless: false,
            task: shown?.task.task.identifier,
            activity: activityScope(for: selection))
    }

    /// Tell the reader what this window shows now. Frames for the previous
    /// scope are ignored from here on.
    func syncWorkScope() {
        guard workObservation != nil else { return }
        let scope = workScope
        guard scope != sentScope else { return }
        sentScope = scope
        scopeFloor = send { .scope(id: $0, scope) }
    }

    /// After sleep, events may have been missed.
    func rescanWork() {
        guard workObservation != nil else { return }
        _ = send { .refresh(id: $0) }
    }

    private func send(_ request: (Int) -> WorkRequest) -> Int {
        nextRequestId += 1
        workObservation?.request(request(nextRequestId))
        return nextRequestId
    }

    /// A local write changed Session rows; a frame read before it is stale.
    private func supersedeSessions() {
        sessionsGeneration &+= 1
        guard workObservation != nil else { return }
        sessionsFloor = send { .checkpoint(id: $0, replay: ["sessions"]) }
    }

    private func finishCheckpoints(through answered: Int, reason: String?) {
        let ready = checkpointWaiters.filter { $0.id <= answered }
        checkpointWaiters.removeAll { $0.id <= answered }
        for waiter in ready {
            waiter.deadline.cancel()
            if let reason { waiter.resume.resume(throwing: RegistryQueryError(reason)) }
            else { waiter.resume.resume() }
        }
    }

    /// Confirmation follows the command's commit. Subscriptions choose what
    /// to read; an unrelated frame cannot confirm a saved draft.
    func confirmWorkUpdate(after home: String? = nil) async throws {
        if let home, home != workDataDirectory {
            throw RegistryQueryError("Machine changed before the update was confirmed")
        }
        guard workObservation != nil else {
            if query.streamsWork { throw RegistryQueryError("Update accepted; Work observation is unavailable") }
            // One-shot transports have no subscription to deliver their writes.
            await refresh()
            if let reason = roadmap.errorMessage { throw RegistryQueryError(reason) }
            return
        }
        let id = send { .checkpoint(id: $0, replay: []) }
        try await withCheckedThrowingContinuation { continuation in
            let deadline = Task { [weak self] in
                do { try await Task.sleep(for: .seconds(10)) } catch { return }
                guard let self, let index = self.checkpointWaiters.firstIndex(where: { $0.id == id }) else { return }
                let waiter = self.checkpointWaiters.remove(at: index)
                waiter.resume.resume(throwing: RegistryQueryError("Refresh was not confirmed within 10 seconds"))
            }
            checkpointWaiters.append((id, continuation, deadline))
        }
    }

    private func resumePlanningWaiters(through answered: Int) {
        let ready = planningWaiters.filter { $0.id <= answered }
        planningWaiters.removeAll { $0.id <= answered }
        for waiter in ready { waiter.resume.resume() }
    }

    /// The reader ended. What is shown stays, marked as no longer current.
    private func workUnavailable(_ reason: String) {
        roadmap = .unavailable(lastGood: roadmap.value, reason: reason)
        if let value = waves.value { waves = .unavailable(lastGood: value, reason: reason) }
        if repoPath != nil, let value = sessions.value { sessions = .unavailable(lastGood: value, reason: reason) }
        peerPlanningReadings = peerPlanningReadings.mapValues {
            .unavailable(lastGood: $0.value, reason: reason)
        }
        if repoPath != nil { applySelectionPlans(nil, reason: reason) }
        if repoPath != nil { peerPlanning = .unavailable(lastGood: peerPlanning.value, reason: reason) }
        if let value = processActivity.value { processActivity = .unavailable(lastGood: value, reason: reason) }
        taskExecution.values = taskExecution.values.mapValues { .unavailable(lastGood: $0.value, reason: reason) }
        taskWork.values = taskWork.values.mapValues { .unavailable(lastGood: $0.value, reason: reason) }
        if let selection, selection.kind == .task {
            taskWork.values[selection.id] = .unavailable(lastGood: taskWork[selection.id].value, reason: reason)
        }
    }

    private func apply(_ frame: WorkFrame) async {
        let part = frame.content.part
        if case .checkpoint = frame.content {
            // A confirmed command may have changed Workflow definitions.
            await observeDefinitions()
            finishCheckpoints(through: frame.answers ?? 0, reason: frame.unavailable)
            return
        }
        guard frame.sequence > appliedSequence[part] ?? 0 else { return }
        let answers = frame.answers ?? 0
        let reason = frame.unavailable ?? "Work reader returned no \(part) reading"
        switch frame.content {
        case .planning(let body):
            guard answers >= planningFloor else { return }
            await applyPlanning(body, wire: frame.wire, reason: reason)
            if body != nil, answers >= scopeFloor { await refreshSelectedView() }
            planningSequence = frame.sequence
            resumePlanningWaiters(through: answers)
        case .selections(let body):
            guard answers >= max(planningFloor, scopeFloor) else { return }
            applySelectionPlans(body, reason: reason)
            if body != nil { await refreshSelectedView() }
        case .peerPlanning(let body):
            guard answers >= max(planningFloor, scopeFloor), let repoPath else { return }
            guard let body else {
                peerPlanning = .unavailable(lastGood: peerPlanning.value, reason: reason)
                break
            }
            guard body.repo.normalizedFilePath == repoPath.normalizedFilePath else { return }
            peerPlanningGeneration &+= 1
            let next = WorkReading.available(body.destinations)
            if peerPlanning != next { peerPlanning = next }
        case .sessions(let body):
            guard answers >= max(sessionsFloor, scopeFloor), let repoPath else { return }
            guard let body else {
                sessions = .unavailable(lastGood: sessions.value, reason: reason)
                break
            }
            guard body.repo.normalizedFilePath == repoPath.normalizedFilePath,
                  !body.includesHeadless else { return }
            let remote = (sessions.value ?? []).filter {
                guard let machine = $0.workspace?.machineId else { return false }
                return workMachine == nil || machine != workMachine
            }
            let next = WorkReading.available(body.entries + remote)
            if sessions != next { sessions = next }
            if savedSessionRepos.remove(repositoryReadingKey(repoPath)) != nil {
                LaunchJournal.home.refreshed("sessions", ms: workOpened.elapsedMs, ok: true)
            }
            endLaunchWhenCurrent()
            if let page = frame.wire?.sessionPage {
                cache?.saveSessions([page], repo: repoPath)
            }
        case .task(let body):
            // A frame about its own Task stays true after the selection moves on.
            if let body, let row = body.row, row.task.identifier == body.task, watchedTaskRow != row {
                watchedTaskRow = row
            }
            guard answers >= max(taskFloor, scopeFloor), let shown = selection, shown.kind == .task,
                  let task = task(id: shown.id)?.task else { return }
            guard let body else {
                taskWork.values[task.id] = .unavailable(lastGood: taskWork[task.id].value, reason: reason)
                comments.values[task.id] = .unavailable(lastGood: comments[task.id].value, reason: reason)
                taskExecution.values[task.id] = .unavailable(lastGood: taskExecution[task.id].value, reason: reason)
                break
            }
            guard body.task == task.task.identifier else { return }
            if comments[task.id] != .available(body.comments) { comments.values[task.id] = .available(body.comments) }
            let next = WorkReading.available(body.work)
            if taskWork[task.id] != next { taskWork.values[task.id] = next }
            let runs = Dictionary(body.graphProcesses.map { ($0.entry.id, $0) }) { _, newer in newer }
            if graphProcesses != runs { graphProcesses = runs }
            if let execution = body.execution, taskExecution[task.id] != .available(execution) {
                taskExecution.values[task.id] = .available(execution)
            }
        case .workActivity(let body):
            guard answers >= scopeFloor, let scope = activityScope(for: selection) else { return }
            guard let body else {
                workActivity = .unavailable(lastGood: workActivity.value, reason: reason)
                break
            }
            guard body.scope == scope else { return }
            if workActivityScope != scope { workActivityScope = scope }
            let next = WorkReading.available(body.snapshot)
            if workActivity != next { workActivity = next }
        case .activity(let body):
            guard let body else {
                processActivity = .unavailable(lastGood: processActivity.value, reason: reason)
                break
            }
            await Self.resolveRepoOrigins(body.nodes.compactMap(\.repo))
            let next = WorkReading.available(body)
            if processActivity != next { processActivity = next }
        case .checkpoint:
            break
        }
        appliedSequence[part] = frame.sequence
        // A changed plan may come with changed Workflow definitions.
        if part == WorkPart.planning.rawValue { await observeDefinitions() }
    }

    private func observeDefinitions() async {
        definitionRevision &+= 1
        if graphCatalogReadings[graphCatalogKey] != nil { await loadGraphCatalog() }
    }

    private func applyPlanning(_ body: WorkFrame.Planning?, wire: WorkFrame.Wire?, reason: String) async {
        // Failed stream readings supersede older one-shot requests too.
        roadmapGeneration &+= 1
        guard let body else {
            roadmap = .unavailable(lastGood: roadmap.value, reason: reason)
            return
        }
        await Self.resolveRepoOrigins(body.waves.map(\.repo))
        let reportsLaunch = showsSavedPlanning || roadmapGeneration == 1
        guard publishRoadmap(.success((body.roadmap, wire?.roadmap)), lastGood: roadmap.value) else { return }
        reconcileSelectionRepository()
        let nextWaves = WorkReading.available(body.waves.map { $0.toWave() })
        if waves != nextWaves { waves = nextWaves }
        if reportsLaunch {
            LaunchJournal.home.refreshed("planning", ms: workOpened.elapsedMs, ok: true)
        }
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
        syncWorkScope()
    }

    private func dropMachineContent() {
        applySelectionPlans(nil, reason: "Work reading changed; showing retained membership")
        selectionReadings = [:]
        definitionRevision = 0
        graphCatalogRevisions = [:]
        graphCatalogRequests = [:]
        graphCatalogReadings = [:]
        roadmap = .loading
        waves = .loading
        showsSavedPlanning = false
        for navigation in navigationByRepo.values {
            navigation.clearSelection()
        }
        sessionReadings = [:]
        peerPlanningReadings = [:]
        peerPlanningGeneration &+= 1
        savedSessionRepos = []
        taskWork = TaskReadings<TaskOverview>()
        taskExecution = TaskReadings<TaskExecution>()
        graphProcesses = [:]
    }

    /// Explicit Retry/recovery, or a one-shot transport without subscriptions.
    func refresh() async {
        if workObservation != nil {
            // Frames read before this request may predate the caller's write.
            let id = send { .refresh(id: $0) }
            (planningFloor, sessionsFloor, taskFloor) = (id, id, id)
            await withCheckedContinuation { planningWaiters.append((id, $0)) }
            return
        }
        async let sessions: Void = refreshSessions()
        async let peers: Void = refreshPeerPlanning()
        await refreshPlanning()
        await sessions
        await peers
    }

    private func refreshPeerPlanning() async {
        guard !usesFixedFixture, let repo = repoPath else { return }
        peerPlanningGeneration &+= 1
        let generation = peerPlanningGeneration
        let key = repositoryReadingKey(repo)
        let result: Result<[PeerPlanningStatus], Error>
        do { result = .success(try await query.peerPlanningStatus(cwd: repo)) }
        catch { result = .failure(error) }
        guard generation == peerPlanningGeneration, repo == repoPath, key == repositoryReadingKey(repo) else { return }
        peerPlanning = reading(from: result, lastGood: peerPlanning.value)
    }

    func refreshPlanning() async {
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

    /// Task owners' GitHub users by lowercase email, read once per repository.
    private(set) var githubPeople: [String: GitHubPerson] = [:]
    private var githubPeopleRepo: String?

    /// Without GitHub the board keeps owners' names and initials.
    func loadGitHubPeople() async {
        guard !usesFixedFixture, let repo = repoPath, repo != githubPeopleRepo else { return }
        githubPeopleRepo = repo
        guard let people = try? await query.repoPeople(cwd: repo), repo == repoPath else { return }
        githubPeople = Dictionary(people.compactMap { person in person.github.map { (person.email.lowercased(), $0) } },
                                  uniquingKeysWith: { first, _ in first })
    }

    private var portfolioGeneration = 0

    /// Consume the shared membership; only authored Wave discovery is window-local.
    func applyPortfolio(_ discovered: [PortfolioRepo], initialRepoPath: String?) async {
        guard !usesFixedFixture, !Task.isCancelled else { return }
        portfolioGeneration &+= 1
        let generation = portfolioGeneration
        await Self.resolveRepoOrigins(discovered.map(\.path))
        let authored = await PortfolioDiscovery.authoredWaves(in: discovered)
        guard generation == portfolioGeneration, !Task.isCancelled else { return }
        if let trusted = unverifiedRepoPath {
            unverifiedRepoPath = nil
            let checked = await Task.detached { PortfolioDiscovery.resolveLaunchRepo(trusted) }.value
            if repoPath == trusted, checked != trusted { setRepoPath(checked) }
        }
        guard generation == portfolioGeneration, !Task.isCancelled else { return }
        repos = discovered
        authoredWavesByRepo = authored
        if repoPath == nil, let initialRepoPath {
            repoPath = PortfolioDiscovery.resolveLaunchRepo(initialRepoPath)
        }
        // Repository discovery cannot establish that selected Work was removed.
        // Planning refresh owns that reconciliation once its evidence is complete.
    }

    // Command ownership survives navigation. These are transport handles and feedback;
    // selected Project, readiness and execution outcomes arrive in Work frames.
    private var projectCommands: [String: Task<Void, Never>] = [:]
    private(set) var projectCommandErrors: [String: String] = [:]

    func isProjectActivationPending(id: String) -> Bool {
        projectCommands[id] != nil
    }

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
        setRepoPath(path, preservingOpening: false)
    }

    /// The router has observed the same durable repository at a new address.
    /// Retain navigation and native owners; refresh readers in their new scope.
    func relocateRepository(to path: String) {
        guard repoPath != path else { return }
        historyLookup?.cancel()
        historyLookup = nil
        sessionsGeneration &+= 1
        peerPlanningGeneration &+= 1
        workActivityGeneration &+= 1
        repoPath = path
        rescanWork()
    }

    private func setRepoPath(_ path: String?, preservingOpening: Bool) {
        if navigation.palette == .search || navigation.palette == .sidebarSearch { navigation.palette = nil }
        if let openRepository, let path,
           repoPath?.normalizedFilePath != WaveOrigin.resolve(path).normalizedFilePath {
            openRepository(path, nil)
            return
        }
        if !preservingOpening { dismissTaskLink() }
        let path = path.map(WaveOrigin.resolve)
        if repoPath?.normalizedFilePath != path?.normalizedFilePath {
            historyLookup?.cancel()
            historyLookup = nil
            sessionsGeneration &+= 1
            peerPlanningGeneration &+= 1
            workActivityGeneration &+= 1
            if !usesFixedFixture { workActivity = .loading }
        }
        // Navigation is already scoped to this repository. A partial planning
        // read cannot invalidate its saved selection merely because we return.
        if repoPath != path { repoPath = path }
    }


    func updateTaskDirective(task: RoadmapTask, text: String) async throws {
        let home = workDataDirectory
        try await query.updateTaskDirective(id: task.id, text: text, cwd: task.task.repo)
        let observed: WorkReading<RoadmapSnapshot>
        if query.streamsWork {
            try await confirmWorkUpdate(after: home)
            observed = roadmap
        } else {
            // Polls started before this write must not restore the old directive.
            roadmapGeneration &+= 1
            let generation = roadmapGeneration
            let result = await readRoadmap()
            if generation == roadmapGeneration { publishRoadmap(result, lastGood: roadmap.value) }
            observed = reading(from: result.map { $0.0 }, lastGood: nil)
        }
        if let reason = observed.errorMessage {
            throw RegistryQueryError("Update accepted, but planning refresh failed: \(reason)")
        }
        guard observed.value?.tasks.items.contains(where: { $0.id == task.id }) == true else {
            throw RegistryQueryError("Update accepted, but the Task is absent from refreshed planning. Your draft is retained.")
        }
    }

    /// A Task on the shown Dashboard or Wave view keeps that view in the trail, as its
    /// tile does; the strip turns to the Task's page.
    func select(_ requested: WorkReference?) {
        if let requested, requested.kind == .task, let board = navigation.trailBoard,
           taskIDs(on: board).contains(task(id: requested.id)?.task.id ?? requested.id) {
            return enter(requested, from: board)
        }
        select(requested, preservingOpening: false)
    }

    /// Working Tasks: the repository plan's started, uncompleted Tasks, in plan order.
    var workingTaskIDs: [String] { taskIDs(in: builtInPlan(.repository)?.resolution.members ?? []) }

    /// The Wave the sidebar lists, while its planning is current.
    var sidebarWave: WaveRoadmap? {
        navigation.sidebarWaveID.flatMap { id in visibleRoadmaps.first { $0.wave.id == id } }
    }

    /// The sidebar scope's members: its built-in plan's, or a Dashboard's visible ones in arrangement order.
    private var sidebarMembers: [WorkMemberRef] {
        switch navigation.sidebarScope {
        case .repository: return builtInPlan(.repository)?.resolution.members ?? []
        case .wave(let id): return builtInPlan(.wave(id))?.resolution.members ?? []
        case .dashboard(let id): return dashboard(id)?.projection.visible ?? []
        }
    }

    /// Every Wave at the repository; a scoped Wave's child Waves; a Dashboard's Wave members.
    var sidebarWaves: [WaveRoadmap] {
        guard navigation.sidebarScope != .repository else { return visibleRoadmaps }
        return sidebarMembers.filter { $0.work.kind == .wave }
            .compactMap { member in visibleRoadmaps.first { $0.wave.id == member.work.id } }
    }

    private func builtInPlan(_ expression: WorkSelection.Expression) -> WorkViewPlan? {
        selectionPlans.value?.first { $0.dashboardID == nil && $0.selection.expression == expression }
    }

    /// A Dashboard's visible Tasks in arrangement order; Working Tasks and a Wave's in plan order.
    private func taskIDs(on board: WorkNavigation.Board) -> [String] {
        taskIDs(in: boardTiles(board))
    }

    /// Members name Tasks by Work ID; rows use the planning ID when it is known.
    private func taskIDs(in members: [WorkMemberRef]) -> [String] {
        var seen = Set<String>()
        return members.filter { $0.work.kind == .task }
            .map { task(id: $0.work.id)?.task.id ?? $0.work.id }
            .filter { seen.insert($0).inserted }
    }

    var sidebarTaskIDs: [String] {
        taskIDs(in: sidebarMembers).filter { id in
            switch navigation.sidebarFilter {
            case .working: return true
            case .available:
                guard sessions.errorMessage == nil else { return false }
                return taskSessions(id).contains { $0.availability == .available && $0.state != .closed }
            case .busy:
                if case .reported(_, let report) = SessionProgramStatus(sessions: taskSessions(id), unavailable: sessions.errorMessage) { return report.state == .working }
                return false
            }
        }
    }

    /// A Wave row's count: its board's Working and Autonomous columns, not Backlog or Done.
    func startedTaskCount(_ wave: WaveRoadmap) -> Int {
        let standup = WaveStandup(tasks: wave.tasks.items, closed: [], doneDays: 0, now: .now, github: [:], sessions: [:])
        return standup.working.count + standup.autonomous.count
    }

    /// Below the Working Tasks: the scope's Autonomous Tasks, as the Wave board buckets
    /// them (shipping, no one's turn). Dashboards and Available/Busy filters have none.
    var sidebarAutonomousTaskIDs: [String] {
        guard navigation.sidebarFilter == .working else { return [] }
        let board: WorkNavigation.Board
        switch navigation.sidebarScope {
        case .repository: board = .repository
        case .wave(let id): board = .wave(id)
        case .dashboard: return []
        }
        let tasks = standupTasks(board)?.items ?? []
        let working = Set(sidebarTaskIDs)
        return WaveStandup(tasks: tasks, closed: [], doneDays: 0, now: .now, github: [:], sessions: [:])
            .autonomous.map(\.task.id).filter { !working.contains($0) }
    }

    func taskSessions(_ id: String) -> [SessionRecord] {
        let runtimeID = task(id: id)?.task.runtime?.workId ?? id
        return (sessions.value ?? []).filter {
            $0.taskIds.contains(runtimeID) || $0.workspace?.taskId == runtimeID
        }
    }

    func openPrimaryConversation(in workspace: SessionsWorkspace, wave: String?, store: SessionsStore) async {
        guard let workspaceRegistry else { return }
        await workspace.openPrimaryConversation(wave: wave, store: store, registry: workspaceRegistry)
    }

    /// ⌘N: the Nth Task row on screen.
    func selectSidebarTask(slot: Int) {
        let ids = sidebarTaskIDs
        guard ids.indices.contains(slot) else { return }
        select(.task(id: ids[slot]))
    }

    /// ⌃⇥: the next Task row on screen, wrapping.
    func cycleSidebarTask(_ offset: Int) {
        let ids = sidebarTaskIDs
        guard !ids.isEmpty else { return }
        let index = ids.firstIndex { isSelected(.task(id: $0)) }
        let next = index.map { ($0 + offset + ids.count) % ids.count } ?? (offset > 0 ? 0 : ids.count - 1)
        select(.task(id: ids[next]))
    }

    func goBack() {
        guard let previous = navigation.backStack.popLast() else { return }
        navigation.forwardStack.append(navigation.destination)
        selectDestination(previous)
    }

    func goForward() {
        guard let next = navigation.forwardStack.popLast() else { return }
        navigation.backStack.append(navigation.destination)
        selectDestination(next)
    }

    func isSelected(_ reference: WorkReference) -> Bool {
        if selection == reference { return true }
        guard let selection, selection.kind == .task, reference.kind == .task else { return false }
        return task(id: selection.id)?.task.id == task(id: reference.id)?.task.id
            && task(id: selection.id) != nil
    }

    private func select(_ requested: WorkReference?, preservingOpening: Bool, recordingHistory: Bool = true) {
        if navigation.palette == .search || navigation.palette == .sidebarSearch { navigation.palette = nil }
        if !preservingOpening { dismissTaskLink() }
        historyLookup?.cancel()
        historyLookup = nil
        if let requested, requested.kind == .task {
            Perf.begin(Perf.workflowReady, "task", id: requested.id)
        }
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
        if recordingHistory, navigation.destination != .work(selection) {
            navigation.backStack.append(navigation.destination)
            navigation.forwardStack = []
        }
        if workspaceRegistry == nil {
            navigation.selectedSessionId = nil
            navigation.content = selection == nil ? .overview : .details
        }
        if navigation.showsSelection { workspaceRegistry?.clearPresentation() }
        setSelection(selection)
        if selection == nil { navigation.sidebarWaveID = nil }
        if let selection, selection.kind == .wave { navigation.sidebarWaveID = selection.id }
        clearSelectionIfOutsideScope()
        openSelectedWorkspace()
    }

    private func openHistoricalReference(_ reference: String) async {
        for wave in visibleRoadmaps {
            guard !Task.isCancelled else { return }
            let result = try? await query.status(wave: wave.wave.name, cwd: wave.wave.repo)
            guard !Task.isCancelled else { return }
            guard let entries = result?.projects.items else { continue }
            if entries.contains(where: { $0.id == reference || $0.slug == reference || $0.linearId == reference }) {
                select(.wave(id: wave.wave.id))
                historyReference = reference
                historyWave = wave.wave
                return
            }
        }
    }

    func refreshWorkActivity() async {
        guard !usesFixedFixture else { return }
        if workObservation != nil {
            if activityScope(for: selection) == nil {
                workActivity = .unavailable(lastGood: nil, reason: "Selected Work is absent from the latest Work evidence")
            }
            syncWorkScope()
            return
        }
        workActivityGeneration &+= 1
        let generation = workActivityGeneration
        let requestedSelection = selection
        guard let scope = activityScope(for: requestedSelection) else {
            workActivity = .unavailable(
                lastGood: nil,
                reason: "Selected Work is absent from the latest Work evidence"
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
        guard !usesFixedFixture else { return }
        if workObservation != nil {
            syncWorkScope()
            return
        }
        guard let repoPath else {
            sessions = .available([])
            return
        }
        // The periodic reader and full refresh share one enumeration. Joining
        // also lets callers wait for all pages instead of invalidating each other.
        if let refresh = sessionsRefresh,
           refresh.repo == repoPath, refresh.generation == sessionsGeneration {
            await refresh.task.value
            return
        }
        sessionsGeneration &+= 1
        let generation = sessionsGeneration
        let task = Task { await readSessions(repoPath: repoPath, generation: generation) }
        sessionsRefresh = (repoPath, generation, task)
        await task.value
        if sessionsRefresh?.generation == generation { sessionsRefresh = nil }
    }

    private func readSessions(repoPath: String, generation: Int) async {
        let key = repositoryReadingKey(repoPath)
        let initialIDs = Set((sessions.value ?? []).map(\.id))
        var records: [SessionRecord] = []
        var after: String?
        let wire = WireCapture()
        let query = query.recording(wire.record)
        let started = ContinuousClock.now
        do {
            repeat {
                let page = try await query.sessionPage(includingHeadless: false, after: after, cwd: repoPath)
                guard sessionsGeneration == generation, self.repoPath == repoPath,
                      repositoryReadingKey(repoPath) == key,
                      !Task.isCancelled else { return }
                records += page.entries
                let seen = Set(records.map(\.id))
                // Keep prior rows until enumeration finishes, and preserve records
                // added locally during this read. Native panes have their own lifetime.
                let retained = (sessions.value ?? []).filter {
                    !seen.contains($0.id) && (page.next != nil || !initialIDs.contains($0.id) || (workMachine != nil && $0.workspace?.machineId != nil && $0.workspace?.machineId != workMachine))
                }
                let next = WorkReading.available(records + retained)
                if sessions != next { sessions = next }
                after = page.next
            } while after != nil
            savedSessionRepos.remove(repositoryReadingKey(repoPath))
            LaunchJournal.home.refreshed("sessions", ms: started.elapsedMs, ok: true)
            endLaunchWhenCurrent()
            cache?.saveSessions(wire.texts, repo: repoPath)
        } catch {
            guard sessionsGeneration == generation, self.repoPath == repoPath,
                      repositoryReadingKey(repoPath) == key,
                  !Task.isCancelled else { return }
            LaunchJournal.home.refreshed("sessions", ms: started.elapsedMs, ok: false)
            let next = WorkReading.unavailable(lastGood: sessions.value, reason: error.localizedDescription)
            if sessions != next { sessions = next }
        }
    }

    func loadSessionHistory(task: RoadmapTask) async {
        await loadTaskReading(\.sessionHistory, task: task.id) { [query] in
            try await query.taskHistory(task: task.task.identifier, cwd: WaveOrigin.resolve(task.task.repo))
        }
    }

    func loadTaskContext(task: RoadmapTask) async {
        await loadTaskReading(\.taskContext, task: task.id) { [query] in
            try await query.taskContext(task: task.task.identifier, cwd: WaveOrigin.resolve(task.task.repo))
        }
    }

    /// A failure keeps the last good value, marked unavailable; an older
    /// response never replaces a newer one. Leaving the Task cancels its read,
    /// which is not evidence the read failed.
    private func loadTaskReading<Value: Sendable>(
        _ readings: ReferenceWritableKeyPath<WorkModel, TaskReadings<Value>>,
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

    /// File definitions have no store revision. Returning from an editor
    /// rereads only catalogues this window has opened.
    func rereadDefinitions() async {
        if graphCatalogReadings[graphCatalogKey] != nil { await loadGraphCatalog(force: true) }
    }

    /// Read the Graph catalogue for the current repository the first time it
    /// is needed, again after a stored definition change, or when `force`d.
    /// The Workflow catalogue is the same read.
    func loadGraphCatalog(force: Bool = false) async {
        let key = graphCatalogKey
        if !force, graphCatalogReadings[key]?.value != nil,
           graphCatalogRevisions[key] == definitionRevision { return }
        let revision = definitionRevision
        let home = workDataDirectory
        let request = UUID()
        graphCatalogRequests[key] = request
        let previous = graphCatalogReadings[key]?.value
        let result: Result<[GraphCatalogEntry], Error>
        do { result = .success(try await query.graphCatalog(cwd: repoPath)) }
        catch { result = .failure(error) }
        guard home == workDataDirectory, graphCatalogRequests[key] == request else { return }
        graphCatalogReadings[key] = reading(from: result, lastGood: previous)
        graphCatalogRevisions[key] = revision
    }

    /// The repository file that defines graph `name`; a builtin is first copied
    /// into `.lf/graphs/` so the edit is committed with the code.
    func definitionFile(_ name: String) async throws -> URL {
        let path = try await query.customizeGraph(name, cwd: repoPath)
        await rereadDefinitions()
        return URL(fileURLWithPath: path)
    }

    /// Run the Task's Workflow through `lf task run`, then confirm its subscription
    /// update. `graph` takes up that graph; `from` starts at that step, and an
    /// interactive `from` only puts the Task there. Without either, Rust continues
    /// after the step the Task waits at, taking up its Wave's or repository's default first.
    /// The outcome settles only the Task and repository that started it; a
    /// refusal is kept on that Task's draft and changes nothing else.
    func startTask(_ graph: String?, from: String? = nil, task: RoadmapTask) async {
        await performTaskAction(task.id) {
            try await query.runTask(issue: task.task.identifier, graph: graph, from: from,
                                    cwd: WaveOrigin.resolve(task.task.repo))
        }
    }

    /// The graph a new Task in each scope would take up, keyed by repository and
    /// Wave. Rust resolves it; the New Task sheet only shows it.
    private(set) var graphDefaults: [String: WorkReading<WaveGraph>] = [:]

    func graphDefaultKey(wave: WaveSnapshot?) -> String { "\(repoPath ?? ""):\(wave?.id ?? "")" }

    func loadGraphDefault(wave: WaveSnapshot?) async {
        let key = graphDefaultKey(wave: wave)
        let previous = graphDefaults[key]?.value
        let result: Result<WaveGraph, Error>
        do { result = .success(try await query.waveGraph(wave: wave?.name, cwd: repoPath)) }
        catch { result = .failure(error) }
        graphDefaults[key] = reading(from: result, lastGood: previous)
    }

    /// New Task: one `lf task create REQUEST --run` files it in `wave` (nil: the
    /// repository), starts its workflow and admits its conversation with the
    /// request as the first turn; this window then shows that conversation.
    /// When nothing was filed, the sheet keeps the request and the error. A Task
    /// filed before its run failed is selected with the error at its Start control.
    func createTask(request: String, wave: WaveSnapshot?, graph: String?) async throws {
        let receipt: TaskCreateReceipt
        var runError: String?
        do {
            receipt = try await query.createTask(request: request, wave: wave?.name, graph: graph, cwd: repoPath)
        } catch let failed as TaskCreateRunFailed {
            (receipt, runError) = (failed.receipt, failed.message)
        } catch {
            try? await confirmWorkUpdate()
            throw error
        }
        navigation.palette = nil
        let id = receipt.task.id
        guard let runError else {
            // Open it from its own read now; waiting for every reading to confirm
            // first left the window unchanged for seconds after Start.
            await openTaskConversation(id)
            try? await confirmWorkUpdate()
            return
        }
        try? await confirmWorkUpdate()
        if let found = task(id: id) { openTaskDestination(wave: found.wave, task: found.task) }
        else { select(.task(id: id)) }
        navigation.taskRunDrafts[id] = TaskRunDraft(error: runError)
    }

    /// Ship the Task, then confirm its subscription update; it moves to Shipping
    /// and the window's ship toast offers Undo.
    func shipTask(_ task: RoadmapTask) async {
        await performTaskAction(task.id) {
            try await query.shipTask(issue: task.task.identifier, cwd: WaveOrigin.resolve(task.task.repo))
        }
        noteShipping(task, shipped: true)
    }

    /// Stop Auto-Ship for the Task; it returns to Working and the toast says so.
    func stopShipping(_ task: RoadmapTask) async {
        await performTaskAction(task.id) {
            try await query.stopShipping(issue: task.task.identifier, cwd: WaveOrigin.resolve(task.task.repo))
        }
        noteShipping(task, shipped: false)
    }

    private func noteShipping(_ task: RoadmapTask, shipped: Bool) {
        guard navigation.taskRunDrafts[task.id]?.error == nil else { return }
        navigation.shipNotice = ShipNotice(task: task, shipped: shipped)
    }

    /// The Work the Machine control describes: the current started Task, else the
    /// selected Wave. Repository and Dashboard views, and unstarted Tasks, have none.
    var machineTarget: WorkMachineTarget? {
        if let task = breadcrumb?.task?.task {
            return task.runtime?.owner == nil ? nil : .task(issue: task.task.identifier)
        }
        if let work = selection, work.kind == .wave, navigation.dashboardID == nil { return .wave(id: work.id) }
        return nil
    }

    var localMachineId: String? { workspaceRegistry?.localMachineId }

    /// The Machine the target runs on, from planning. A Wave without its own
    /// default runs here; a started Task without one is unknown.
    func machine(of target: WorkMachineTarget) -> (id: String?, label: String?) {
        switch target {
        case .task:
            let machine = breadcrumb?.task?.task.runtime?.owner?.machine
            return (machine?.id, machine?.label)
        case .wave(let id):
            guard let machine = wave(id: id)?.wave.machine else { return (localMachineId, nil) }
            return (machine.id, machine.label)
        }
    }

    func machines(for target: WorkMachineTarget) async throws -> [Machine] {
        try await query.machines(cwd: machineCwd(target))
    }

    /// Hand off the Task or set the Wave's default, then confirm the Work update
    /// so planning and other windows read the same Machine.
    func useMachine(_ machine: String, for target: WorkMachineTarget) async throws {
        let home = workDataDirectory
        try await query.useMachine(machine, for: target, cwd: machineCwd(target))
        try await confirmWorkUpdate(after: home)
    }

    private func machineCwd(_ target: WorkMachineTarget) -> String? {
        if case .task = target, let task = breadcrumb?.task { return WaveOrigin.resolve(task.task.task.repo) }
        return repoPath
    }

    func assignTask(_ task: RoadmapTask, claimed: Bool) async {
        await performTaskAction(task.id) {
            try await query.assignTask(issue: task.task.identifier, claimed: claimed, cwd: WaveOrigin.resolve(task.task.repo))
        }
    }

    func moveTask(_ task: RoadmapTask, toWave wave: String?) async {
        await performTaskAction(task.id) {
            try await query.moveTask(issue: task.task.identifier, wave: wave,
                                    cwd: WaveOrigin.resolve(task.task.repo))
        }
    }

    func completeTask(_ task: RoadmapTask) async {
        await performTaskAction(task.id) {
            try await query.completeTask(issue: task.task.identifier, cwd: WaveOrigin.resolve(task.task.repo))
        }
    }

    func reopenTask(_ task: RoadmapTask) async {
        await performTaskAction(task.id) {
            try await query.reopenTask(issue: task.task.identifier, cwd: WaveOrigin.resolve(task.task.repo))
        }
    }

    func abandonTask(_ task: RoadmapTask) async {
        await performTaskAction(task.id) {
            try await query.abandonTask(issue: task.task.identifier, cwd: WaveOrigin.resolve(task.task.repo))
        }
    }

    /// Feedback belongs to the navigation and Task that began the command.
    private func performTaskAction(_ taskId: String, _ action: () async throws -> Void) async {
        let owner = navigation
        let home = workDataDirectory
        guard owner.taskRunDrafts[taskId]?.acting != true else { return }
        owner.taskRunDrafts[taskId, default: TaskRunDraft()].phase = .runningCommand
        owner.taskRunDrafts[taskId]?.error = nil
        do {
            try await action()
            owner.taskRunDrafts[taskId]?.phase = .awaitingObservation
            try await confirmWorkUpdate(after: home)
            owner.taskRunDrafts[taskId] = nil
        } catch {
            let accepted = owner.taskRunDrafts[taskId]?.phase == .awaitingObservation
            owner.taskRunDrafts[taskId]?.phase = nil
            owner.taskRunDrafts[taskId]?.error = accepted
                ? "Task update accepted; refresh not confirmed. \(error.localizedDescription)"
                : error.localizedDescription
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
        let key = repositoryReadingKey(repo)
        let target = draft.sessionId
        owner.renaming?.submitting = true
        owner.renaming?.error = nil
        do {
            let record = try await query.renameSession(id: target, name: draft.text, cwd: repo)
            // A read started before the rename must not restore the old name.
            supersedeSessions()
            replaceSession(record, key: key)
            if owner.renaming?.id == draft.id { owner.renaming = nil }
        } catch {
            guard owner.renaming?.id == draft.id else { return }
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
        let key = repositoryReadingKey(repo)
        owner.binding?.submitting = true
        owner.binding?.error = nil
        do {
            let record = try await query.bindSession(id: preview.sessionId, taskId: preview.taskId, cwd: repo)
            supersedeSessions()
            replaceSession(record, key: key)
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

    private func replaceSession(_ record: SessionRecord, key: RepositoryReadingKey) {
        let replace = { (records: [SessionRecord]) in
            records.map { $0.id == record.id ? record : $0 }
        }
        switch sessionReadings[key] {
        case .available(let records):
            sessionReadings[key] = .available(replace(records))
        case .unavailable(let records, let reason):
            sessionReadings[key] = .unavailable(lastGood: records.map(replace), reason: reason)
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
        roadmap.value?.waves.first { $0.currentProject?.id == projectId || $0.currentProject?.slug == projectId || $0.currentProject?.linearId == projectId }
    }

    /// Current repository inventory only; retained navigation evidence is display-only.
    var visibleTasks: [RoadmapTask] { taskIndex.tasks }

    /// The repository's Tasks with lookups by ID, Work ID and identifier. Every
    /// sidebar row looks Tasks up on each update; a filtered scan per lookup made
    /// returning to a long list take over a second.
    private struct TaskIndex {
        let key: (revision: Int, repoPath: String?)
        let tasks: [RoadmapTask]
        let positions: [String: Int]
        let waves: [String: WaveRoadmap]
    }
    @ObservationIgnored private var taskIndexCache: TaskIndex?

    private var taskIndex: TaskIndex {
        let tasks = roadmap.value?.tasks.items ?? []
        let key = (revision: WaveOrigin.revision, repoPath: repoPath)
        if let cached = taskIndexCache, cached.key == key { return cached }
        let visible = repoPath.map { path in
            let repository = repoIdentity(path)
            return tasks.filter { repoIdentity($0.task.repo) == repository }
        } ?? tasks
        // The first Task matching any of its keys wins, as a scan in order would.
        var positions: [String: Int] = [:]
        for (index, task) in visible.enumerated() {
            for name in [task.id, task.runtime?.workId, task.task.identifier].compactMap({ $0 }) where positions[name] == nil {
                positions[name] = index
            }
        }
        let waves = Dictionary(visibleRoadmaps.map { ($0.wave.id, $0) }, uniquingKeysWith: { first, _ in first })
        let index = TaskIndex(key: key, tasks: visible, positions: positions, waves: waves)
        taskIndexCache = index
        return index
    }

    private func currentTask(id: String) -> (wave: WaveRoadmap?, task: RoadmapTask)? {
        let index = taskIndex
        guard let position = index.positions[id] else { return nil }
        let task = index.tasks[position]
        return (task.task.waveId.flatMap { index.waves[$0] }, task)
    }

    func task(id: String) -> (wave: WaveRoadmap?, task: RoadmapTask)? {
        if let current = currentTask(id: id) { return current }
        if let row = watchedTaskRow, row.id == id || row.runtime?.workId == id {
            return (row.task.waveId.flatMap { taskIndex.waves[$0] }, row)
        }
        if let retained = navigation.selectedTaskEvidence, retained.task.id == id || retained.task.runtime?.workId == id { return retained }
        return nil
    }

    func waveId(for work: WorkReference) -> String? {
        switch work.kind {
        case .wave:
            work.id
        case .project:
            waveForChapter(projectId: work.id)?.wave.id
        case .task:
            task(id: work.id)?.task.task.waveId
        }
    }

    func clearSelectionIfOutsideScope() {
        guard navigation.dashboardID == nil, let selection else { return }
        if selectionPlans.value?.contains(where: { plan in
            plan.contents.contains { $0.work.reference == selection }
        }) == true { return }
        if let repository = workspaceRepository,
           workspaceRegistry?.retained(workspaceKey(for: selection, repository: repository)) != nil { return }
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
        roadmap: WorkReading<RoadmapSnapshot>,
        waves: WorkReading<[Wave]>,
        workActivity: WorkReading<WorkActivitySnapshot>,
        repos: [PortfolioRepo],
        fixed: Bool = false
    ) {
        self.roadmap = roadmap
        if fixed { self.sessions = .available([]) }
        self.waves = waves
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
                wave: selected.wave?.wave.name,
                project: nil,
                task: selected.task.task.identifier
            )
        }
    }

    private func setSelection(_ selection: WorkReference?) {
        viewOpening = nil
        selectionOpening = nil
        guard navigation.destination != .work(selection) else { return }
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
    private func readRoadmap() async -> Result<(RoadmapSnapshot, String?), Error> {
        let wire = WireCapture()
        do {
            let snapshot = try await query.recording(wire.record).roadmap()
            return .success((snapshot, wire.texts.last))
        } catch {
            return .failure(error)
        }
    }

    /// Stream and one-shot reads share the decision to replace visible planning
    /// and its cache. A failed replacement leaves both intact.
    @discardableResult
    private func publishRoadmap(_ result: Result<(RoadmapSnapshot, String?), Error>, lastGood: RoadmapSnapshot?) -> Bool {
        if case .success(let (snapshot, _)) = result, let reason = planningFailure(snapshot, replacing: lastGood) {
            roadmap = .unavailable(lastGood: lastGood ?? snapshot, reason: reason)
            return false
        }
        let next = reading(from: result.map(\.0), lastGood: lastGood)
        if roadmap != next { roadmap = next }
        guard case .success(let (_, text)) = result else { return false }
        showsSavedPlanning = false
        if let text { cache?.saveRoadmap(text) }
        return true
    }

    /// Do not replace previously readable planning with a failed partial frame.
    /// An already unavailable Wave must not prevent other Waves from refreshing.
    private func planningFailure(_ snapshot: RoadmapSnapshot, replacing previous: RoadmapSnapshot?) -> String? {
        if let reason = snapshot.tasks.unavailableReason { return reason }
        if case .available(_, true) = snapshot.tasks { return "Task reading is incomplete" }
        for wave in snapshot.waves {
            guard let old = previous?.waves.first(where: { $0.wave.id == wave.wave.id }) else { continue }
            if case .available = old.projects {
                if let reason = wave.projects.unavailableReason { return "\(wave.wave.displayName): \(reason)" }
                if case .available(_, true) = wave.projects { return "\(wave.wave.displayName): Project reading is incomplete" }
            }
            if case .available = old.tasks {
                if let reason = wave.tasks.unavailableReason { return "\(wave.wave.displayName): \(reason)" }
                if case .available(_, true) = wave.tasks { return "\(wave.wave.displayName): Task reading is incomplete" }
            }
        }
        return nil
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
    ) -> WorkReading<Value> {
        switch result {
        case .success(let value):
            .available(value)
        case .failure(let error):
            .unavailable(lastGood: lastGood, reason: error.localizedDescription)
        }
    }
}

extension WorkModel {
    var dashboards: [WorkViewPlan] { selectionPlans.value?.filter { $0.dashboardID != nil } ?? [] }
    func dashboard(_ id: String) -> WorkViewPlan? { selectionPlans.value?.first { $0.dashboardID == id } }

    private func selectDestination(_ destination: WorkNavigation.Destination, recordingHistory: Bool = false) {
        switch destination {
        case .work(let work): select(work, preservingOpening: false, recordingHistory: recordingHistory)
        case .entered(let board, let work):
            // Re-selecting the zoomed member (⌘N on the open row) is not a new visit.
            select(work, preservingOpening: false, recordingHistory: recordingHistory && navigation.destination != destination)
            navigation.destination = .entered(from: board, work: work)
        case .dashboard, .view, .workingTasks:
            if recordingHistory, navigation.destination != destination {
                navigation.backStack.append(navigation.destination)
                navigation.forwardStack = []
            }
            dismissTaskLink()
            viewOpening = nil
            selectionOpening = nil
            navigation.destination = destination
            navigation.palette = nil
            navigation.selectedSessionId = nil
            if let plan = selectedViewPlan { presentView(plan) }
        }
    }

    /// Dashboards are repository-level; their sidebar's back control returns to the repository.
    func selectDashboard(_ id: String) {
        navigation.sidebarWaveID = nil
        selectDestination(.dashboard(id), recordingHistory: true)
    }

    func selectWorkingTasksDashboard() {
        navigation.sidebarWaveID = nil
        selectDestination(.workingTasks, recordingHistory: true)
    }

    /// The Working Tasks Dashboard's Tasks, in the sidebar's order.
    var workingTaskMembers: [WorkMemberRef] { boardTiles(.workingTasks) }

    /// Entering from a Wave view turns its strip to the entered Task's page.
    func enter(_ work: WorkReference, from board: WorkNavigation.Board) {
        showWaveTile(work, in: board)
        selectDestination(.entered(from: board, work: work), recordingHistory: true)
    }

    /// Turn a Wave view's footer to the page holding `work`; positions count the whole list.
    /// Only Wave views read the page; a Dashboard's grid keeps its own.
    private func showWaveTile(_ work: WorkReference, in view: WorkNavigation.Board) {
        guard let plan = boardPlan(view),
              let index = taskIDs(in: boardTiles(view)).firstIndex(of: task(id: work.id)?.task.id ?? work.id) else { return }
        navigation.wavePages[view] = index / max(1, plan.slots.count)
    }

    /// A board's Task tiles: a Dashboard's visible ones in arrangement order, a Wave's
    /// in the shared order; the repository page's are its Working Tasks.
    func boardTiles(_ board: WorkNavigation.Board) -> [WorkMemberRef] {
        let members = if case .saved = board { boardPlan(board)?.projection.visible } else { boardPlan(board)?.resolution.members }
        var seen = Set<WorkWorkspaceKey>()
        return (members ?? []).filter { $0.work.kind == .task && seen.insert($0.work).inserted }
    }

    func boardPlan(_ board: WorkNavigation.Board) -> WorkViewPlan? {
        switch board {
        case .saved(let id): dashboard(id)
        case .workingTasks, .repository: builtInPlan(.repository)
        case .wave(let id): builtInPlan(.wave(id))
        }
    }

    /// The Wave view stays selected; its Tasks open after it, in tile order, so a
    /// script addresses them as it does a Dashboard's.
    private func presentWaveTiles() {
        guard let registry = workspaceRegistry, let view = navigation.waveView,
              let key = registry.selectedWork, key.kind != .task, let plan = boardPlan(view) else { return }
        let tiles = boardTiles(view).map(\.work)
        openTiles(plan, tiles, in: registry)
        registry.present(open: [key] + tiles)
    }

    private func presentView(_ plan: WorkViewPlan) {
        guard let registry = workspaceRegistry else { return }
        // Working Tasks renders the repository plan's Tasks; its Waves stay in the sidebar.
        let open: [WorkWorkspaceKey] = if case .workingTasks = navigation.destination {
            workingTaskMembers.map(\.work)
        } else {
            plan.projection.visible.map(\.work)
        }
        openTiles(plan, open, in: registry)
        registry.present(open: open)
    }

    private func openTiles(_ plan: WorkViewPlan, _ open: [WorkWorkspaceKey], in registry: SessionsWorkspaceRegistry) {
        for content in plan.contents where open.contains(content.work) {
            let entry = registry.workspace(for: content.work, title: content.title,
                location: content.location.map { WorkspaceIdentity(machineId: $0.machineID, worktree: $0.cwd) })
            if let primary = content.primarySession, entry.multiplexer.sessionPane?.sessionId == nil {
                entry.multiplexer.load(sessionId: primary, location: entry.location, focus: false)
            }
            entry.startPersistence(query: query, repoPath: repoPath)
        }
    }

    func saveDashboard(id: String, expression: String, name: String, pinned: Bool, revision: Int64?) async {
        guard !dashboardSaving, let repoPath else { return }
        dashboardSaving = true
        dashboardError = nil
        defer { dashboardSaving = false }
        do {
            var saved = try await query.editDashboardQuery(id: id, expression: expression, revision: revision, repoPath: repoPath)
            if saved.name != name {
                saved = try await query.renameDashboard(id: id, name: name, revision: saved.revision, repoPath: repoPath)
            }
            if saved.pinned != pinned {
                _ = try await query.pinDashboard(id: id, pinned: pinned, revision: saved.revision, repoPath: repoPath)
            }
            try await refreshDashboard(id, repoPath: repoPath)
            if self.repoPath == repoPath { selectDashboard(id) }
        } catch {
            dashboardError = error.localizedDescription
            // A preceding metadata write may have succeeded; read effects, never replay writes.
            try? await refreshDashboard(id, repoPath: repoPath)
        }
    }

    /// Hide or show a tile, as `lf desktop hide`/`show` do; its place is kept.
    func setDashboardTile(_ id: String, member: WorkWorkspaceKey, hidden: Bool) async {
        guard !dashboardSaving, let repoPath else { return }
        dashboardSaving = true
        dashboardError = nil
        defer { dashboardSaving = false }
        do {
            _ = try await query.setDashboardTile(id: id, member: member, hidden: hidden, repoPath: repoPath)
            try await refreshDashboard(id, repoPath: repoPath)
        } catch { dashboardError = error.localizedDescription }
    }

    /// A tile drop: the dragged Work takes the target's place in the Dashboard's order.
    func reorderDashboard(_ id: String, member: WorkWorkspaceKey, to target: WorkWorkspaceKey) async {
        guard member != target, !dashboardSaving, let repoPath else { return }
        dashboardSaving = true
        dashboardError = nil
        defer { dashboardSaving = false }
        do {
            _ = try await query.reorderDashboard(id: id, member: member, to: target, repoPath: repoPath)
            try await refreshDashboard(id, repoPath: repoPath)
        } catch { dashboardError = error.localizedDescription }
    }

    func setDashboardSlots(_ id: String, slots: DashboardSlotLayout?, revision: Int64) async {
        guard !dashboardSaving, let repoPath else { return }
        dashboardSaving = true
        dashboardError = nil
        defer { dashboardSaving = false }
        do {
            _ = try await query.saveDashboardSlots(id: id, slots: slots, revision: revision, repoPath: repoPath)
            try await refreshDashboard(id, repoPath: repoPath)
        } catch { dashboardError = error.localizedDescription }
    }

    /// The sidebar and the Working Tasks grid write the same order; both read the
    /// repository plan, so each shows the other's move.
    /// The row moves on drop; the written order replaces it, and a failed write re-reads it.
    func reorderWorkingTask(_ task: String, to target: String) async {
        guard task != target, let repoPath else { return }
        dashboardError = nil
        moveRepositoryMember(task, to: target)
        do {
            _ = try await query.reorderTask(task, to: target, repoPath: repoPath)
            try await refreshPlan(nil, repoPath: repoPath)
        } catch {
            dashboardError = error.localizedDescription
            try? await refreshPlan(nil, repoPath: repoPath)
        }
    }

    /// `task` takes `target`'s place in the repository plan, as `lf desktop move` writes it.
    private func moveRepositoryMember(_ task: String, to target: String) {
        guard var plans = selectionPlans.value,
              let index = plans.firstIndex(where: { $0.dashboardID == nil && $0.selection.expression == .repository }) else { return }
        var members = plans[index].resolution.members
        guard let from = members.firstIndex(where: { $0.work.kind == .task && $0.work.id == task }),
              let to = members.firstIndex(where: { $0.work.kind == .task && $0.work.id == target }) else { return }
        members.insert(members.remove(at: from), at: to)
        plans[index].resolution.members = members
        applySelectionPlans(plans, reason: "")
    }

    /// The Working Tasks member a sidebar row shows; rows may use the planning ID.
    func workingTaskWorkID(row id: String) -> String? {
        workingTaskMembers.first { (task(id: $0.work.id)?.task.id ?? $0.work.id) == id }?.work.id
    }

    private func refreshDashboard(_ id: String, repoPath: String) async throws {
        try await refreshPlan("query:" + id, repoPath: repoPath)
    }

    /// Re-read one plan after a write; nil reads the repository's built-in plan.
    private func refreshPlan(_ expression: String?, repoPath: String) async throws {
        let plan = try await query.viewPlan(expression: expression, repoPath: repoPath)
        guard self.repoPath == repoPath else { return }
        replacePlan(plan)
    }

    /// A Dashboard plan replaces its Dashboard's; a built-in plan replaces its expression's.
    private func replacePlan(_ plan: WorkViewPlan) {
        var plans = selectionPlans.value ?? []
        plans.removeAll { other in
            other.dashboardID == plan.dashboardID
                && (plan.dashboardID != nil || other.selection.expression == plan.selection.expression)
        }
        plans.append(plan)
        applySelectionPlans(plans, reason: "")
    }
}


extension WorkModel {
    var selectedViewPlan: WorkViewPlan? {
        switch navigation.destination {
        case .work, .entered: nil
        case .dashboard(let id): dashboard(id)
        case .view(let plan): plan
        case .workingTasks: builtInPlan(.repository)
        }
    }

    private func refreshSelectedView() async {
        guard case .view(let previous) = navigation.destination, let repoPath else { return }
        do {
            let plan = try await query.viewPlan(expression: previous.selection.queryText, repoPath: repoPath)
            guard self.repoPath == repoPath, navigation.destination == .view(previous) else { return }
            navigation.destination = .view(plan)
            presentView(plan)
        } catch {
            guard self.repoPath == repoPath, navigation.destination == .view(previous) else { return }
            dashboardError = error.localizedDescription
        }
    }

    func openView(expression: String, url: URL? = nil) async {
        guard let repoPath else { return }
        let request = UUID()
        viewOpening = request
        let link = url?.absoluteString ?? "loopflow://open"
        selectionOpening = DesktopOpening(url: link, status: .opening, reason: nil)
        do {
            let plan = try await query.viewPlan(expression: expression, repoPath: repoPath)
            guard viewOpening == request, self.repoPath == repoPath else { return }
            if plan.dashboardID != nil { replacePlan(plan) }
            selectDestination(plan.dashboardID.map { .dashboard($0) } ?? .view(plan), recordingHistory: true)
            selectionOpening = DesktopOpening(url: link, status: .usable, reason: nil)
        } catch {
            guard viewOpening == request else { return }
            viewOpening = nil
            dashboardError = error.localizedDescription
            selectionOpening = DesktopOpening(url: link, status: .failed, reason: error.localizedDescription)
        }
    }
}

/// Drag payload for moving a Task in the shared Working Tasks order.
enum TaskOrderDrag {
    static func token(_ workID: String) -> String { "task-order:" + workID }
    static func taskID(_ token: String) -> String? {
        token.hasPrefix("task-order:") ? String(token.dropFirst("task-order:".count)) : nil
    }
}

