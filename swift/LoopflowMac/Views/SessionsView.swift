#if os(macOS)
import AppKit
import Combine
import Loopflow
import OSLog
import SwiftUI

extension Notification.Name {
    static let openSessions = Notification.Name("loopflow.openSessions")
}

/// The pane whose strip the pointer is over: only that pane shows its trio.
@MainActor
@Observable
final class PaneHover {
    var paneId: String?
}

extension PaneState {
    /// Content owns terminal identity; workspace placement grants no location fallback.
    var terminal: TerminalIdentity? {
        switch content {
        case .shell(let location, _): location.map { .shell(id, machineId: $0.machineId) }
        case .session(let id, let location, _): location.map { .session(id, machineId: $0.machineId) }
        case .empty, .files, .flowLog: nil
        }
    }
}

/// One window's retained workspace for Work: the pane layout and the
/// pool of live surfaces behind it.
@MainActor
@Observable
final class SessionsWorkspace {
    var conversationRequest: UUID?
    var openingConversation: Bool { conversationRequest != nil }
    var conversationError: String?
    var didConsiderRepositoryEntry = false
    private var didPrepareTaskPanes = false
    private var retainedShells: [String: (content: PaneContent, command: [String])] = [:]

    /// Populate this retained Work even if navigation changes while lf prepares it.
    func openPrimaryConversation(wave: String? = nil, store: SessionsStore, registry: SessionsWorkspaceRegistry) async {
        guard !openingConversation else { return }
        let request = UUID()
        conversationRequest = request
        conversationError = nil
        defer { if conversationRequest == request { conversationRequest = nil } }
        do {
            let record = try await store.query.ensurePrimarySession(wave: wave, cwd: store.repoPath)
            store.reconcile([record])
            registry.revealSession(record, in: self)
            presentation.content = .terminals
            presentation.selectedSessionId = record.id
            await store.select(record.id)
        } catch { conversationError = error.localizedDescription }
    }

    func toggleFocus(_ paneId: String) {
        Perf.begin(Perf.retainedWorkspaceAction, multiplexer.zoomedPaneId == nil ? "focus" : "restore", id: "workspace")
        multiplexer.toggleZoom(paneId)
    }
    func setCollapsed(paneId: String, collapsed: Bool) {
        Perf.begin(Perf.retainedWorkspaceAction, collapsed ? "collapse" : "expand", id: "workspace")
        multiplexer.setCollapsed(paneId: paneId, collapsed: collapsed)
    }

    var key: WorkWorkspaceKey
    var title: String
    var location: WorkspaceIdentity?
    var presentation = WorkNavigation.WorkspacePresentation()
    let localMachineId: String?

    func prepareTaskPanes(query: RegistryQuery = RegistryQueryLocal.shared) {
        guard key.kind == .task, !didPrepareTaskPanes else { return }
        let panes = multiplexer.layout.allPanes
        guard panes.count == 1, let session = panes.first,
              session.content != .empty, session.content.location != nil else { return }
        didPrepareTaskPanes = true
        let content = PaneContent.files(taskId: key.id,
            location: session.content.location, execution: session.executionLocation)
        // A location error is presented by the browser's normal error view.
        _ = try? files(for: content, task: nil, query: query)
        guard let browser = multiplexer.show(content, beside: session.id, focus: false) else { return }
        multiplexer.updateRatio(between: session.id, and: browser.id, ratio: LayoutNode.goldenRatio)
    }

    @discardableResult
    func newShell(beside pane: String? = nil, focus: Bool = true) throws -> PaneState? {
        let (location, execution) = try shellLocation(beside: pane ?? multiplexer.focusedPaneId)
        let command = try shellCommand(at: location, execution: execution)
        return multiplexer.newShell(command: command, location: location, execution: execution, beside: pane, focus: focus)
    }

    /// A selection changes a retained companion; adding a split stays explicit.
    func selectContent(_ content: PaneContent, in pane: PaneState, containsSession: Bool = false) throws {
        guard multiplexer.layout.pane(for: pane.id)?.incarnation == pane.incarnation else { return }
        if isSessionPane(pane) || containsSession {
            if let companion = multiplexer.layout.allPanes.first(where: { $0.id != pane.id && !isSessionPane($0) }) {
                try selectContent(content, in: companion)
            } else if case .shell = content {
                try newShell(beside: pane.id)
            } else {
                multiplexer.show(content, beside: pane.id)
            }
            return
        }
        // One retained document view can only be mounted in one pane at a time.
        if let visible = multiplexer.layout.allPanes.first(where: { $0.id != pane.id && $0.content == content }) {
            if case .files = content { multiplexer.setFocusedPane(visible.id); return }
            if case .flowLog = content { multiplexer.setFocusedPane(visible.id); return }
        }
        var replacement = content
        var command: [String]?
        if case .shell = content {
            if let retained = retainedShells[pane.id] {
                replacement = retained.content
                command = retained.command
            } else {
                let (location, execution) = try shellLocation(beside: pane.id)
                replacement = .shell(location: location, execution: execution)
                command = try shellCommand(at: location, execution: execution)
            }
        }
        guard pane.content != replacement else { multiplexer.setFocusedPane(pane.id); return }
        if case .shell = pane.content {
            retainedShells[pane.id] = (pane.content, multiplexer.shellCommands[pane.id] ?? [])
        }
        surfaces.clearFocus()
        multiplexer.replace(pane.id, with: replacement, shellCommand: command)
    }

    private func isSessionPane(_ pane: PaneState) -> Bool {
        pane.sessionId != nil || pane.terminal.flatMap { surfaces.programStatus(for: $0)?.sessionId } != nil
    }

    func closePane(_ paneId: String) {
        let pane = multiplexer.layout.pane(for: paneId)
        if let retained = retainedShells.removeValue(forKey: paneId), let location = retained.content.location {
            surfaces.release(.shell(paneId, machineId: location.machineId))
        } else if let pane, case .shell = pane.content, let terminal = pane.terminal {
            surfaces.release(terminal)
        }
        multiplexer.close(paneId)
    }

    func browserPage(for taskId: String) -> TaskFilesStore.Page {
        taskFiles[taskId]?.store.page ?? .files
    }

    func selectFilesPage(taskId: String) {
        taskFiles[taskId]?.store.page = .files
    }

    @discardableResult
    func split(_ pane: String, axis: SplitAxis, focus: Bool = true) throws -> PaneState? {
        let (location, execution) = try shellLocation(beside: pane)
        let command = try shellCommand(at: location, execution: execution)
        return multiplexer.split(pane, axis: axis, command: command, location: location, execution: execution, focusNewPane: focus)
    }

    private func shellLocation(beside paneId: String) throws -> (WorkspaceIdentity, TaskLocationObservation?) {
        guard let pane = multiplexer.layout.pane(for: paneId) else {
            throw RegistryQueryError("The addressed pane disappeared; no shell was opened.")
        }
        if pane.content == .empty {
            guard let location else {
                throw RegistryQueryError("Prepare this Work's location before opening a shell.")
            }
            return (location, nil)
        }
        guard let location = pane.content.location else {
            throw RegistryQueryError("Pane location is unavailable; no shell was opened.")
        }
        return (location, pane.executionLocation)
    }

    private func shellCommand(at identity: WorkspaceIdentity, execution: TaskLocationObservation?) throws -> [String] {
        if identity.machineId == localMachineId { return [] }
        guard let location = execution, location.machineID == identity.machineId,
              case .recorded(let task, let checkout) = location.location,
              checkout == identity.worktree else {
            throw RegistryQueryError("Reopen the remote Task before adding a shell; no local shell was opened.")
        }
        return [try LocalWaveAgentLauncher.controlLfPath(), "--machine", location.machineID,
            "--repository", location.repositoryID, "task", "shell", task,
            "--checkout", identity.worktree]
    }
    let multiplexer = MultiplexerStore()
    let hover = PaneHover()
    private var taskFiles: [String: (store: TaskFilesStore, execution: TaskLocationObservation?)] = [:]

    func files(taskId: String, issue: String, cwd: String, query: RegistryQuery = RegistryQueryLocal.shared, execution: TaskLocationObservation? = nil) -> TaskFilesStore {
        if let files = taskFiles[taskId] { return files.store }
        let files = TaskFilesStore(issue: issue, cwd: cwd, query: query)
        taskFiles[taskId] = (files, execution)
        return files
    }
    /// Resolve Files from its content, not the selected checkout or a previous link.
    func files(for content: PaneContent, task: RoadmapTask?, query: RegistryQuery) throws -> TaskFilesStore {
        guard case .files(let taskId, let location, let execution) = content else {
            throw RegistryQueryError("The addressed pane is not Files.")
        }
        let issue: String
        let checkout: String
        if let execution {
            guard [taskId, task?.runtime?.workId, task?.task.identifier].compactMap({ $0 }).contains(execution.taskID) else {
                throw RegistryQueryError("The pane's recorded owner belongs to another Task; no files were read.")
            }
            guard case .recorded(let ownerTask, let path?) = execution.location else {
                throw RegistryQueryError("Task location is unavailable. Retained drafts have not moved.")
            }
            issue = ownerTask
            checkout = path
        } else {
            guard let recorded = location, recorded.machineId == localMachineId,
                  query.remoteMachine == nil else {
                throw RegistryQueryError("Task file location is unavailable. Reopen the Task; no local fallback was used.")
            }
            if let current = task?.reference.workspace?.identity, current != recorded {
                throw RegistryQueryError("The Task file location changed. Its previous drafts are retained, not redirected.")
            }
            issue = task?.runtime?.workId ?? taskId
            checkout = recorded.worktree
        }
        if let entry = taskFiles[taskId] {
            let retained = entry.store
            guard entry.execution?.repositoryID == execution?.repositoryID,
                  entry.execution?.taskID == execution?.taskID,
                  entry.execution?.location == execution?.location,
                  retained.cwd == checkout,
                  retained.query.remoteMachine == (execution?.machineID ?? query.remoteMachine),
                  execution == nil || retained.issue == issue else {
                throw RegistryQueryError("The Task file location changed. Its previous drafts are retained, not redirected.")
            }
            return retained
        }
        // Retained documents keep their original transport; build it only once.
        let fileQuery = execution.map {
            query.onMachine($0.machineID, repository: $0.repositoryID).pinTaskLocation(to: $0)
        } ?? query
        return files(taskId: taskId, issue: issue, cwd: checkout, query: fileQuery, execution: execution)
    }
    let surfaces: GhosttySurfacePool
    private var surfaceClosed: AnyCancellable?

    private var retainedStore: SessionsStore?

    func sessionStore(repoPath: String, query: RegistryQuery) -> SessionsStore {
        if let retainedStore {
            retainedStore.repoPath = WaveOrigin.resolve(repoPath)
            return retainedStore
        }
        let store = SessionsStore(repoPath: repoPath, query: query, surfaces: surfaces)
        retainedStore = store
        return store
    }

    init(key: WorkWorkspaceKey, title: String, location: WorkspaceIdentity?, localMachineId: String?, surfaces: GhosttySurfacePool = GhosttySurfacePool()) {
        self.key = key
        self.title = title
        self.location = location
        self.localMachineId = localMachineId
        self.surfaces = surfaces
        // The workspace outlives SessionsView, including while a shell exits
        // on the Work screen or while another repository is selected.
        surfaceClosed = NotificationCenter.default.publisher(for: .ghosttySurfaceClosed)
            .sink { [weak self] notification in
                guard let terminal = notification.object as? TerminalIdentity,
                      case .shell(let paneId, _) = terminal else { return }
                MainActor.assumeIsolated {
                    guard let self else { return }
                    if self.retainedShells[paneId]?.content.location?.machineId == terminal.machineId {
                        self.retainedShells.removeValue(forKey: paneId)
                    }
                    guard self.multiplexer.layout.pane(for: paneId)?.terminal == terminal else { return }
                    self.multiplexer.close(paneId)
                }
            }
    }
}

/// Owned by each window's root view. Keying workspaces per window keeps
/// Sessions↔Work navigation and repo switches lossless inside that window
/// while never sharing an NSView or multiplexer across windows — a Session
/// viewed in another window is just another "active elsewhere" client.
@MainActor
@Observable
final class SessionsWorkspaceRegistry {
    // Observable only for environment delivery; lazy creation must not invalidate views.
    @ObservationIgnored private var workspaces: [WorkWorkspaceKey: SessionsWorkspace] = [:]
    let surfaces = GhosttySurfacePool()
    private(set) var localMachineId: String?
    private(set) var machineError: String?

    init(localMachineId: String? = nil) { self.localMachineId = localMachineId }

    func sessionStore(repository: String, repoPath: String, query: RegistryQuery) -> SessionsStore {
        let path = WaveOrigin.resolve(repoPath)
        let root = WorkWorkspaceKey(repository: repository, kind: .repository, id: repository)
        return workspace(for: root, title: URL(fileURLWithPath: path).lastPathComponent,
            location: localMachineId.map { WorkspaceIdentity(machineId: $0, worktree: path) })
            .sessionStore(repoPath: path, query: query)
    }

    func refreshMachine(query: RegistryQuery) async {
        do { localMachineId = try await query.localMachineId(); machineError = nil }
        catch { machineError = error.localizedDescription }
    }

    private(set) var layout: WorkLayout?
    var openWork: [WorkWorkspaceKey] { layout?.keys ?? [] }
    private(set) var selectedWork: WorkWorkspaceKey?

    var keys: [WorkWorkspaceKey] { Array(workspaces.keys) }

    func retained(_ key: WorkWorkspaceKey) -> SessionsWorkspace? { workspaces[key] }

    @discardableResult
    func open(_ key: WorkWorkspaceKey, title: String, location: WorkspaceIdentity?, besideFocused: Bool = false) -> SessionsWorkspace {
        let workspace = workspace(for: key, title: title, location: location)
        if selectedWork != key { surfaces.clearFocus() }
        if !openWork.contains(key) {
            if let layout, let focused = selectedWork, layout.keys.contains(focused), focused.repository == key.repository {
                self.layout = layout.replacing(focused, with: besideFocused
                    ? .split(.vertical, .workspace(focused), .workspace(key), 0.5) : .workspace(key))
            } else { layout = .workspace(key) }
        }
        selectedWork = key
        return workspace
    }

    func hide(_ key: WorkWorkspaceKey) {
        guard let index = openWork.firstIndex(of: key) else { return }
        layout = layout?.removing(key)
        if selectedWork == key {
            surfaces.clearFocus()
            selectedWork = openWork.isEmpty ? nil : openWork[max(0, index - 1)]
        }
    }

    func resize(between first: WorkWorkspaceKey, and second: WorkWorkspaceKey, ratio: Double) {
        layout = layout?.resizing(between: first, and: second, ratio: ratio)
    }

    func inspect() -> [DesktopWorkspaceInspection] {
        workspaces.values.sorted { ($0.key.repository, $0.key.kind.rawValue, $0.key.id) < ($1.key.repository, $1.key.kind.rawValue, $1.key.id) }.map { workspace in
            let panes = workspace.multiplexer
            return DesktopWorkspaceInspection(work: workspace.key, title: workspace.title,
                isOpen: openWork.contains(workspace.key), selected: selectedWork == workspace.key,
                layout: DesktopLayoutInspection(panes.layout) { pane in
                    pane.terminal.flatMap { surfaces.surfaceIncarnation(for: $0) }
                }, focusedPane: panes.focusedPaneId,
                zoomedPane: panes.zoomedPaneId, hiddenPanes: panes.collapsedPaneIds.sorted())
        }
    }

    /// Record the Session owner; only explicit Changes opens a Files companion.
    @discardableResult
    func openLinkedFiles(_ destination: LinkedSession, in key: WorkWorkspaceKey, query: RegistryQuery) throws -> TaskFilesStore? {
        guard let identity = destination.record.workspace?.identity else { return nil }
        let workspace = workspace(for: key, title: destination.task.task.name, location: identity)
        workspace.multiplexer.updateSessionLocation(destination.record.id, location: identity, execution: destination.location)
        guard destination.showsChanges else { return nil }
        let content = PaneContent.files(taskId: destination.task.id, location: identity, execution: destination.location)
        let files = try workspace.files(for: content, task: destination.task, query: query)
        files.showsChanges = true
        files.page = .files
        if let pane = workspace.multiplexer.pane(forSessionId: destination.record.id) {
            try workspace.selectContent(content, in: pane)
        } else { workspace.multiplexer.show(content, focus: false) }
        return files
    }

    func readText(_ request: DesktopTextRequest) throws -> DesktopTextReading {
        let (workspace, pane) = try retainedPane(for: request.target)
        let store = workspace.multiplexer
        let result: DesktopTextResult
        if let terminal = pane.terminal {
            result = try surfaces.readText(request, terminal: terminal)
        } else {
            result = .unavailable(reason: .notTerminal)
        }
        return DesktopTextReading(request: request, observedAt: Int64(Date().timeIntervalSince1970),
            hidden: !openWork.contains(workspace.key) || !store.isVisible(pane),
            result: result)
    }

    @discardableResult
    func controlPane(_ request: DesktopPaneCommand, model: WorkModel) throws -> String {
        let target = request.target
        let (workspace, pane) = try retainedPane(for: target)
        let store = workspace.multiplexer
        // Validate both ends before changing anything. Moving a view is not
        // reassociating its Work or transferring a client to another checkout.
        func peer(_ other: DesktopPaneTarget) throws {
            guard other.repository == target.repository, other.window == target.window,
                  other.work == target.work,
                  other.pane != target.pane else {
                throw RegistryQueryError("Choose two distinct panes in the same retained workspace; no pane was changed.")
            }
            _ = try retainedPane(for: other)
        }
        var result: PaneState? = pane
        switch request.action {
        case .text(let surface, let text):
            guard let terminal = pane.terminal else {
                throw RegistryQueryError("The addressed pane is not a terminal; no input was sent.")
            }
            try surfaces.insertText(text, terminal: terminal, surface: surface)
        case .key(let surface, let key):
            guard let terminal = pane.terminal else {
                throw RegistryQueryError("The addressed pane is not a terminal; no input was sent.")
            }
            try surfaces.sendKey(key, terminal: terminal, surface: surface)
        case .shell:
            result = try workspace.newShell(beside: target.pane, focus: false)
        case .files(let task), .flowLog(let task):
            guard let projection = model.task(id: task), workspace.key.kind == .task,
                  (workspace.key.id == projection.task.id || workspace.key.id == projection.task.runtime?.workId),
                  let identity = pane.content.location ?? workspace.location,
                  (model.navigation.preparedTaskWorktrees[projection.task.id]
                    ?? projection.task.reference.workspace?.identity ?? identity) == identity else {
                throw RegistryQueryError("The Task is not associated with this checkout. Inspect its recorded workspace; no pane was changed.")
            }
            let content: PaneContent = if case .files = request.action {
                .files(taskId: projection.task.id, location: identity, execution: pane.executionLocation)
            } else {
                .flowLog(taskId: projection.task.id, location: identity, execution: pane.executionLocation)
            }
            if case .files = request.action { workspace.selectFilesPage(taskId: projection.task.id) }
            result = store.show(content, beside: target.pane, focus: false)
        case .hide: workspace.setCollapsed(paneId: target.pane, collapsed: true)
        case .restore: workspace.setCollapsed(paneId: target.pane, collapsed: false)
        case .focus:
            open(workspace.key, title: workspace.title, location: workspace.location)
            model.select(workspace.key.reference)
            store.setFocusedPane(target.pane)
            if let terminal = pane.terminal { surfaces.focus(terminal) }
        case .split(let axis): result = try workspace.split(target.pane, axis: axis, focus: false)
        case .move(let destination, let axis):
            try peer(destination)
            store.move(target.pane, beside: destination.pane, axis: axis)
        case .resize(let toward, let ratio):
            try peer(toward)
            guard ratio.isFinite, (0.1...0.9).contains(ratio) else {
                throw RegistryQueryError("Split ratio must be between 0.1 and 0.9; no pane was changed.")
            }
            store.updateRatio(between: target.pane, and: toward.pane, ratio: ratio)
        case .zoom(let enabled): store.setZoom(target.pane, enabled: enabled)
        }
        guard let result else { throw RegistryQueryError("The addressed pane disappeared; inspect Desktop before retrying.") }
        return result.id
    }

    private func retainedPane(for target: DesktopPaneTarget) throws -> (SessionsWorkspace, PaneState) {
        guard target.repository == target.work.repository,
              let workspace = workspaces[target.work],
              let pane = workspace.multiplexer.layout.pane(for: target.pane),
              pane.incarnation == target.incarnation else {
            throw RegistryQueryError("The pane content was removed or replaced. Inspect Desktop again; no pane was changed.")
        }
        return (workspace, pane)
    }

    func key(containingShell id: String, machineId: String) -> WorkWorkspaceKey? {
        workspaces.first {
            guard let pane = $0.value.multiplexer.layout.pane(for: id) else { return false }
            return pane.terminal == .shell(id, machineId: machineId)
        }?.key
    }

    /// A partial inventory can update content location, never remove a Work tab.
    func reconcileMembership(_ records: [SessionRecord]) {
        for workspace in workspaces.values {
            for record in records {
                let belongs: Bool?
                switch workspace.key.kind {
                case .task:
                    let tasks = Set(record.taskIds + [record.workspace?.taskId, record.work?.kind == .task ? record.work?.id : nil].compactMap { $0 })
                    belongs = tasks.isEmpty ? nil : tasks.contains(workspace.key.id)
                case .wave: belongs = record.waveId.map { $0 == workspace.key.id }
                case .repository: belongs = nil
                }
                // A peer's physical Task ids cannot disprove the local Work
                // correspondence carried by an explicitly opened remote pane.
                if belongs == false, record.workspace?.machineId == localMachineId {
                    workspace.multiplexer.removeSessions([record.id])
                }
                else { workspace.multiplexer.updateSessionLocation(record.id, location: record.workspace?.identity) }
            }
        }
    }

    /// One native occurrence per window. A deliberate eligible Work opening can
    /// relocate presentation, but neither copies a surface nor binds the Session.
    func revealSession(_ record: SessionRecord, in workspace: SessionsWorkspace) {
        for other in workspaces.values where other !== workspace {
            other.multiplexer.removeSessions([record.id])
        }
        workspace.multiplexer.reveal(sessionId: record.id, location: record.workspace?.identity)
    }

    func reassociateRepository(from previous: String, to repository: String) {
        for key in keys where key.repository == previous {
            canonicalize(key, as: WorkWorkspaceKey(repository: repository, kind: key.kind,
                id: key.kind == .repository ? repository : key.id))
        }
    }

    func key(containingSession id: String) -> WorkWorkspaceKey? {
        workspaces.first { $0.value.multiplexer.pane(forSessionId: id) != nil }?.key
    }

    func canonicalize(_ previous: WorkWorkspaceKey, as key: WorkWorkspaceKey) {
        guard previous != key, workspaces[key] == nil, let workspace = workspaces.removeValue(forKey: previous) else { return }
        workspace.key = key
        workspaces[key] = workspace
        layout = layout?.replacing(previous, with: .workspace(key))
        if selectedWork == previous { selectedWork = key }
    }

    func workspace(for key: WorkWorkspaceKey, title: String, location: WorkspaceIdentity?) -> SessionsWorkspace {
        if let existing = workspaces[key] {
            if existing.title != title { existing.title = title }
            if let location, existing.location != location { existing.location = location }
            return existing
        }
        let workspace = SessionsWorkspace(key: key, title: title, location: location,
            localMachineId: localMachineId, surfaces: surfaces)
        workspaces[key] = workspace
        return workspace
    }

}

struct SessionItem: Identifiable, Equatable {
    enum State: Equatable {
        case pending
        case elsewhere
        case opening
        case prepared
        case live
        case failed(String)
        case ended(String)
    }

    var record: SessionRecord
    var state: State
    var agentOverride: String?
    var launchId = UUID()

    var id: String { record.id }

    var launchArguments: [String] {
        record.openArgv + (agentOverride.map { ["--agent", $0] } ?? [])
    }

    var surface: SessionRecord? {
        switch state {
        case .prepared, .live: record
        case .pending, .elsewhere, .opening, .failed, .ended: nil
        }
    }

    var error: String? {
        guard case .failed(let message) = state else { return nil }
        return message
    }
}

@MainActor
@Observable
final class SessionsStore {
    private(set) var sessions: [SessionItem] = []

    let surfaces: GhosttySurfacePool

    var repoPath: String
    let query: RegistryQuery
    private var executionLocations: [String: TaskLocationObservation] = [:]

    func executionLocation(for id: String) -> TaskLocationObservation? { executionLocations[id] }

    func retainLocation(_ location: TaskLocationObservation, for record: SessionRecord) {
        executionLocations[record.id] = location
    }

    func query(for record: SessionRecord) -> RegistryQuery {
        guard let location = executionLocations[record.id] else { return query }
        return query.onMachine(location.machineID, repository: location.repositoryID)
    }

    func taskSubject(for record: SessionRecord) -> WorkReference? {
        executionLocations[record.id].map { .task(id: $0.taskID) }
    }

    func presentationDirectory(for record: SessionRecord, checkout: String) -> String {
        executionLocations[record.id] == nil ? checkout : repoPath
    }

    private let metrics: SessionsLatencyMetrics
    @ObservationIgnored private var hasRecordedSessionsLoad = false

    init(
        repoPath: String,
        query: RegistryQuery = RegistryQueryLocal.shared,
        surfaces: GhosttySurfacePool = GhosttySurfacePool()
    ) {
        self.repoPath = WaveOrigin.resolve(repoPath)
        self.query = query
        self.surfaces = surfaces
        metrics = SessionsLatencyMetrics(scope: URL(fileURLWithPath: repoPath).lastPathComponent)
    }

    func reconcile(_ records: [SessionRecord]) {
        if !hasRecordedSessionsLoad {
            metrics.recordSessionsLoaded(count: records.count)
            hasRecordedSessionsLoad = true
        }
        // Working-set pages are filtered. Absence establishes neither deletion
        // nor completion and must not discard prepared commands or native drafts.

        for record in records {
            let observedState: SessionItem.State = localTerminal(for: record) != nil
                ? .live : record.action(.moveHere) != nil ? .elsewhere : .pending
            if let index = _index(record.id) {
                // Keep a prepared command until the retained native view consumes it.
                if case .prepared = sessions[index].state {
                    sessions[index].record.workspace = record.workspace
                    continue
                }
                sessions[index].record = record
                if case .opening = sessions[index].state { continue }
                if observedState == .pending {
                    switch sessions[index].state {
                    case .failed, .ended: continue
                    default: break
                    }
                }
                sessions[index].state = observedState
            } else {
                sessions.append(SessionItem(record: record, state: observedState))
            }
        }
        surfaces.associateProgramStatus(sessions.map(\.record))
    }

    private func recover(_ id: String, replacing: Bool = false) async {
        guard let index = _index(id),
              let action = sessions[index].record.action(replacing ? .moveHere : .open) else { return }
        // A cached observation failure can outlive the client that caused it.
        // An explicit open revalidates through connect; it never replaces a client.
        if replacing, action.unavailableReason != nil { return }
        switch sessions[index].state {
        case .pending, .failed, .ended:
            sessions[index].state = .opening
        case .elsewhere where replacing:
            sessions[index].state = .opening
        case .elsewhere, .opening, .prepared, .live:
            return
        }
        do {
            let record = sessions[index].record
            let executionQuery = query(for: record)
            if let location = executionLocations[id] {
                let current = try await executionQuery.taskLocation(task: location.taskID, repository: location.repositoryID, machine: location.machineID)
                guard current.location == location.location else {
                    throw RegistryQueryError("The recorded execution location changed; reopen the Task before connecting.")
                }
            }
            Perf.agentInput("connect_prepare_begin", session: id)
            let surface = try await executionQuery.prepareSession(
                id: id,
                takeover: replacing,
                cwd: repoPath
            )
            Perf.agentInput("connect_prepare_returned", session: id)
            guard let latest = _index(id) else { return }
            if case .opening = sessions[latest].state {
                sessions[latest].record = surface
                if !replacing, surface.action(.moveHere) != nil {
                    sessions[latest].state = .elsewhere
                } else if let reason = surface.action(replacing ? .moveHere : .open)?.unavailableReason {
                    sessions[latest].state = .failed(reason)
                } else {
                    sessions[latest].state = .prepared
                }
            }
        } catch {
            guard let latest = _index(id) else { return }
            sessions[latest].state = .failed(error.localizedDescription)
        }
    }

    func adoptPreparation(_ record: SessionRecord) {
        guard let index = _index(record.id), localTerminal(for: record) == nil else { return }
        switch sessions[index].state {
        case .pending, .failed, .ended:
            sessions[index].record = record
            if record.action(.moveHere) != nil {
                sessions[index].state = .elsewhere
            } else if let reason = record.action(.open)?.unavailableReason {
                sessions[index].state = .failed(reason)
            } else {
                sessions[index].state = .prepared
            }
        case .elsewhere, .opening, .prepared, .live:
            break
        }
    }

    func select(_ id: String) async {
        guard let index = _index(id) else { return }
        if localTerminal(for: sessions[index].record) != nil {
            sessions[index].state = .live
            return
        }
        if case .live = sessions[index].state {
            sessions[index].state = sessions[index].record.action(.moveHere) != nil
                ? .elsewhere : .pending
        }
        await recover(id)
    }

    func moveHere(_ id: String) async {
        await recover(id, replacing: true)
    }

    @discardableResult
    func switchProvider(_ id: String, to agent: String) async throws -> SessionRecord {
        guard let index = _index(id) else { throw RegistryQueryError("Session is unavailable.") }
        let previous = sessions[index]
        sessions[index].state = .opening
        do {
            let prepared = try await query(for: previous.record).prepareSession(id: id, takeover: true, cwd: repoPath)
            if let reason = prepared.action(.moveHere)?.unavailableReason {
                throw RegistryQueryError(reason)
            }
            guard let current = _index(id) else { throw RegistryQueryError("Session is unavailable.") }
            if let terminal = localTerminal(for: previous.record) { surfaces.release(terminal) }
            sessions[current].record = prepared
            sessions[current].agentOverride = agent
            sessions[current].launchId = UUID()
            sessions[current].state = .prepared
            return prepared
        } catch {
            if let current = _index(id) { sessions[current] = previous }
            throw error
        }
    }

    func reset(_ id: String) async throws -> SessionRecord {
        guard let index = _index(id) else { throw RegistryQueryError("Session is unavailable.") }
        let previous = sessions[index]
        sessions[index].state = .opening
        do {
            let successor = try await query(for: previous.record).resetSession(
                id: id, agent: previous.record.provider, cwd: repoPath)
            if let terminal = localTerminal(for: previous.record) { surfaces.release(terminal) }
            if let location = executionLocations[id] { retainLocation(location, for: successor) }
            reconcile([successor])
            adoptPreparation(successor)
            if let current = _index(id) { sessions[current].state = .ended("Replaced by a fresh conversation.") }
            return successor
        } catch {
            if let current = _index(id) { sessions[current] = previous }
            throw error
        }
    }

    func beginPaneLoad(_ id: String) {
        metrics.beginPaneLoad(id)
    }

    func localTerminal(for record: SessionRecord) -> TerminalIdentity? {
        guard let machineId = record.workspace?.machineId else { return nil }
        let session = TerminalIdentity.session(record.id, machineId: machineId)
        if surfaces.hasSurface(session) { return session }
        return record.terminalIds.lazy.map { TerminalIdentity.shell($0, machineId: machineId) }
            .first(where: surfaces.hasSurface)
    }

    func recordPaneLive(_ id: String) {
        metrics.recordPaneLive(id)
        if let index = _index(id), case .prepared = sessions[index].state {
            sessions[index].state = .live
        }
    }

    func recordPaneFailure(_ id: String, reason: String) {
        guard let index = _index(id), sessions[index].state == .prepared else { return }
        sessions[index].state = .failed(reason)
    }

    /// A retained surface's child ended — provider exit or an external
    /// takeover. Reclassify at once so the pane swaps to its placeholder
    /// instead of presenting a dead terminal until the next poll. A window
    /// whose own pool still holds a live surface for this Session (it just
    /// took the client over) is unaffected.
    func noteSurfaceClosed(_ terminal: TerminalIdentity, output: String? = nil) {
        guard case .session(let id, let machineId) = terminal,
              !surfaces.hasSurface(terminal),
              let index = _index(id), sessions[index].record.workspace?.machineId == machineId
        else { return }
        switch sessions[index].state {
        case .prepared, .live:
            sessions[index].state = sessions[index].record.action(.moveHere) != nil
                ? .elsewhere : .ended(output ?? "Terminal output is unavailable.")
        case .pending, .elsewhere, .opening, .failed, .ended:
            break
        }
    }

    private func _index(_ id: String) -> Int? {
        sessions.firstIndex { $0.id == id }
    }
}

@MainActor
private final class SessionsLatencyMetrics {
    private let logger = Logger(subsystem: "studio.loopflow", category: "sessions-latency")
    private let scope: String
    private let sessionsStartedAt = DispatchTime.now()
    private var paneStarts: [String: DispatchTime] = [:]

    init(scope: String) {
        self.scope = scope
    }

    func recordSessionsLoaded(count: Int) {
        let elapsed = Self._milliseconds(since: sessionsStartedAt)
        logger.info(
            "metric=time_to_sessions scope=\(self.scope, privacy: .public) value_ms=\(elapsed, privacy: .public) count=\(count, privacy: .public)"
        )
    }

    func beginPaneLoad(_ id: String) {
        paneStarts[id] = DispatchTime.now()
    }

    func recordPaneLive(_ id: String) {
        guard let start = paneStarts.removeValue(forKey: id) else { return }
        let elapsed = Self._milliseconds(since: start)
        logger.info(
            "metric=time_to_live_pane session=\(id, privacy: .private(mask: .hash)) value_ms=\(elapsed, privacy: .public)"
        )
    }

    private static func _milliseconds(since start: DispatchTime) -> Double {
        Double(DispatchTime.now().uptimeNanoseconds - start.uptimeNanoseconds) / 1_000_000
    }
}

struct SessionsView: View {
    let model: WorkModel
    let repoPath: String
    let workspaces: SessionsWorkspaceRegistry
    var repository: String? = nil
    @State private var resolvedRepository: String?
    @State private var repositoryError: String?
    var query: RegistryQuery = RegistryQueryLocal.shared

    var body: some View {
        let taskPage = model.linkedTaskPage
        Group {
            if let home = workspaces.localMachineId, let repository = repository ?? resolvedRepository {
                SessionsContentView(model: model, repoPath: repoPath, workspaces: workspaces,
                                    machineId: home, repository: repository, query: query)
            } else {
                HStack {
                    if model.navigation.showsNavigator {
                        WorkNavigator(model: model, onOpenSession: { _ in })
                    }
                    if let error = workspaces.machineError ?? repositoryError {
                        ContentUnavailableView("Workspace unavailable", systemImage: "folder", description: Text(error))
                            .task(id: taskPage) {
                                if let request = taskPage { model.finishTaskPage(request, error: error) }
                            }
                    } else {
                        ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                }
            }
        }
        .task {
            await workspaces.refreshMachine(query: query)
            if repository == nil {
                do { resolvedRepository = try await query.repositoryIdentity(cwd: repoPath).id }
                catch { repositoryError = error.localizedDescription }
            }
            if let home = workspaces.localMachineId, workspaces.machineError == nil { model.confirmMachine(home) }
        }
    }
}

/// Unified Work navigation around the existing retained native workspace.
struct SessionsContentView: View {
    @Environment(\.theme) private var theme
    let machineId: String
    private var rootIdentity: WorkspaceIdentity { WorkspaceIdentity(machineId: machineId, worktree: store.repoPath) }
    let repository: String
    private var currentKey: WorkWorkspaceKey { key(for: model.selection) }
    private func key(for reference: WorkReference?) -> WorkWorkspaceKey {
        model.workspaceKey(for: reference, repository: repository)
    }
    private func title(for key: WorkWorkspaceKey) -> String { model.workspaceTitle(key) }
    private var workspace: SessionsWorkspace {
        workspaces.workspace(for: currentKey, title: title(for: currentKey),
            location: currentKey.kind == .task ? taskIdentity : rootIdentity)
    }
    @Bindable var model: WorkModel
    private let workspaces: SessionsWorkspaceRegistry
    private let query: RegistryQuery
    private let store: SessionsStore
    @State private var restorePaletteFocus = true
    @State private var launchError: String?
    @State private var launchErrorTitle = "Could not start conversation"
    @Environment(\.palette) private var palette

    private var multiplexer: MultiplexerStore {
        workspace.multiplexer
    }
    private var taskIdentity: WorkspaceIdentity? {
        if let task = fileTask {
            return navigation.preparedTaskWorktrees[task.task.id] ?? task.task.reference.workspace?.identity
        }
        return selectedWorkspace?.taskId == nil ? nil : selectedWorkspace?.identity
    }
    private var taskPath: String? { taskIdentity?.worktree }
    private var selectedWorkspace: SessionWorkspace? {
        guard model.repoPath?.normalizedFilePath == store.repoPath.normalizedFilePath else { return nil }
        return store.sessions.first { $0.id == navigation.selectedSessionId }?.record.workspace
    }
    init(model: WorkModel, repoPath: String, workspaces: SessionsWorkspaceRegistry,
         machineId: String, repository: String, query: RegistryQuery = RegistryQueryLocal.shared) {
        self.machineId = machineId
        self.repository = repository
        self.model = model
        self.workspaces = workspaces
        self.query = query
        model.bindWorkspaces(workspaces, repository: repository)
        self.store = workspaces.sessionStore(repository: repository, repoPath: repoPath, query: query)
    }

    private var navigation: WorkNavigation { model.navigation }
    private var terminalsVisible: Bool { workspaces.selectedWork != nil && (model.selection?.kind == .task || workspace.presentation.content == .terminals) }
    private var fileTask: TaskProjection? {
        // A departing repository view must not mount the next repository's
        // retained terminals while SwiftUI replaces its hierarchy.
        guard model.repoPath?.normalizedFilePath == store.repoPath.normalizedFilePath else { return nil }
        return model.breadcrumb?.task
    }

    var body: some View {
        VStack(spacing: 0) {
            navigationBar
            GeometryReader { geometry in
                HStack(spacing: 0) {
                    if navigation.showsNavigator {
                        navigator
                            .frame(width: min(navigation.sidebarWidth, max(260, geometry.size.width - 360)))
                        SidebarDivider(navigation: navigation, availableWidth: geometry.size.width)
                    }
                    if let layout = workspaces.layout {
                        WorkLayoutView(layout: layout, registry: workspaces) { key in
                            workspacePanel(key).id(key)
                        }
                        .padding([.horizontal, .bottom], PaneGeometry.windowInset)
                        .background(palette.surfaceMuted)
                        .accessibilityIdentifier("work-layout")
                    } else {
                        ContentUnavailableView("No Work open", systemImage: "rectangle.on.rectangle",
                            description: Text("Open a repository, Wave or Task from the sidebar."))
                    }
                }
                .overlay(alignment: .topLeading) {
                    if navigation.palette == .search && !navigation.showsNavigator {
                        ZStack(alignment: .topLeading) {
                            Color.black.opacity(0.001)
                                .contentShape(Rectangle())
                                .onTapGesture { navigation.palette = nil }
                                .accessibilityIdentifier("work-navigation-dismiss")
                            navigator
                                .frame(width: min(navigation.sidebarWidth, geometry.size.width),
                                       height: max(0, geometry.size.height - 12))
                                .clipShape(RoundedRectangle(cornerRadius: 10))
                                .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(palette.border))
                                .shadow(color: .black.opacity(0.2), radius: 16, x: 4, y: 8)
                                .accessibilityIdentifier("work-navigation-dropdown")
                        }
                    }
                }
            }
        }
        .background(palette.background)
        .tint(palette.actionInk)
        .environment(model)
        .background {
            HStack {
                Button("Back") { model.goBack() }.keyboardShortcut("[", modifiers: .command)
                    .disabled(navigation.backStack.isEmpty)
                Button("Forward") { model.goForward() }.keyboardShortcut("]", modifiers: .command)
                    .disabled(navigation.forwardStack.isEmpty)
            }.hidden().accessibilityHidden(true).frame(width: 0, height: 0)
        }
        .overlay {
            if terminalsVisible, navigation.palette == nil {
                SessionsShortcutMonitor { _handle($0) }
                    .allowsHitTesting(false).frame(width: 0, height: 0)
            }
        }
        .background {
            WorkPaletteShortcut(presented: navigation.palette != nil, restoreFocus: restorePaletteFocus,
                toggleSidebar: { navigation.toggleNavigator() }) {
                restorePaletteFocus = true
                navigation.palette = .search
            }.frame(width: 0, height: 0)
        }
        .sheet(isPresented: Binding(
            get: { navigation.palette != nil && navigation.palette != .search },
            set: { if !$0, navigation.palette != .search { navigation.palette = nil } }
        )) {
            switch navigation.palette {
            case .workflow(let name):
                WorkflowCatalogInspector(entry: model.workflowCatalog.value?.first { $0.name == name }, model: model)
            case .flow(let name):
                FlowCatalogInspector(entry: model.flowCatalog.value?.first { $0.name == name }, navigation: navigation, model: model)
            case .search, nil:
                EmptyView()
            }
        }
        .onChange(of: multiplexer.focusedPaneId) { _, _ in
            guard terminalsVisible, model.repoPath?.normalizedFilePath == store.repoPath.normalizedFilePath else { return }
            if !focusedPaneSessions.contains(where: { $0.id == navigation.selectedSessionId }) {
                navigation.selectedSessionId = focusedPaneSessions.first?.id
            }
        }
        .onChange(of: model.repositoryEntryReady, initial: true) { _, ready in
            guard model.repoPath?.normalizedFilePath == store.repoPath.normalizedFilePath else { return }
            let root = workspaces.workspace(for: key(for: nil), title: title(for: key(for: nil)), location: rootIdentity)
            guard ready, !root.didConsiderRepositoryEntry else { return }
            root.didConsiderRepositoryEntry = true
            guard workspaces.selectedWork == root.key, model.selection == nil else { return }
            Task { await root.openPrimaryConversation(store: store, registry: workspaces) }
        }
        .onChange(of: navigation.showsNavigator) { _, shown in
            if !shown, terminalsVisible, let terminal = multiplexer.focusedPane.terminal {
                workspaces.surfaces.focus(terminal)
            }
        }
        .onChange(of: model.sessions.value, initial: true) { _, records in
            guard model.repoPath?.normalizedFilePath == store.repoPath.normalizedFilePath,
                  let records else { return }
            store.reconcile(records)
            if model.sessions.errorMessage == nil { workspaces.reconcileMembership(records) }
        }

        .onReceive(NotificationCenter.default.publisher(for: .ghosttySurfaceClosed)) { notification in
            guard let terminal = notification.object as? TerminalIdentity else { return }
            store.noteSurfaceClosed(terminal, output: notification.userInfo?["output"] as? String)
        }
        .onReceive(NotificationCenter.default.publisher(for: .openSessions)) { _ in
            navigation.content = .terminals
        }
        .alert(launchErrorTitle, isPresented: Binding(
            get: { launchError != nil },
            set: { if !$0 { launchError = nil } }
        )) {
            Button("OK") { launchError = nil }
        } message: { Text(launchError ?? "") }
        .accessibilityIdentifier("sessions-surface")
    }

    private func focusPane(_ pane: String, session: String?, in workspace: SessionsWorkspace) {
        if currentKey != workspace.key { model.select(workspace.key.reference) }
        workspace.multiplexer.setFocusedPane(pane)
        // Companion shells retain the current conversation context.
        guard let session, navigation.selectedSessionId != session else { return }
        navigation.selectedSessionId = session
        navigation.content = .terminals
    }

    private var navigator: some View {
        WorkNavigator(model: model, onOpenSession: { record in
            dismissNavigationForSelection()
            openSession(record)
        }, onConversation: { work in
            dismissNavigationForSelection()
            model.select(work)
            startConversation()
        }, onNewShell: {
            dismissNavigationForSelection()
            newShell(in: workspace)
            navigation.content = .terminals
        }, onShowTerminals: {
            dismissNavigationForSelection()
            navigation.content = .terminals
        }, onOpenTask: { work in
            dismissNavigationForSelection()
            openTask(work)
        }, onOpenRepository: {
            dismissNavigationForSelection()
            openRepositoryConversation()
        }, onOpenWave: { work in
            dismissNavigationForSelection()
            openWaveConversation(work)
        })
    }

    private func dismissNavigationForSelection() {
        restorePaletteFocus = false
        if navigation.palette == .search { navigation.palette = nil }
    }

    private var navigationBar: some View {
        HStack(spacing: 8) {
            Button {
                restorePaletteFocus = true
                navigation.palette = navigation.palette == .search ? nil : .search
            } label: {
                HStack(spacing: 10) {
                    Image(systemName: "magnifyingglass").foregroundStyle(palette.emphasisInk)
                    Text(title(for: currentKey)).font(Typography.strong(16))
                        .lineLimit(1).truncationMode(.middle)
                    Spacer(minLength: 12)
                    Text("⌘K").font(Typography.code(11)).foregroundStyle(palette.textSecondary)
                    Image(systemName: navigation.palette == .search ? "chevron.up" : "chevron.down")
                        .font(.system(size: 11, weight: .semibold))
                }
                .padding(.horizontal, 14).frame(height: 44)
                .background(palette.surface, in: RoundedRectangle(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(palette.border))
            }
            .frame(maxWidth: navigation.sidebarWidth)
            .buttonStyle(.plain)
            .help("Choose Work (⌘K)").accessibilityIdentifier("work-location-picker")
            .accessibilityValue(title(for: currentKey))
            Spacer(minLength: 0)
            if let task = fileTask, let wave = model.breadcrumb?.wave?.roadmap.wave {
                TaskLifecycleActions(model: model, task: task.task, wave: wave,
                    work: model.taskWork[task.task.id])
                workspaceCreation(for: workspace, task: task.task)
                if multiplexer.zoomedPaneId != nil {
                    Button("Restore") { workspace.toggleFocus(multiplexer.focusedPaneId) }
                }
            }
        }
        .padding(.horizontal, 10).padding(.vertical, 4)
        .background(palette.surfaceMuted)
    }

    private func workspacePanel(_ key: WorkWorkspaceKey) -> some View {
        let task = key.kind == .task ? model.task(id: key.id) : nil
        let location = task.flatMap { navigation.preparedTaskWorktrees[$0.task.id] ?? $0.task.reference.workspace?.identity }
        let entry = workspaces.workspace(for: key, title: title(for: key),
            location: key.kind == .task ? location : rootIdentity)
        let active = currentKey == key
        let crumb = active ? model.breadcrumb : model.visibleWork.breadcrumb(
            selection: key.reference, sessionId: entry.presentation.selectedSessionId)
        let terminals = key.kind == .task || entry.presentation.content == .terminals
        let context = workspaces.openWork.count > 1
        return VStack(spacing: 0) {
            if context {
                HStack(spacing: 8) {
                    HStack(spacing: 6) {
                        if let wave = crumb?.wave {
                            Text(wave.roadmap.wave.displayName).font(Typography.body(10))
                                .foregroundStyle(palette.emphasisInk)
                        }
                        Text(entry.title).font(Typography.strong(12)).lineLimit(1)
                    }
                    Spacer(minLength: 0)
                    if workspaces.openWork.count > 1 {
                        Button { model.hideWorkspace(key) } label: { Image(systemName: "xmark") }
                            .buttonStyle(WorkPlainButtonStyle()).help("Hide workspace; keep its panes and drafts")
                            .accessibilityIdentifier("work-hide-\(key.id)")
                    }
                }
                .padding(.horizontal, 10).padding(.vertical, 4).contentShape(Rectangle()).onTapGesture { model.select(key.reference) }
                .background(palette.surfaceMuted)
                .accessibilityIdentifier("work-context-\(key.id)")
            }
            if active {
                WorkSessionToolbar(model: model, crumb: crumb, onOpenSession: openSession)
            }
            if terminals, let task, active {
                missingCheckout
                    .task(id: model.linkedTaskPage) {
                        if let request = model.linkedTaskPage,
                           request.taskID == task.task.id, request.repo == store.repoPath.normalizedFilePath {
                            model.finishTaskPage(request)
                        }
                    }
            }
            if terminals {
                WorkspacePanesView(workspace: entry, isFocused: active && navigation.palette == nil, sessions: store, model: model,
                    onFocus: { pane, session in focusPane(pane, session: session, in: entry) })
                    .overlay(alignment: .top) {
                        if let error = entry.conversationError {
                            HStack {
                                Text(error).textSelection(.enabled)
                                Button("Retry") {
                                    if key.kind == .task, let work = key.reference { openTask(work) }
                                    else if let work = key.reference { openWaveConversation(work) }
                                    else { openRepositoryConversation() }
                                }
                            }.foregroundStyle(theme.palette.text)
                                .padding(8).background(theme.terminal.background.color)
                        }
                    }
            } else if active {
                HSplitView {
                    WorkSurfaceView(model: model, onOpenSession: openSession, onOpenTask: openTask,
                                    onNewSession: newTaskSession)
                    if navigation.showsActivity { WorkActivityView(model: model).frame(minWidth: 230, maxWidth: 360) }
                }
            } else {
                Button("Open \(entry.title)") { model.select(key.reference) }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .clipShape(RoundedRectangle(cornerRadius: PaneGeometry.workspaceCornerRadius))
        .overlay {
            if context { RoundedRectangle(cornerRadius: PaneGeometry.workspaceCornerRadius).strokeBorder(active ? palette.borderActive : palette.border, lineWidth: 1).allowsHitTesting(false) }
        }
        .accessibilityIdentifier("work-workspace-\(key.id)")
    }

    private func newShell(in workspace: SessionsWorkspace) {
        do { try workspace.newShell() }
        catch { launchErrorTitle = "Could not open shell"; launchError = error.localizedDescription }
    }

    /// One menu changes what the multiplexer shows: a shell, the Task's files
    /// or its Flow process log.
    private func workspaceCreation(for workspace: SessionsWorkspace, task: RoadmapTask) -> some View {
        Menu {
            Button("New shell", systemImage: "terminal") { newShell(in: workspace) }
                .accessibilityIdentifier("work-add-shell")
            let multiplexer = workspace.multiplexer
            let location = workspace.location
            let execution = multiplexer.focusedPane.executionLocation
            Button("Files", systemImage: "doc") {
                workspace.selectFilesPage(taskId: task.id)
                multiplexer.show(.files(taskId: task.id, location: location, execution: execution))
            }
                .accessibilityIdentifier("work-add-files")
            Button("Flow processes", systemImage: "list.bullet.rectangle") { multiplexer.show(.flowLog(taskId: task.id, location: location, execution: execution)) }
                .accessibilityIdentifier("work-add-flow-log")
        } label: { Image(systemName: "plus") }
        .menuStyle(.borderlessButton).fixedSize()
        .help("Add a shell, files or the Flow process log")
        .accessibilityLabel("Add a shell, files or the Flow process log")
        .accessibilityIdentifier("work-create")
    }

    @ViewBuilder
    private var missingCheckout: some View {
        if let task = fileTask, task.task.reference.workspace?.localExists == false,
           navigation.preparedTaskWorktrees[task.task.id] == nil {
            HStack(spacing: Spacing.sm) {
                if let runtime = task.task.runtime, runtime.status.isTerminal {
                    Text("Recorded checkout is missing. This Task is \(runtime.status.label); it cannot be restored.")
                } else {
                    Text("Recorded checkout is missing")
                    Button("Restore checkout") { prepareTaskWorkspace() }
                }
                Spacer()
            }
            .font(Typography.meta).foregroundStyle(palette.textSecondary)
            .padding(.horizontal, 14).padding(.vertical, 6)
            .accessibilityIdentifier("work-missing-checkout")
        }
    }

    private func prepareTaskWorkspace() {
        guard let task = fileTask else { return }
        Task { @MainActor in
            do {
                let repo = store.repoPath
                let issue = task.task.task.identifier
                let path = try await Task.detached(priority: .userInitiated) {
                    try LocalWaveAgentLauncher.checkoutTask(repoPath: repo, issue: issue)
                }.value
                navigation.preparedTaskWorktrees[task.task.id] = path
            } catch {
                launchErrorTitle = "Could not prepare checkout"
                launchError = error.localizedDescription
            }
        }
    }

    private func openSession(_ record: SessionRecord) {
        model.openSession(record, store: store, besideFocused: NSEvent.modifierFlags.contains(.command))
    }

    private func openTask(_ work: WorkReference) {
        if NSEvent.modifierFlags.contains(.command) { model.select(work, besideFocused: true) }
        let trace = UUID().uuidString
        Perf.$inputTrace.withValue(trace) {
            Perf.agentInput("task_click", task: work.id)
            Task { await model.openTaskConversation(work.id) }
        }
    }

    private func openRepositoryConversation() {
        guard model.repoPath?.normalizedFilePath == store.repoPath.normalizedFilePath else { return }
        model.select(nil, besideFocused: NSEvent.modifierFlags.contains(.command))
        navigation.content = .terminals
        let entry = workspace
        Task { await entry.openPrimaryConversation(store: store, registry: workspaces) }
    }

    private func openWaveConversation(_ work: WorkReference) {
        guard let wave = model.wave(id: work.id)?.wave else { return }
        model.select(work, besideFocused: NSEvent.modifierFlags.contains(.command))
        navigation.content = .terminals
        let entry = workspace
        Task { await entry.openPrimaryConversation(wave: wave.name, store: store, registry: workspaces) }
    }

    private func startConversation() {
        guard let conversationScope = model.conversationScope else { return }
        do { try launch(conversationScope, in: workspace) }
        catch {
            launchErrorTitle = "Could not start conversation"
            launchError = error.localizedDescription
        }
    }

    /// Capture the Work entry before preparation; a late reply may populate
    /// that entry, but cannot select or redirect the workspace now on screen.
    private func launch(_ scope: ConversationScope, in workspace: SessionsWorkspace, identity requestedIdentity: WorkspaceIdentity? = nil) throws {
        let lf = try LocalWaveAgentLauncher.controlLfPath()
        guard let identity = requestedIdentity ?? workspace.location else {
            throw RegistryQueryError("Prepare this Work's location before opening a conversation.")
        }
        guard identity.machineId == machineId else { throw RegistryQueryError("Open a conversation on its owning Machine") }
        workspace.presentation.content = .terminals
        workspace.location = identity
        workspace.multiplexer.newShell(command: ConversationLaunch(scope: scope).arguments(lf: lf), location: identity)
    }

    /// An independent conversation with Task context in the Task's checkout.
    /// Resolving the checkout prepares Task Work only; it never starts a
    /// Flow or replaces another Session.
    private func newTaskSession(_ taskId: String) async throws {
        guard let found = model.task(id: taskId) else { return }
        let origin = navigation
        let key = key(for: .task(id: taskId))
        let entry = workspaces.workspace(for: key, title: found.task.task.name, location: found.task.reference.workspace?.identity)
        let issue = found.task.task.identifier
        let checkout: WorkspaceIdentity
        if let workspace = found.task.reference.workspace, workspace.localExists == true, let identity = workspace.identity {
            checkout = identity
        } else {
            let repo = store.repoPath
            checkout = try await Task.detached(priority: .userInitiated) {
                try LocalWaveAgentLauncher.checkoutTask(repoPath: repo, issue: issue)
            }.value
            origin.preparedTaskWorktrees[taskId] = checkout
        }
        try launch(.task(repo: checkout.worktree, id: issue), in: entry, identity: checkout)
    }

    /// Sessions attached to the focused pane: a Session pane's own record, or
    /// every conversation running inside a shell pane.
    private var focusedPaneSessions: [SessionItem] {
        let pane = multiplexer.focusedPane
        switch pane.content {
        case .shell:
            guard let terminal = pane.terminal else { return [] }
            return store.sessions.filter { store.localTerminal(for: $0.record) == terminal }
        case .session(let id, _, _):
            return store.sessions.filter { $0.id == id }
        case .empty, .flowLog, .files:
            return []
        }
    }

    private func split(_ axis: SplitAxis) {
        do { try workspace.split(multiplexer.focusedPaneId, axis: axis) }
        catch { launchErrorTitle = "Could not split pane"; launchError = error.localizedDescription }
    }

    private func _handle(_ shortcut: SessionsShortcut) {
        switch shortcut {
        case .newTerminal:
            newShell(in: workspace)
        case .splitRight:
            split(.vertical)
        case .splitDown:
            split(.horizontal)
        case .close:
            workspace.closePane(multiplexer.focusedPaneId)
        case .undoClose:
            multiplexer.undoClose()
        case .zoom:
            workspace.toggleFocus(multiplexer.focusedPaneId)
        case .focus(let direction):
            multiplexer.focus(direction)
        }
    }
}

private struct SidebarDivider: View {
    let navigation: WorkNavigation
    let availableWidth: CGFloat
    @Environment(\.palette) private var palette
    @State private var start: CGFloat?

    var body: some View {
        Rectangle().fill(palette.border).frame(width: 5)
            .contentShape(Rectangle())
            .onHover { if $0 { NSCursor.resizeLeftRight.push() } else { NSCursor.pop() } }
            .gesture(DragGesture(minimumDistance: 0).onChanged { value in
                if start == nil { start = navigation.sidebarWidth }
                navigation.resizeSidebar((start ?? navigation.sidebarWidth) + value.translation.width,
                                         availableWidth: availableWidth)
            }.onEnded { _ in start = nil })
            .accessibilityLabel("Sidebar width")
            .accessibilityValue("\(Int(navigation.sidebarWidth)) points")
            .accessibilityAdjustableAction { direction in
                navigation.resizeSidebar(navigation.sidebarWidth + (direction == .increment ? 20 : -20),
                                         availableWidth: availableWidth)
            }
            .accessibilityIdentifier("work-sidebar-divider")
    }
}

private struct WorkLayoutView<Content: View>: View {
    let layout: WorkLayout
    let registry: SessionsWorkspaceRegistry
    @ViewBuilder let content: (WorkWorkspaceKey) -> Content

    var body: some View {
        switch layout {
        case .workspace(let key): content(key)
        case .split(let axis, let first, let second, let ratio):
            GeometryReader { geometry in
                // Reflow whole workspaces vertically when side-by-side panes would be too narrow.
                let vertical = axis == .vertical && geometry.size.width >= 720
                let extent = vertical ? geometry.size.width : geometry.size.height
                let effectiveAxis: SplitAxis = vertical ? .vertical : .horizontal
                let sizing = SplitSizing(extent: extent, firstMinimum: vertical ? 360 : 200,
                                         secondMinimum: vertical ? 360 : 200)
                let displayedRatio = sizing.constrain(ratio)
                let stack = vertical ? AnyLayout(HStackLayout(spacing: 0)) : AnyLayout(VStackLayout(spacing: 0))
                stack {
                    WorkLayoutView(layout: first, registry: registry, content: content)
                        .frame(width: vertical ? max(0, extent * displayedRatio - sizing.divider / 2) : nil,
                               height: vertical ? nil : max(0, extent * displayedRatio - sizing.divider / 2))
                    SplitDivider(axis: effectiveAxis, ratio: displayedRatio, sizing: sizing, update: { value in
                        if let firstKey = first.keys.first, let secondKey = second.keys.first {
                            registry.resize(between: firstKey, and: secondKey, ratio: value)
                        }
                    })
                    WorkLayoutView(layout: second, registry: registry, content: content)
                }
            }
        }
    }
}

private enum SessionsShortcut {
    case newTerminal
    case splitRight
    case splitDown
    case close
    case undoClose
    case zoom
    case focus(SpatialDirection)
}

private struct SessionsShortcutMonitor: NSViewRepresentable {
    let handler: (SessionsShortcut) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(handler: handler)
    }

    func makeNSView(context: Context) -> NSView {
        let view = NSView(frame: .zero)
        context.coordinator.install(for: view)
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        context.coordinator.view = nsView
    }

    static func dismantleNSView(_ nsView: NSView, coordinator: Coordinator) {
        coordinator.uninstall()
    }

    @MainActor
    final class Coordinator {
        weak var view: NSView?
        private let handler: (SessionsShortcut) -> Void
        private var monitor: Any?

        init(handler: @escaping (SessionsShortcut) -> Void) {
            self.handler = handler
        }

        func install(for view: NSView) {
            self.view = view
            monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
                guard let self,
                      let window = self.view?.window,
                      event.window === window,
                      let shortcut = Self._shortcut(for: event)
                else { return event }
                // A focused text view owns ⌘Z. Every other shortcut stays here, so ⌘W
                // closes a pane instead of falling through to close the window.
                if case .undoClose = shortcut, window.firstResponder is NSTextView { return event }
                self.handler(shortcut)
                return nil
            }
        }

        func uninstall() {
            if let monitor {
                NSEvent.removeMonitor(monitor)
                self.monitor = nil
            }
        }

        private static func _shortcut(for event: NSEvent) -> SessionsShortcut? {
            let modifiers = event.modifierFlags.intersection([
                .command, .shift, .option, .control,
            ])
            // Shift is the one modifier `charactersIgnoringModifiers` keeps, so
            // ⌘⇧D arrives as "D".
            let key = event.charactersIgnoringModifiers?.lowercased()
            if modifiers == [.command], key == "t" {
                return .newTerminal
            }
            if modifiers == [.command], key == "d" {
                return .splitRight
            }
            if modifiers == [.command, .shift], key == "d" {
                return .splitDown
            }
            if modifiers == [.command], event.charactersIgnoringModifiers == "w" {
                return .close
            }
            if modifiers == [.command], event.charactersIgnoringModifiers == "z" {
                return .undoClose
            }
            if modifiers == [.command, .shift], event.keyCode == 36 || event.keyCode == 76 {
                return .zoom
            }
            guard modifiers == [.command, .option] else { return nil }
            return switch event.specialKey {
            case .leftArrow: .focus(.left)
            case .rightArrow: .focus(.right)
            case .upArrow: .focus(.up)
            case .downArrow: .focus(.down)
            default: nil
            }
        }
    }
}

#endif
