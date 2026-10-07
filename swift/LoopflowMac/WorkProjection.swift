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

    /// Shared durable evidence that work began. Inspection, a prepared
    /// checkout, or a live process in the checkout never supplies it.
    var started: Bool { task.runtime?.started == true }

    /// The started working set. A Task with open Sessions stays reachable even
    /// when its start predates recorded evidence. Locally done or abandoned
    /// Work has left it even while planning has not caught up.
    var inWorkingSet: Bool {
        if !sessions.isEmpty { return true }
        return started && task.condition.unresolvedExecution
    }
}

enum WorkPresentation: String, CaseIterable {
    case compact = "Compact"
    case full = "Full hierarchy"
    case sessions = "Sessions"
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
    let unmatchedSessions: [SessionRecord]

    init(roadmaps: [WaveRoadmap], sessions: [SessionRecord]) {
        var matched = Set<String>()
        let visibleTaskIds = Set(roadmaps.flatMap { $0.tasks.items.compactMap { $0.runtime?.workId } })
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
        // Missing planning and unattributed Sessions remain reachable, including
        // mandatory human boundaries and multiple conversations on one subject.
        unmatchedSessions = sessions.filter { !matched.contains($0.id) }
    }

    /// The drill-down trail for the selected Work or Session. A Session's
    /// siblings share its parent subject; the final crumb chooses among them.
    func breadcrumb(selection: WorkReference?, sessionId: String?) -> WorkBreadcrumb? {
        if let sessionId {
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

struct WorkOutlineSubject {
    let key: WorkNodeKey
    let title: String
}

/// Disposable visible rows. Planning and Session identities remain the source.
struct WorkOutlineRow: Identifiable {
    enum Content {
        case work(WorkOutlineSubject, hasChildren: Bool, sessions: [SessionRecord])
        case session(SessionRecord)
    }

    enum ID: Hashable {
        case work(WorkNodeKey)
        case session(String)
    }

    let content: Content
    var detail: String?
    var depth: Int
    let ancestors: [WorkOutlineSubject]

    var id: ID {
        switch content {
        case .work(let subject, _, _): .work(subject.key)
        case .session(let session): .session(session.id)
        }
    }

    var title: String {
        switch content {
        case .work(let subject, _, _): subject.title
        case .session(let session): session.title
        }
    }

    var session: SessionRecord? {
        guard case .session(let session) = content else { return nil }
        return session
    }

    var workKey: WorkNodeKey? {
        guard case .work(let subject, _, _) = content else { return nil }
        return subject.key
    }

    /// Open Sessions presented inline on a Task row rather than as leaves.
    var inlineSessions: [SessionRecord] {
        guard case .work(_, _, let sessions) = content else { return [] }
        return sessions
    }
}

extension WorkProjection {
    func outline(
        presentation: WorkPresentation, collapsed: Set<WorkNodeKey>,
        search: String, planningReadable: Bool
    ) -> [WorkOutlineRow] {
        let query = search.trimmingCharacters(in: .whitespacesAndNewlines)
        func matches(_ text: String) -> Bool {
            query.isEmpty || text.localizedCaseInsensitiveContains(query)
        }
        func expanded(_ key: WorkNodeKey) -> Bool {
            !query.isEmpty || !collapsed.contains(key)
        }
        var rows: [WorkOutlineRow] = []
        func appendSession(_ session: SessionRecord, depth: Int, ancestors: [WorkOutlineSubject], include: Bool = false) {
            guard include || matches(session.title) || matches(session.detail)
                || ancestors.contains(where: { matches($0.title) }) else { return }
            rows.append(WorkOutlineRow(content: .session(session), detail: nil, depth: depth, ancestors: ancestors))
        }
        func appendWork(_ subject: WorkOutlineSubject, detail: String? = nil, depth: Int,
                        ancestors: [WorkOutlineSubject], hasChildren: Bool,
                        sessions: [SessionRecord] = []) {
            rows.append(WorkOutlineRow(
                content: .work(subject, hasChildren: hasChildren, sessions: sessions),
                detail: detail, depth: depth,
                ancestors: ancestors
            ))
        }
        for wave in waves {
            let waveSubject = WorkOutlineSubject(key: wave.id, title: wave.roadmap.wave.displayName)
            let complete: Bool
            if case .available(_, false) = wave.roadmap.tasks {
                complete = planningReadable && wave.roadmap.unavailableTasks.isEmpty
            } else { complete = false }
            let flat = presentation == .sessions
            let workingSet = wave.tasks.filter(\.inWorkingSet)
            let waveHasChildren = !workingSet.isEmpty || !wave.sessions.isEmpty
            let omitWave = flat || (presentation == .compact && waves.count == 1 && complete
                && waveHasChildren && expanded(wave.id))
            let waveStart = rows.count
            if !omitWave { appendWork(waveSubject, depth: 0, ancestors: [], hasChildren: waveHasChildren,
                                      sessions: wave.tasks.flatMap(\.sessions)) }
            if flat || expanded(wave.id) {
                let waveDepth = omitWave ? 0 : 1
                for session in wave.sessions { appendSession(session, depth: waveDepth, ancestors: [waveSubject]) }
                let ancestors = [waveSubject]
                for task in workingSet {
                    let taskSubject = WorkOutlineSubject(key: task.id, title: task.task.task.name)
                    let taskMatches = matches(taskSubject.title) || matches(task.task.task.identifier)
                        || matches(waveSubject.title)
                        || task.sessions.contains(where: { matches($0.title) || matches($0.detail) })
                    guard taskMatches else { continue }
                    if flat {
                        for session in task.sessions {
                            appendSession(session, depth: 0, ancestors: ancestors + [taskSubject],
                                          include: matches(task.task.task.identifier))
                        }
                    } else {
                        // Open Sessions ride the Task row as a count and names.
                        appendWork(taskSubject, depth: waveDepth, ancestors: ancestors,
                                   hasChildren: false, sessions: task.sessions)
                    }
                }
            }
            if !query.isEmpty, !omitWave, rows.count == waveStart + 1, !matches(waveSubject.title) { rows.removeLast() }
        }
        // Known Work stays visible when its planning row is unavailable.
        // Only genuinely unbound Sessions belong in the orphan section.
        for session in unmatchedSessions where session.work != nil || session.waveId != nil || session.primaryScope != nil || session.workspace?.taskId != nil || !session.taskIds.isEmpty {
            let waveId = session.waveId ?? (session.work?.kind == .wave ? session.work?.id : nil)
            let wave = waves.first { $0.roadmap.wave.id == waveId }
            var ancestors: [WorkOutlineSubject] = []
            if let waveId {
                ancestors.append(WorkOutlineSubject(
                    key: WorkNodeKey(repo: wave?.id.repo ?? "", work: .wave(id: waveId)),
                    title: wave?.roadmap.wave.displayName ?? "Wave \(waveId)"
                ))
            }
            if session.primaryScope == nil, let work = session.work, work.kind != .wave {
                ancestors.append(WorkOutlineSubject(
                    key: WorkNodeKey(repo: wave?.id.repo ?? "", work: work),
                    title: "\(work.kind == .task ? "Task" : "Project") \(work.id)"
                ))
            }
            appendSession(session, depth: 0, ancestors: ancestors)
        }
        if presentation == .sessions {
            // The shortest ancestry suffix that distinguishes equal titles. IDs
            // remain the tie-breaker for two conversations on the same subject.
            let all = rows
            rows = all.map { row in
                var row = row
                let peers = all.filter { $0.title == row.title }
                if peers.count > 1 {
                    for count in 1...max(1, row.ancestors.count) {
                        let suffix = row.ancestors.suffix(count).map(\.title).joined(separator: " / ")
                        if !suffix.isEmpty { row.detail = suffix }
                        if peers.filter({ $0.ancestors.suffix(count).map(\.title).joined(separator: " / ") == suffix }).count == 1 { break }
                    }
                    if peers.filter({ $0.ancestors.map(\.title) == row.ancestors.map(\.title) }).count > 1,
                       let id = row.session?.id { row.detail = [row.detail, id].compactMap { $0 }.joined(separator: " · ") }
                }
                row.depth = 0
                return row
            }
        }
        return rows
    }

    /// Diagnostic inventory: no Task association, regardless of attention state.
    func orphanSessions(search: String) -> [SessionRecord] {
        let query = search.trimmingCharacters(in: .whitespacesAndNewlines)
        let records = waves.flatMap { $0.sessions } + unmatchedSessions
        return records.filter { session in
            guard session.workspace?.taskId == nil, session.taskIds.isEmpty else { return false }
            return query.isEmpty || [session.title, session.detail, session.workPath ?? ""]
                .contains { $0.localizedCaseInsensitiveContains(query) }
        }
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
    let sessionId: String
    var text: String
    var error: String?
    var submitting = false
}

/// One Task's in-flight `lf task run` or `lf task move` and its refusal.
struct TaskFlowDraft: Equatable {
    var acting = false
    var error: String?
}

/// Per-window/repository navigation; independent of terminal layout and liveness.
@MainActor
@Observable
final class WorkNavigation {
    enum Content { case overview, details, terminals }
    enum Palette: Equatable {
        case search
        case flow(String)
    }
    var content: Content = .overview
    var palette: Palette?
    var recentDestinations: [WorkPaletteRow] = []
    /// Source revisions isolate disclosure from changed definitions and repositories.
    var expandedTemplateGroups: [String: Set<String>] = [:]
    /// Show a launched repository shell while retaining the selected Task.
    var showsRetainedTerminals = false
    var showsActivity = false
    var presentation: WorkPresentation = .compact
    var selectedSessionId: String? {
        didSet {
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
    var showsHeadlessSessions = false
    /// Flow drafts by planning Task id; they survive Task and Session navigation.
    var flowDrafts: [String: TaskFlowDraft] = [:]
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
    /// Flow execs whose graph and steps are open, keyed by driver Exec.
    var expandedFlowRuns: Set<String> = []
    var repositoryCollapsed = false
    /// The orphan Session section's disclosure. `nil` follows the default:
    /// collapsed beside planned Work, open when orphans are all there is.
    var collapsed: Set<WorkNodeKey> = []
    var search = ""
    var selection: WorkReference?
    var selectedTaskEvidence: (wave: WaveRoadmap, task: RoadmapTask)?
    var listScrollOffset: CGFloat = 0

    func isExpanded(_ key: WorkNodeKey) -> Bool {
        !search.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !collapsed.contains(key)
    }

    func toggle(_ key: WorkNodeKey) {
        guard search.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        if !collapsed.insert(key).inserted { collapsed.remove(key) }
    }
}
