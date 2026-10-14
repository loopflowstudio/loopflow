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
        switch key.kind {
        case .repository: return repoPath.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Repository unavailable"
        case .wave: return visibleRoadmaps.first { $0.wave.id == key.id }?.wave.name ?? workspaceRegistry?.retained(key)?.title ?? "Wave unavailable"
        case .task: return task(id: key.id)?.task.task.name ?? workspaceRegistry?.retained(key)?.title ?? "Task unavailable"
        }
    }

    func bindWorkspaces(_ registry: SessionsWorkspaceRegistry, repository: String) {
        guard workspaceRegistry !== registry else { return }
        workspaceRegistry = registry
        workspaceRepository = repository
        openSelectedWorkspace()
    }

    func openSelectedWorkspace() {
        guard let registry = workspaceRegistry, let repository = workspaceRepository else { return }
        let key = workspaceKey(for: selection, repository: repository)
        let location: WorkspaceIdentity?
        if key.kind == .task, let task = task(id: key.id)?.task {
            location = navigation.preparedTaskWorktrees[task.id] ?? task.reference.workspace?.identity
        } else if key.kind != .task, let machine = registry.localMachineId, let repoPath {
            location = WorkspaceIdentity(machineId: machine, worktree: repoPath)
        } else { location = nil }
        let workspace = registry.open(key, title: workspaceTitle(key), location: location)
        navigation.workspacePresentation = workspace.presentation
        navigation.selectedSessionId = workspace.presentation.selectedSessionId
    }

    @ObservationIgnored var openRepository: ((String, URL?) -> Void)?

    private(set) var taskLinkURL: URL?
    private(set) var taskLinkReading: WorkReading<RoadmapSnapshot> = .loading
    var showsTaskLink = false
    private(set) var linkedTaskPage: LinkedTaskPage?
    private(set) var taskOpening: DesktopOpening?
    @ObservationIgnored private var destinationGeneration = 0

    @ObservationIgnored private var taskLinkExpectedID: String?
    @ObservationIgnored private var taskLinkConversation = false

    func openTaskLink(_ url: URL, expectedTaskID: String? = nil, conversation: Bool = false) async {
        guard !Task.isCancelled else { return }
        dismissTaskLink()
        let generation = destinationGeneration
        taskOpening = DesktopOpening(url: url.absoluteString, status: .opening, reason: nil)
        defer {
            if Task.isCancelled, destinationGeneration == generation { dismissTaskLink() }
        }
        // Observation invalidates on every write, changed or not. Reopening
        // what is already open must not redraw the window.
        if taskLinkURL != url { taskLinkURL = url }
        taskLinkExpectedID = expectedTaskID
        taskLinkConversation = conversation
        if showsTaskLink { showsTaskLink = false }
        do {
            let link = try TaskLink(url: url)
            // Already observed planning opens directly, like the outline and palette.
            // Unscoped links and identity mismatches still resolve through a read.
            if let loaded = loadedTask(link), expectedTaskID == nil || loaded.task.id == expectedTaskID {
                try await openLinkedTask(wave: loaded.wave, task: loaded.task, link: link, generation: generation)
                return
            }
            taskLinkReading = .loading
            let result = try await query.taskDestination(issue: link.issue, repo: link.repo)
            guard !Task.isCancelled, destinationGeneration == generation else { return }
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
            } else {
                taskOpening = DesktopOpening(url: url.absoluteString, status: .failed, reason: "Task lookup is missing, ambiguous or unavailable; choose a repository-qualified Task.")
                showsTaskLink = true
            }
        } catch {
            Perf.agentInput("opening_failed", task: expectedTaskID ?? "")
            failTaskLink(error, url: url, generation: generation)
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

    func chooseLinkedTask(wave: WaveRoadmap, task: RoadmapTask) async {
        guard let taskLinkURL else { return }
        dismissTaskLink()
        let generation = destinationGeneration
        taskOpening = DesktopOpening(url: taskLinkURL.absoluteString, status: .opening, reason: nil)
        defer {
            if Task.isCancelled, destinationGeneration == generation { dismissTaskLink() }
        }
        do {
            try await openLinkedTask(wave: wave, task: task, link: TaskLink(url: taskLinkURL), generation: generation)
        } catch {
            failTaskLink(error, url: taskLinkURL, generation: generation)
        }
    }

    private func failTaskLink(_ error: Error, url: URL, generation: Int) {
        guard !Task.isCancelled, destinationGeneration == generation else { return }
        taskOpening = DesktopOpening(url: url.absoluteString, status: .failed, reason: error.localizedDescription)
        taskLinkReading = .unavailable(lastGood: nil, reason: error.localizedDescription)
        // A sidebar opening already has its retained Task error and retry.
        // Only unresolved external links need the lookup sheet.
        let hasTaskPage = taskLinkConversation
            && taskLinkExpectedID.map { selection == .task(id: $0) } == true
        showsTaskLink = !hasTaskPage
    }

    /// The one Task a repository-qualified link names in planning this window holds.
    private func loadedTask(_ link: TaskLink) -> (wave: WaveRoadmap, task: RoadmapTask)? {
        guard let repo = link.repo?.normalizedFilePath else { return nil }
        guard case .available(let snapshot) = roadmap else { return nil }
        let matches = snapshot.waves.filter { $0.wave.repo.normalizedFilePath == repo }.flatMap { wave in
            wave.tasks.items.filter { $0.task.identifier == link.issue || $0.id == link.issue || $0.runtime?.workId == link.issue }.map { (wave, $0) }
        }
        return matches.count == 1 ? matches[0] : nil
    }

    private func openLinkedTask(wave: WaveRoadmap, task: RoadmapTask, link: TaskLink, generation: Int) async throws {
        let opensConversation = link.session != nil || link.diff || taskLinkConversation
        let isLocalRepository = repoPath?.normalizedFilePath == wave.wave.repo.normalizedFilePath
        if opensConversation, isLocalRepository {
            openTaskDestination(wave: wave, task: task, preservingOpening: true)
            navigation.content = .terminals
        }
        let key = workspaceKey(for: .task(id: task.id), repository: workspaceRepository ?? "")
        let workspace = opensConversation && isLocalRepository ? workspaceRegistry?.retained(key) : nil
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

    private func prepareLinkedTask(wave: WaveRoadmap, task: RoadmapTask, link: TaskLink, generation: Int) async throws {
        try Task.checkCancellation()
        guard destinationGeneration == generation else { return }
        var task = task
        if let openRepository, repoPath?.normalizedFilePath != wave.wave.repo.normalizedFilePath {
            openRepository(wave.wave.repo, taskLinkURL)
            return
        }
        var location: TaskLocationObservation?
        var executionQuery = query
        var executionTask = task.runtime?.workId ?? task.id
        var executionIdentity = navigation.preparedTaskWorktrees[task.id] ?? task.reference.workspace?.identity
        if let machine = link.machine, let repository = link.repository {
            let identity = try await query.repositoryIdentity(cwd: wave.wave.repo)
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
            linkedTaskPage = LinkedTaskPage(generation: generation, taskID: task.id, repo: wave.wave.repo.normalizedFilePath)
            return
        }
        let sameRepo = repoPath?.normalizedFilePath == wave.wave.repo.normalizedFilePath
        let retainedRecords = sameRepo && location == nil ? sessions.value ?? [] : []
        let sessionID: String
        var ensuredRecord: SessionRecord?
        if let selected = link.session {
            sessionID = selected
        } else {
            // Reuse the primary-Session owner rather than creating another
            // conversation or replaying a retained terminal's launch command.
            Perf.agentInput("primary_begin", task: task.id)
            let record = try await executionQuery.ensureTaskSession(issue: executionTask, cwd: wave.wave.repo)
            Perf.agentInput("primary_returned", task: task.id, session: record.id, detail: "provider=\(record.provider ?? "unknown")")
            guard !Task.isCancelled, destinationGeneration == generation else { return }
            sessionID = record.id
            ensuredRecord = record
            // Preparation may have supplied a checkout/runtime absent from the
            // planning read. Refresh this exact Task, never the current selection.
            if location == nil {
                let refreshed = try await query.taskDestination(issue: task.task.identifier, repo: wave.wave.repo)
                guard !Task.isCancelled, destinationGeneration == generation else { return }
                guard let current = refreshed.waves.flatMap({ $0.tasks.items }).first(where: { $0.id == task.id }) else {
                    throw RegistryQueryError("The prepared Task is no longer available; its Session was not retargeted.")
                }
                task = current
                executionTask = current.runtime?.workId ?? current.id
                executionIdentity = navigation.preparedTaskWorktrees[task.id] ?? task.reference.workspace?.identity
            }
        }
        var records: [SessionRecord] = []
        if let ensuredRecord {
            // Ensure already returned this Session's authoritative reading.
            // Opening one Task must not wait for every repository Session.
            records = [ensuredRecord]
        } else if link.session != nil, retainedRecords.contains(where: { $0.id == sessionID }) {
            records = retainedRecords
        } else {
            // An explicit Session link not held by this window still needs
            // lookup evidence; publish its inventory only after the last page.
            var after: String?
            repeat {
                let page = try await executionQuery.sessionPage(includingHeadless: true, after: after, cwd: wave.wave.repo)
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
        if location != nil || ensuredRecord != nil {
            // One explicitly opened Session updates its own reading. Merge the
            // latest inventory: it may have refreshed during preparation.
            records = [record] + (sessions.value ?? []).filter { $0.id != record.id }
        }
        if ensuredRecord != nil, let reason = sessions.errorMessage {
            // One successful acquisition does not repair an unrelated inventory failure.
            sessions = .unavailable(lastGood: records, reason: reason)
        } else {
            sessions = .available(records)
        }
        let destination = LinkedSession(generation: generation, record: record, task: task, showsChanges: link.diff, location: location)
        try await installConversation(destination, repository: wave.wave.repo)
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
                              store: SessionsStore, registry: SessionsWorkspaceRegistry) -> SessionsWorkspace {
        Perf.begin(Perf.taskWorkspaceReady, "session", id: record.id)
        let workspace: SessionsWorkspace
        if case .shell(let id, let machine) = store.localTerminal(for: record),
           let key = registry.key(containingShell: id, machineId: machine),
           let retained = registry.retained(key) {
            workspace = retained
            select(key.reference, preservingOpening: true)
            workspace.multiplexer.setCollapsed(paneId: id, collapsed: false)
            workspace.multiplexer.setFocusedPane(id)
            store.surfaces.focus(.shell(id, machineId: machine))
        } else {
            workspace = target
            registry.revealSession(record, in: workspace)
            if let machine = record.workspace?.machineId {
                store.surfaces.focus(.session(record.id, machineId: machine))
            }
        }
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
            sessionPane = layout.pane(for: id)
        } else { sessionPane = layout.allPanes.first { $0.sessionId == destination.record.id } }
        var panes = [PaneState]()
        if let sessionPane { panes.append(sessionPane) }
        if destination.showsChanges,
           let pane = layout.allPanes.first(where: {
               if case .files(let task, _, _) = $0.content { return task == destination.task.id }
               return false
           }) { panes.append(pane) }
        guard panes.count == (destination.showsChanges ? 2 : 1) else {
            taskOpening = taskOpening.map { DesktopOpening(url: $0.url, status: .failed, reason: "Opening panes are no longer available.") }
            return
        }
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
            case .elsewhere: reason = "Session is open elsewhere; use Move here explicitly."
            case .pending where item.record.action(.open) == nil:
                reason = "Session has no available opening action."
            default: reason = files?.isRefreshingChanges == true ? nil : files?.changesError
            }
        }
        if let reason {
            taskOpening = DesktopOpening(url: opening.url, status: .failed, reason: reason)
        } else if item.state == .live,
                  !destination.showsChanges || (files?.changes != nil && files?.isRefreshingChanges == false) {
            taskOpening = DesktopOpening(url: opening.url, status: .usable, reason: nil)
        }
    }

    func openTaskDestination(wave: WaveRoadmap, task: RoadmapTask) {
        openTaskDestination(wave: wave, task: task, preservingOpening: false)
    }

    private func openTaskDestination(wave: WaveRoadmap, task: RoadmapTask, preservingOpening: Bool) {
        if let openRepository, repoPath?.normalizedFilePath != wave.wave.repo.normalizedFilePath {
            openRepository(wave.wave.repo, TaskLink(issue: task.task.identifier, repo: wave.wave.repo).url)
            return
        }
        setRepoPath(wave.wave.repo, preservingOpening: preservingOpening)
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
        guard let found = task(id: id),
              let url = TaskLink(issue: found.task.task.identifier, repo: found.wave.wave.repo).url else { return }
        let key = workspaceKey(for: .task(id: id), repository: workspaceRepository ?? "")
        let retainedConversation = workspaceRegistry?.retained(key)?.multiplexer.layout.allPanes.contains {
            $0.sessionId != nil
        } ?? false
        if !retainedConversation, taskOpening?.status == .opening,
           taskLinkExpectedID == found.task.id, taskLinkConversation { return }
        Perf.agentInput("workspace_selected", task: id, detail: "retained=\(retainedConversation)")
        openTaskDestination(wave: found.wave, task: found.task)
        navigation.content = .terminals
        if !retainedConversation {
            await openTaskLink(url, expectedTaskID: found.task.id, conversation: true)
        }
    }

    func openPaletteTask(_ id: String) async {
        if let found = task(id: id) {
            await openTaskConversation(found.task.id)
            return
        }
        guard let recent = navigation.recentDestinations.first(where: { $0.id == .task(id) }),
              let repoPath else { return }
        guard let url = TaskLink(issue: recent.key, repo: repoPath).url else { return }
        await openTaskLink(url, expectedTaskID: id, conversation: true)
    }

    func retryTaskLink() async {
        guard let taskLinkURL else { return }
        await openTaskLink(taskLinkURL, expectedTaskID: taskLinkExpectedID, conversation: taskLinkConversation)
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
            path: repoPath, selectionKind: workspaces.selectedWork?.kind.rawValue, selectionId: workspaces.selectedWork?.id,
            reading: reading, reason: workStatus.message,
            task: inspectSelectedTask(), session: inspectSelectedSession(),
            supportedOperations: ["list", "hide", "restore", "focus", "split", "move", "resize", "zoom", "shell", "files", "flow-log", "read", "text", "key"], workspaces: workspaces.inspect(), opening: taskOpening)
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

    var taskHistoryFilters: [String: TaskHistoryFilter] = [:]
    var taskHistoryNow = Date()
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
        WorkProjection(roadmaps: visibleRoadmaps, sessions: sessions.value ?? [])
    }
    var breadcrumb: WorkBreadcrumb? {
        if let current = projection.breadcrumb(selection: selection, sessionId: navigation.selectedSessionId) {
            return current
        }
        guard let retained = navigation.selectedTaskEvidence, selection == .task(id: retained.task.id) else { return nil }
        // Exact lookup evidence supports inspection without joining the current plan.
        return WorkProjection(roadmaps: [retained.wave], sessions: sessions.value ?? [])
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
        WorkProjection(roadmaps: visibleRoadmaps, sessions: visibleSessions)
    }

    private(set) var roadmap: WorkReading<RoadmapSnapshot> = .loading {
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
    /// Selectable Flows per repository, read once on demand; never per repaint.
    private var flowCatalogReadings: [String: WorkReading<[FlowCatalogEntry]>] = [:]
    var flowCatalog: WorkReading<[FlowCatalogEntry]> {
        flowCatalogReadings[repoPath ?? ""] ?? .loading
    }
    private var workflowCatalogReadings: [String: WorkReading<[WorkflowCatalogEntry]>] = [:]
    private var workflowWave: String? { breadcrumb?.wave?.roadmap.wave.id }
    private var workflowCatalogKey: String { "\(repoPath ?? "")|\(workflowWave ?? "")" }
    var workflowCatalog: WorkReading<[WorkflowCatalogEntry]> {
        workflowCatalogReadings[workflowCatalogKey] ?? .loading
    }
    /// Comment threads, read on demand for the shown Task.
    private(set) var comments = TaskReadings<TaskComments>()
    /// The shown Task's work, from the workspace reader's `task` part.
    private(set) var taskWork = TaskReadings<TaskWork>()
    /// That Task's Flow processes keyed by Flow process, from the same part.
    private(set) var flowProcesses: [String: FlowProcessDetail] = [:]
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
    /// The Wave whose detail a view is showing, if any.
    var detailWaveId: String?
    /// The scoped Wave's `lf wave status` reading, as last streamed.
    private(set) var waveDetail: StreamedWaveDetail?
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

    /// Current planning is only an entry candidate. Acquisition checks the full
    /// historical Wave inventory before treating this repository as empty.
    var repositoryEntryReady: Bool {
        guard repoPath != nil, workStatus == .current, case .available = waves else { return false }
        return true
    }

    var isEmptyRepositoryCandidate: Bool {
        guard repositoryEntryReady, visibleWaves.isEmpty, visibleRoadmaps.isEmpty else { return false }
        return !(sessions.value ?? []).contains { $0.work != nil || !$0.taskIds.isEmpty || $0.workspace?.taskId != nil }
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
            guard let selection = entry.selection, let waves = roadmap.value?.waves else { continue }
            let evidence = waves.lazy.compactMap { wave in
                wave.tasks.items.first { $0.id == selection.id }.map { (wave, $0) }
            }.first
            guard selection.kind == .task ? evidence != nil : waves.contains(where: { $0.wave.id == selection.id }) else { continue }
            let navigation = navigationByRepo[repo] ?? WorkNavigation()
            navigationByRepo[repo] = navigation
            navigation.selection = selection
            navigation.selectedTaskEvidence = evidence
            navigation.content = .details
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
                    navigation.selection = nil
                    navigation.selectedTaskEvidence = nil
                    navigation.content = .overview
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
        if usesFixedFixture {
            if AppTestMode.current() == .sessionFixtures { await refreshSessions() }
            return
        }
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
                workObservation = opened
                workOpened = .now
                appliedSequence = [:]
                (planningFloor, sessionsFloor, scopeFloor, taskFloor, sentScope) = (0, 0, 0, 0, nil)
                syncWorkScope()
                for try await frame in opened.frames {
                    guard !Task.isCancelled else { break }
                    await apply(frame)
                    if frame.content.part != "heartbeat" { delay = .seconds(1) }
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
            wave: detailWaveId,
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
        sessionsFloor = send { .refresh(id: $0) }
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
        if repoPath != nil { peerPlanning = .unavailable(lastGood: peerPlanning.value, reason: reason) }
        if let value = processActivity.value { processActivity = .unavailable(lastGood: value, reason: reason) }
        taskWork.values = taskWork.values.mapValues { .unavailable(lastGood: $0.value, reason: reason) }
        if let selection, selection.kind == .task {
            taskWork.values[selection.id] = .unavailable(lastGood: taskWork[selection.id].value, reason: reason)
        }
    }

    private func apply(_ frame: WorkFrame) async {
        let part = frame.content.part
        if case .heartbeat = frame.content { return }
        guard frame.sequence > appliedSequence[part] ?? 0 else { return }
        if let workDataDirectory, workDataDirectory != frame.home {
            // The stream names its data directory, not the opaque Machine ID
            // returned by machine id and used by Session locations.
            dropMachineContent()
        }
        workDataDirectory = frame.home
        let answers = frame.answers ?? 0
        let reason = frame.unavailable ?? "Work reader returned no \(part) reading"
        switch frame.content {
        case .planning(let body):
            guard answers >= planningFloor else { return }
            await applyPlanning(body, wire: frame.wire, reason: reason)
            planningSequence = frame.sequence
            resumePlanningWaiters(through: answers)
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
            guard answers >= max(taskFloor, scopeFloor), let shown = selection, shown.kind == .task,
                  let task = task(id: shown.id)?.task else { return }
            guard let body else {
                taskWork.values[task.id] = .unavailable(lastGood: taskWork[task.id].value, reason: reason)
                break
            }
            guard body.task == task.task.identifier else { return }
            comments.receive(body.comments, for: task.id)
            let next = WorkReading.available(body.work)
            if taskWork[task.id] != next { taskWork.values[task.id] = next }
            let runs = Dictionary(body.flowProcesses.map { ($0.entry.id, $0) }) { _, newer in newer }
            if flowProcesses != runs { flowProcesses = runs }
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
        case .heartbeat:
            break
        }
        appliedSequence[part] = frame.sequence
    }

    private func applyPlanning(_ body: WorkFrame.Planning?, wire: WorkFrame.Wire?, reason: String) async {
        taskHistoryNow = Date()
        // Failed stream readings supersede older one-shot requests too.
        roadmapGeneration &+= 1
        guard let body else {
            roadmap = .unavailable(lastGood: roadmap.value, reason: reason)
            return
        }
        await Self.resolveRepoOrigins(body.waves.map(\.repo))
        let reportsLaunch = showsSavedPlanning || roadmapGeneration == 1
        guard publishRoadmap(.success((body.roadmap, wire?.roadmap)), lastGood: roadmap.value) else { return }
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
        roadmap = .loading
        waves = .loading
        showsSavedPlanning = false
        for navigation in navigationByRepo.values {
            navigation.selection = nil
            navigation.selectedTaskEvidence = nil
            navigation.content = .overview
        }
        sessionReadings = [:]
        peerPlanningReadings = [:]
        peerPlanningGeneration &+= 1
        savedSessionRepos = []
        waveDetail = nil
        taskWork = TaskReadings<TaskWork>()
        flowProcesses = [:]
    }

    /// One explicit read of everything, for a change the cadence should not wait on.
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

    private func setRepoPath(_ path: String?, preservingOpening: Bool) {
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


    func updateTaskDirective(task: RoadmapTask, wave: WaveSnapshot, text: String) async throws {
        try await query.updateTaskDirective(id: task.id, wave: wave.name, text: text, cwd: wave.repo)
        if workObservation != nil {
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
        select(requested, preservingOpening: false)
    }

    func select(_ requested: WorkReference?, preservingOpening: Bool) {
        if !preservingOpening { dismissTaskLink() }
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
        if workspaceRegistry == nil {
            navigation.selectedSessionId = nil
            navigation.content = selection == nil ? .overview : .details
        }
        setSelection(selection)
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
        guard !usesFixedFixture || AppTestMode.current() == .sessionFixtures else { return }
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

    /// Read one Task's thread.
    func loadComments(task: RoadmapTask, wave: WaveSnapshot) async {
        await loadTaskReading(\.comments, task: task.id) { [query] in
            try await query.taskComments(id: task.id, wave: wave.name, cwd: WaveOrigin.resolve(wave.repo))
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

    /// Definitions are files, which no store revision follows. Coming back
    /// from an editor, read the catalogue this window already shows again, so
    /// a saved mistake shows as invalid.
    func rereadDefinitions() async {
        guard flowCatalogReadings[repoPath ?? ""] != nil || workflowCatalogReadings[workflowCatalogKey] != nil else { return }
        await loadFlowCatalog(force: true)
        await loadWorkflowCatalog(force: true)
    }

    /// Read the Flow catalogue for the current repository the first time it
    /// is needed, or again after its definitions may have changed (`force`).
    func loadFlowCatalog(force: Bool = false) async {
        let key = repoPath ?? ""
        if !force, flowCatalogReadings[key]?.value != nil { return }
        let previous = flowCatalogReadings[key]?.value
        let result: Result<[FlowCatalogEntry], Error>
        do { result = .success(try await query.flowCatalog(cwd: repoPath)) }
        catch { result = .failure(error) }
        flowCatalogReadings[key] = reading(from: result, lastGood: previous)
    }

    func loadWorkflowCatalog(force: Bool = false) async {
        let key = workflowCatalogKey
        if !force, workflowCatalogReadings[key]?.value != nil { return }
        let previous = workflowCatalogReadings[key]?.value
        let result: Result<[WorkflowCatalogEntry], Error>
        do { result = .success(try await query.workflowCatalog(cwd: repoPath, wave: workflowWave)) }
        catch { result = .failure(error) }
        workflowCatalogReadings[key] = reading(from: result, lastGood: previous)
    }

    func workflowSource(_ name: String, wave: WaveSnapshot) async throws -> String {
        return try await query.workflowSource(name, wave: wave.id, cwd: WaveOrigin.resolve(wave.repo))
    }

    func saveWorkflow(_ name: String, content: String, wave: WaveSnapshot) async throws {
        try await query.saveWorkflow(name, content: content, wave: wave.id, cwd: WaveOrigin.resolve(wave.repo))
        await refresh()
        await loadWorkflowCatalog(force: true)
    }

    /// Why a Wave's last workflow or source change was refused, by Wave.
    private(set) var workflowErrors: [String: String] = [:]

    /// Make `name` the workflow of the Wave's current chapter, then reread planning.
    func setWorkflow(_ name: String, wave: WaveSnapshot) async {
        do {
            try await query.setWorkflow(name, wave: wave.id, cwd: WaveOrigin.resolve(wave.repo))
            workflowErrors[wave.id] = nil
            await refresh()
        } catch {
            workflowErrors[wave.id] = error.localizedDescription
        }
    }

    func definitionSource(_ entry: FlowCatalogEntry) async throws -> URL {
        let path = try await query.customizeFlow(entry.name, cwd: repoPath)
        await loadFlowCatalog(force: true)
        return URL(fileURLWithPath: path)
    }

    /// Launch a fresh Flow for the Task, then refresh the shared reading.
    /// Without `flow`, Rust takes up the Project's workflow or the only edge.
    /// The outcome settles only the Task and repository that started it; a
    /// refusal is kept on that Task's draft and changes nothing else.
    func startTask(_ flow: String?, task: RoadmapTask, wave: WaveSnapshot) async {
        await performTaskAction(task.id) {
            try await query.runTask(issue: task.task.identifier, flow: flow, cwd: WaveOrigin.resolve(wave.repo))
        }
    }

    /// Put the Task at a node of its Workflow, then refresh the shared
    /// reading. A refusal is kept on that Task's draft.
    func moveTask(to node: String, task: RoadmapTask, wave: WaveSnapshot) async {
        await performTaskAction(task.id) {
            try await query.moveTask(
                issue: task.task.identifier, node: node, cwd: WaveOrigin.resolve(wave.repo))
        }
    }

    func pauseTask(_ task: RoadmapTask, wave: WaveSnapshot, paused: Bool) async {
        await performTaskAction(task.id) {
            try await query.pauseTask(issue: task.task.identifier, paused: paused, cwd: WaveOrigin.resolve(wave.repo))
        }
    }

    func completeTask(_ task: RoadmapTask, wave: WaveSnapshot) async {
        await performTaskAction(task.id) {
            try await query.completeTask(issue: task.task.identifier, cwd: WaveOrigin.resolve(wave.repo))
        }
    }

    /// Feedback belongs to the navigation and Task that began the command.
    private func performTaskAction(_ taskId: String, _ action: () async throws -> Void) async {
        let owner = navigation
        guard owner.taskRunDrafts[taskId]?.acting != true else { return }
        owner.taskRunDrafts[taskId, default: TaskRunDraft()].acting = true
        owner.taskRunDrafts[taskId]?.error = nil
        do {
            try await action()
            owner.taskRunDrafts[taskId] = nil
            await refresh()
        } catch {
            owner.taskRunDrafts[taskId]?.acting = false
            owner.taskRunDrafts[taskId]?.error = error.localizedDescription
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
    private func currentTask(id: String) -> (wave: WaveRoadmap, task: RoadmapTask)? {
        for wave in visibleRoadmaps {
            if let task = wave.tasks.items.first(where: { $0.id == id || $0.runtime?.workId == id || $0.task.identifier == id }) {
                return (wave, task)
            }
        }
        return nil
    }

    func task(id: String) -> (wave: WaveRoadmap, task: RoadmapTask)? {
        if let current = currentTask(id: id) { return current }
        if let retained = navigation.selectedTaskEvidence, (retained.task.id == id || retained.task.runtime?.workId == id) {
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
        if fixed && AppTestMode.current() != .sessionFixtures { self.sessions = .available([]) }
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
