import Foundation
import Loopflow
import Observation

/// Planning identity stays stable when durable Task Work or a Session appears.
struct WorkNodeKey: Hashable {
    let repo: String
    let work: WorkReference
}

struct TaskProjection: Identifiable {
    let id: WorkNodeKey
    let task: RoadmapTask
    let sessions: [SessionRecord]
}

struct WaveProjection: Identifiable {
    let id: WorkNodeKey
    let roadmap: WaveRoadmap
    let sessions: [SessionRecord]
    let tasks: [TaskProjection]
}

/// A presentation of the two shared readings, never another inventory.
struct WorkProjection {
    let waves: [WaveProjection]
    let standaloneTasks: [TaskProjection]
    let unmatchedSessions: [SessionRecord]

    init(roadmaps: [WaveRoadmap], inventory: [RoadmapTask], sessions: [SessionRecord]) {
        var matched = Set<String>()
        let visibleTaskIds = Set(inventory.flatMap { [$0.id, $0.runtime?.workId].compactMap { $0 } })
        var taskSessions: [String: [SessionRecord]] = [:]
        var waveSessions: [String: [SessionRecord]] = [:]
        // Index the shared memberships once. A Session may belong to more than
        // one Task; workspace and explicit membership must not duplicate a row.
        for session in sessions {
            var taskIds = Set(session.taskIds)
            if let taskId = session.workspace?.taskId { taskIds.insert(taskId) }
            let visibleMemberships = taskIds.intersection(visibleTaskIds)
            if session.primaryScope == nil {
                for taskId in visibleMemberships {
                    taskSessions[taskId, default: []].append(session)
                }
            }
            if visibleMemberships.isEmpty {
                let waveId = session.work?.kind == .wave ? session.work?.id
                    : (session.work?.kind == .project ? session.waveId : nil)
                if let waveId { waveSessions[waveId, default: []].append(session) }
            }
        }
        waves = roadmaps.map { wave in
            let repo = wave.wave.repo
            let records = waveSessions[wave.wave.id] ?? []
            matched.formUnion(records.map(\.id))
            return WaveProjection(
                id: WorkNodeKey(repo: repo, work: .wave(id: wave.wave.id)),
                roadmap: wave,
                sessions: records,
                tasks: wave.tasks.items.sorted { $0.task.rank < $1.task.rank }.map { task in
                    let records = task.runtime.flatMap { taskSessions[$0.workId] } ?? []
                    matched.formUnion(records.map(\.id))
                    return TaskProjection(
                        id: WorkNodeKey(repo: repo, work: .task(id: task.id)),
                        task: task, sessions: records
                    )
                }
            )
        }
        standaloneTasks = inventory.filter { $0.task.waveId == nil }.sorted { $0.task.rank < $1.task.rank }.map { task in
            let records = taskSessions[task.runtime?.workId ?? task.id] ?? []
            matched.formUnion(records.map(\.id))
            return TaskProjection(id: WorkNodeKey(repo: task.task.repo, work: .task(id: task.id)), task: task, sessions: records)
        }
        // Missing planning and unattributed Sessions remain reachable, including
        // mandatory human boundaries and multiple conversations on one subject.
        unmatchedSessions = sessions.filter { !matched.contains($0.id) }
    }

    /// The drill-down trail for the selected Work or Session. A Session's
    /// siblings share its parent subject; the final crumb chooses among them.
    func breadcrumb(selection: WorkReference?, sessionId: String?) -> WorkBreadcrumb? {
        if let sessionId {
            if let task = standaloneTasks.first(where: { $0.sessions.contains { $0.id == sessionId } }),
               let session = task.sessions.first(where: { $0.id == sessionId }) {
                return WorkBreadcrumb(wave: nil, task: task, session: session, siblings: task.sessions)
            }
            for wave in waves {
                if let session = wave.sessions.first(where: { $0.id == sessionId }) {
                    return WorkBreadcrumb(wave: wave, task: nil, session: session, siblings: wave.sessions)
                }
                if let task = wave.tasks.first(where: { $0.sessions.contains { $0.id == sessionId } }),
                   let session = task.sessions.first(where: { $0.id == sessionId }) {
                    return WorkBreadcrumb(wave: wave, task: task, session: session, siblings: task.sessions)
                }
            }
            if let session = unmatchedSessions.first(where: { $0.id == sessionId }) {
                let waveId = session.waveId ?? (session.work?.kind == .wave ? session.work?.id : nil)
                let wave = waves.first { $0.roadmap.wave.id == waveId }
                let siblings = session.work.map { work in
                    unmatchedSessions.filter { $0.work == work }
                } ?? [session]
                return WorkBreadcrumb(wave: wave, task: nil, session: session, siblings: siblings)
            }
        }
        guard let selection else { return nil }
        if let task = standaloneTasks.first(where: {
            $0.id.work == selection || $0.task.runtime.map { WorkReference.task(id: $0.workId) } == selection
        }) {
            return WorkBreadcrumb(wave: nil, task: task, session: nil, siblings: task.sessions)
        }
        for wave in waves {
            if wave.id.work == selection {
                return WorkBreadcrumb(wave: wave, task: nil, session: nil, siblings: wave.sessions)
            }
            if let task = wave.tasks.first(where: {
                $0.id.work == selection || $0.task.runtime.map { WorkReference.task(id: $0.workId) } == selection
            }) {
                return WorkBreadcrumb(wave: wave, task: task, session: nil, siblings: task.sessions)
            }
        }
        return nil
    }

    func subject(for sessionId: String) -> WorkReference? {
        if let task = standaloneTasks.first(where: { $0.sessions.contains { $0.id == sessionId } }) { return task.id.work }
        for wave in waves {
            if wave.sessions.contains(where: { $0.id == sessionId }) { return wave.id.work }
            if let task = wave.tasks.first(where: { $0.sessions.contains { $0.id == sessionId } }) {
                return task.id.work
            }
        }
        guard let session = unmatchedSessions.first(where: { $0.id == sessionId }) else { return nil }
        if session.primaryScope == "repository" { return nil }
        if session.primaryScope == "wave" { return session.waveId.map { .wave(id: $0) } }
        return session.workspace?.taskId.map { .task(id: $0) } ?? session.work
    }
}

/// Wave → Task → Session. Missing ancestry stays missing rather than guessed.
struct WorkBreadcrumb {
    let wave: WaveProjection?
    let task: TaskProjection?
    let session: SessionRecord?
    let siblings: [SessionRecord]

    var waveWork: WorkReference? {
        if let id = session?.waveId { return .wave(id: id) }
        if let work = session?.work, work.kind == .wave { return work }
        return wave?.id.work
    }

    var taskWork: WorkReference? {
        guard session?.primaryScope == nil else { return nil }
        if let task { return task.id.work }
        if let id = session?.workspace?.taskId { return .task(id: id) }
        if let work = session?.work, work.kind == .task { return work }
        return nil
    }
}

/// One exact Session's assignment confirmation, retained through a failed commit.
struct SessionBindingDraft: Equatable {
    let id = UUID()
    let sessionId: String
    let title: String
    var selector = ""
    var preview: SessionBindingPreview?
    var submitting = false
    var error: String?
}

/// An in-place Session name edit. It belongs to exactly one Session; a
/// completion for another Session can never touch it.
struct SessionRenameDraft: Equatable {
    let id = UUID()
    let sessionId: String
    var text: String
    var error: String?
    var submitting = false
}

/// One Task's in-flight `lf task run` or `lf task move` and its refusal.
struct TaskRunDraft: Equatable {
    enum Phase { case runningCommand, awaitingObservation }
    var phase: Phase?
    var acting: Bool { phase != nil }
    var progress: String {
        phase == .awaitingObservation ? "Task updated; refreshing…" : "Updating Task…"
    }
    var error: String?
}

/// Per-window/repository navigation; independent of terminal layout and liveness.
@MainActor
@Observable
final class WorkNavigation {
    enum SidebarFilter: String, CaseIterable {
        case working = "Working tasks"
        case available = "Available"
        case running = "Running"
    }
    /// Presentation order, independent of query arrivals, attention and recency.
    var workingTaskIDs: [String] = []
    var showsSlotHints = false
    var sidebarFilter: SidebarFilter = .working
    var sidebarFocusRequested = false

    func retainWorkingTask(_ id: String, aliases: [String] = []) {
        var seen = Set<String>()
        workingTaskIDs = (workingTaskIDs.map { aliases.contains($0) ? id : $0 } + [id])
            .filter { seen.insert($0).inserted }
    }
    enum Content { case overview, details, terminals }
    enum Palette: Equatable {
        case search
        case sidebarSearch
        case graph(String)
        case workflow(String)
    }
    @MainActor @Observable
    final class WorkspacePresentation {
        var content: Content = .details
        var selectedSessionId: String?
    }
    var workspacePresentation = WorkspacePresentation()
    var content: Content {
        get { workspacePresentation.content }
        set { workspacePresentation.content = newValue }
    }
    var palette: Palette? {
        didSet {
            if oldValue == nil, palette != nil { restorePaletteFocus = true }
        }
    }
    var restorePaletteFocus = true
    var recentDestinations: [WorkPaletteRow] = []
    /// Source revisions isolate disclosure from changed definitions and repositories.
    var expandedCompositionGroups: [String: Set<String>] = [:]
    var showsActivity = false
    var selectedSessionId: String? {
        get { workspacePresentation.selectedSessionId }
        set {
            workspacePresentation.selectedSessionId = newValue
            // Leaving a Session abandons an unsubmitted edit; a submitted one
            // still settles against its own Session.
            if let renaming, renaming.sessionId != selectedSessionId, !renaming.submitting {
                self.renaming = nil
            }
            if let binding, binding.sessionId != selectedSessionId, !binding.submitting {
                self.binding = nil
            }
        }
    }
    var renaming: SessionRenameDraft?
    var binding: SessionBindingDraft?
    /// Graph drafts by planning Task id; they survive Task and Session navigation.
    var taskRunDrafts: [String: TaskRunDraft] = [:]
    var startingTaskSessions: Set<String> = []
    var taskSessionErrors: [String: String] = [:]
    /// Successful `task prepare` receipts, available before the next roadmap read.
    var preparedTaskWorktrees: [String: WorkspaceIdentity] = [:]
    /// Tasks whose Comments are expanded; a presentation fact, not a reading.
    var expandedComments: Set<String> = []
    /// Tasks whose Session history is disclosed; history is read only then.
    var expandedHistory: Set<String> = []
    /// Wave planning notices whose Details are open, keyed by Task ID.
    var expandedNotices: Set<String> = []
    /// Graph processes whose graph and steps are open, keyed by Graph process.
    var expandedGraphProcesses: Set<String> = []
    var selection: WorkReference?
    /// Repository-local visits; nil names the repository conversation.
    var backStack: [WorkReference?] = []
    var forwardStack: [WorkReference?] = []
    var selectedTaskEvidence: (wave: WaveRoadmap?, task: RoadmapTask)?
    var showsNavigator = true

    func toggleNavigator() {
        palette = nil
        showsNavigator.toggle()
    }
    var sidebarWidth: CGFloat = 240
    var sidebarScrollOffset: CGFloat = 0

    func resizeSidebar(_ width: CGFloat, availableWidth: CGFloat) {
        sidebarWidth = min(600, max(240, min(width, availableWidth - 360)))
    }

    /// Selections and visits from another Machine name no Work on this one.
    func clearSelection() {
        selection = nil
        backStack = []
        forwardStack = []
        selectedTaskEvidence = nil
        workingTaskIDs = []
        content = .overview
    }

}
