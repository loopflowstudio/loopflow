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
    let facts: WorkMembershipPlan.Facts

    var state: String {
        guard facts.lifecycle.known else { return "Updating…" }
        let states = Set(facts.lifecycle.states)
        if states == task.selectionStates.subtracting([.available, .busy, .blocked]) { return TaskHeading(task: task).state }
        switch TaskSection(states: facts.lifecycle.states) {
        case .done: return "Completed"
        case .abandoned: return "Canceled"
        case .backlog: return "Not started"
        case .working: return "Working"
        case .autonomous: return "Shipping"
        }
    }

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
    let unplacedTasks: [TaskProjection]
    private let taskParents: [String: WorkMembershipPlan.ParentFact]

    init(roadmaps: [WaveRoadmap], inventory: [RoadmapTask], sessions: [SessionRecord], facts: [WorkMembershipPlan.Facts]) {
        let fields = Dictionary(facts.filter { $0.work.kind == .task }.map { ($0.work.id, $0) }, uniquingKeysWith: { first, _ in first })
        taskParents = fields.mapValues(\.parent)
        let content = Dictionary(inventory.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
        let inventory = facts.filter { $0.work.kind == .task }.compactMap { content[$0.work.id] }
        var matched = Set<String>()
        let visibleTaskIds = Set(inventory.map(\.id))
        var taskSessions: [String: [SessionRecord]] = [:]
        var waveSessions: [String: [SessionRecord]] = [:]
        // Index the shared memberships once. A Session may belong to more than
        // one Task; workspace and explicit membership must not duplicate a row.
        for session in sessions {
            let visibleMemberships = Set(session.taskIds).intersection(visibleTaskIds)
            for taskId in visibleMemberships {
                taskSessions[taskId, default: []].append(session)
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
                tasks: inventory.filter { fields[$0.id]?.parent.wave == wave.wave.id }.map { task in
                    let records = taskSessions[task.id] ?? []
                    matched.formUnion(records.map(\.id))
                    return TaskProjection(
                        id: WorkNodeKey(repo: repo, work: .task(id: task.id)),
                        task: task, sessions: records, facts: fields[task.id]!
                    )
                }
            )
        }
        standaloneTasks = inventory.filter { fields[$0.id]?.parent.wave == nil }.map { task in
            let records = taskSessions[task.id] ?? []
            matched.formUnion(records.map(\.id))
            return TaskProjection(id: WorkNodeKey(repo: task.task.repo, work: .task(id: task.id)), task: task, sessions: records, facts: fields[task.id]!)
        }
        let waveIDs = Set(roadmaps.map { $0.wave.id })
        unplacedTasks = inventory.filter { fields[$0.id]?.parent.wave.map { !waveIDs.contains($0) } == true }.map { task in
            let records = taskSessions[task.id] ?? []
            matched.formUnion(records.map(\.id))
            return TaskProjection(id: WorkNodeKey(repo: task.task.repo, work: .task(id: task.id)),
                task: task, sessions: records, facts: fields[task.id]!)
        }
        // Missing planning and unattributed Sessions remain reachable, including
        // mandatory human boundaries and multiple conversations on one subject.
        unmatchedSessions = sessions.filter { !matched.contains($0.id) }
    }

    /// The drill-down trail for the selected Work or Session. A Session's
    /// siblings share its parent subject; the final crumb chooses among them.
    func breadcrumb(selection: WorkReference?, sessionId: String?) -> WorkBreadcrumb? {
        if let sessionId {
            if let task = (standaloneTasks + unplacedTasks).first(where: { $0.sessions.contains { $0.id == sessionId } }),
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
                let parent = session.taskIds.first.flatMap { taskParents[$0] }
                let waveId = session.taskIds.isEmpty
                    ? session.waveId ?? (session.work?.kind == .wave ? session.work?.id : nil) : parent?.wave
                let wave = waves.first { $0.roadmap.wave.id == waveId }
                let siblings = session.work.map { work in
                    unmatchedSessions.filter { $0.work == work }
                } ?? [session]
                return WorkBreadcrumb(wave: wave, task: nil, session: session, siblings: siblings, taskParent: parent)
            }
        }
        guard let selection else { return nil }
        if let task = (standaloneTasks + unplacedTasks).first(where: {
            $0.id.work == selection
        }) {
            return WorkBreadcrumb(wave: nil, task: task, session: nil, siblings: task.sessions)
        }
        for wave in waves {
            if wave.id.work == selection {
                return WorkBreadcrumb(wave: wave, task: nil, session: nil, siblings: wave.sessions)
            }
            if let task = wave.tasks.first(where: {
                $0.id.work == selection
            }) {
                return WorkBreadcrumb(wave: wave, task: task, session: nil, siblings: task.sessions)
            }
        }
        return nil
    }

    func subject(for sessionId: String) -> WorkReference? {
        if let task = (standaloneTasks + unplacedTasks).first(where: { $0.sessions.contains { $0.id == sessionId } }) { return task.id.work }
        for wave in waves {
            if wave.sessions.contains(where: { $0.id == sessionId }) { return wave.id.work }
            if let task = wave.tasks.first(where: { $0.sessions.contains { $0.id == sessionId } }) {
                return task.id.work
            }
        }
        guard let session = unmatchedSessions.first(where: { $0.id == sessionId }) else { return nil }
        if session.primaryScope == "repository" { return nil }
        if session.primaryScope == "wave" { return session.waveId.map { .wave(id: $0) } }
        return session.taskIds.first.map { .task(id: $0) } ?? session.work
    }
}

/// Wave → Task → Session. Missing ancestry stays missing rather than guessed.
struct WorkBreadcrumb {
    let wave: WaveProjection?
    let task: TaskProjection?
    let session: SessionRecord?
    let siblings: [SessionRecord]
    var taskParent: WorkMembershipPlan.ParentFact? = nil

    var waveWork: WorkReference? {
        if let task { return task.facts.parent.wave.map { .wave(id: $0) } }
        if let taskParent { return taskParent.wave.map { .wave(id: $0) } }
        if session?.taskIds.isEmpty == false { return nil }
        if let id = session?.waveId { return .wave(id: id) }
        if let work = session?.work, work.kind == .wave { return work }
        return wave?.id.work
    }

    var taskWork: WorkReference? {
        if let task { return task.id.work }
        if let id = session?.taskIds.first { return .task(id: id) }
        return nil
    }
}

/// One exact Session's assignment confirmation, retained through a failed commit.
struct SessionBindingDraft: Equatable {
    let id = UUID()
    let sessionId: String
    let title: String
    var assignedTaskId: String?
    var selector = ""
    var taskTitle = ""
    var preview: SessionBindingPreview?
    var submitting = false
    var error: String?
}

/// Local name editor; saving moves the request into WorkInteraction.
struct SessionRenameDraft: Equatable {
    let id = UUID()
    let sessionId: String
    var text: String
}

/// One Task's in-flight execution command and its refusal.
struct TaskRunDraft: Equatable {
    enum Phase { case runningCommand, awaitingObservation }
    var phase: Phase?
    var acting: Bool { phase != nil }
    var progress: String {
        phase == .awaitingObservation ? "Task updated; refreshing…" : "Updating Task…"
    }
    var error: String?
}

/// A Wave view's named parts, as `lf desktop -p` names them.
enum WaveViewPart: String, CaseIterable, Hashable {
    case board, session, tiles

    /// The Tasks strip distracts more than it helps for now (Jack Heart, LOO-673);
    /// Show… or `lf desktop show -p tiles` brings it back.
    static let hiddenByDefault: Set<Self> = [.tiles]

    /// How the view's `Show…` menu names it.
    var title: String {
        switch self {
        case .board: "Board"
        case .session: "Session"
        case .tiles: "Tasks"
        }
    }
}

/// The section a Wave board shows, one at a time; Backlog by default.
enum WaveBoardSection: String, CaseIterable, Hashable {
    case backlog, working, autonomous, done

    var title: String { rawValue.capitalized }
}

/// A Wave view's dividers: the conversation's share of the top band's width, and
/// the top band's share of the view's height, golden by default.
struct WaveViewRatios: Equatable {
    var session = 1 - FocusSizing.golden
    var band = FocusSizing.golden
}

/// Per-window/repository navigation; independent of terminal layout and liveness.
@MainActor
@Observable
final class WorkNavigation {
    struct TaskCreationDraft {
        let request: String
        let wave: String?
        let graph: String?
    }
    var newTaskDraft: TaskCreationDraft?
    var startingTask: PendingOperation?

    enum SidebarFilter: String, CaseIterable {
        case working = "Working tasks"
        case available = "Available"
        case busy = "Busy"
    }
    /// The Wave the sidebar lists; nil lists Working Tasks. Selecting a Task keeps it.
    var sidebarWaveID: String?
    /// What the sidebar lists: a saved Dashboard while it is shown or zoomed from,
    /// else the stored Wave, else the repository's Working Tasks.
    enum SidebarScope: Hashable {
        case repository
        case wave(String)
        case dashboard(String)
    }
    var sidebarScope: SidebarScope {
        if case .saved(let id) = trailBoard { return .dashboard(id) }
        return sidebarWaveID.map(SidebarScope.wave) ?? .repository
    }
    /// Each board's page; it outlives entering a tile and coming back. Nil is the unsaved view.
    var boardPages: [Board?: Int] = [:]
    /// Each Wave view's hidden board and strip; its conversation hides as its session pane.
    var hiddenViewParts: [Board: Set<WaveViewPart>] = [:]
    /// Each Wave view's zoomed board or strip; its conversation zooms as its session pane.
    var zoomedViewParts: [Board: WaveViewPart] = [:]
    var waveRatios: [Board: WaveViewRatios] = [:]
    var boardSections: [Board: WaveBoardSection] = [:]
    var showsSlotHints = false
    var sidebarFilter: SidebarFilter = .working {
        willSet {
            if newValue != sidebarFilter { Perf.begin(Perf.hierarchyInteraction, "filter", id: "outline") }
        }
    }
    var sidebarFocusRequested = false
    enum Content { case overview, details, terminals }
    enum Palette: Equatable {
        case search
        case sidebarSearch
        case graph(String)
        case workflow(String)
        case newTask
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
            if oldValue != palette {
                if palette != nil { startingTask = nil }
            }
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
            if let renaming, renaming.sessionId != selectedSessionId {
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
    /// What the last Auto-Ship or Stop Auto-Ship on the window heading did.
    var shipNotice: ShipNotice?
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
    struct QueryDestination: Equatable {
        let machine: String?
        let repository: String
        let expression: String
    }
    enum Destination: Equatable {
        case work(WorkReference?)
        case dashboard(String)
        case view(QueryDestination)
        /// The default Dashboard: the Working Tasks plan as a grid of slots.
        case workingTasks
        /// One Dashboard member shown as full Work; the trail returns through its Dashboard.
        case entered(from: Board, work: WorkReference)
    }
    /// A view a tile can be entered from: a saved Dashboard, Working Tasks or a Wave
    /// view, of a Wave or of the repository (its default Wave, over every Task).
    enum Board: Hashable {
        case saved(String)
        case workingTasks
        case wave(String)
        case repository
    }
    static let workingTasksTitle = "Working Tasks"
    var destination: Destination = .work(nil)
    var selection: WorkReference? {
        get {
            switch destination {
            case .work(let work): work
            case .entered(_, let work): work
            case .dashboard, .view, .workingTasks: nil
            }
        }
        set { destination = .work(newValue) }
    }
    var showsSelection: Bool {
        switch destination {
        case .work, .entered: false
        case .dashboard, .view, .workingTasks: true
        }
    }
    var dashboardID: String? {
        if case .dashboard(let id) = destination { return id }; return nil
    }
    /// The view the breadcrumb passes through, shown or entered from.
    var trailBoard: Board? {
        switch destination {
        case .dashboard(let id): .saved(id)
        case .workingTasks: .workingTasks
        case .entered(let board, _): board
        case .work: waveView
        case .view: nil
        }
    }

    /// The Wave view on screen: a Wave's, or the repository page, which is the same view.
    var waveView: Board? {
        switch destination {
        case .work(nil): .repository
        case .work(let work?) where work.kind == .wave: .wave(work.id)
        case .work, .entered, .dashboard, .view, .workingTasks: nil
        }
    }
    /// Where the entered member's tile sat, in `ZoomFromTile.space`; read once
    /// by the transition that grows the full Work out of it.
    @ObservationIgnored var zoomOrigin: CGRect?
    /// Repository-local visits; nil names the repository conversation.
    var backStack: [Destination] = []
    var forwardStack: [Destination] = []
    var selectedTaskEvidence: (wave: WaveRoadmap?, task: RoadmapTask)?
    var showsNavigator = true

    func toggleNavigator() {
        palette = nil
        showsNavigator.toggle()
    }
    var sidebarWidth: CGFloat = 240
    /// The pinned Waves section's height; it starts at about three rows.
    var sidebarWavesHeight: CGFloat = 130
    /// Restores the sidebar when it is rebuilt. Written every scrolled frame, so
    /// unobserved: tracking it re-rendered every sidebar row per frame.
    @ObservationIgnored var sidebarScrollOffset: CGFloat = 0

    func resizeSidebar(_ width: CGFloat, availableWidth: CGFloat) {
        sidebarWidth = min(600, max(240, min(width, availableWidth - 360)))
    }

    /// Resolve an initially unknown local endpoint without recording a new visit.
    func bindQueryDestinations(to machine: String) {
        func bind(_ destination: Destination) -> Destination {
            guard case .view(let query) = destination, query.machine == nil else { return destination }
            return .view(QueryDestination(machine: machine, repository: query.repository, expression: query.expression))
        }
        destination = bind(destination)
        backStack = backStack.map(bind)
        forwardStack = forwardStack.map(bind)
    }

    /// Selections and visits from another Machine name no Work on this one.
    func clearSelection() {
        selection = nil
        backStack = []
        forwardStack = []
        selectedTaskEvidence = nil
        sidebarWaveID = nil
        content = .overview
    }

}
