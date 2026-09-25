import Foundation
import Loopflow
import Observation

/// Planning identity stays stable when durable Task Work or a Session appears.
struct WorkspaceNodeKey: Hashable {
    let repo: String
    let work: WorkReference
}

struct WorkspaceTask: Identifiable {
    let id: WorkspaceNodeKey
    let task: RoadmapTask
    let sessions: [SessionRecord]
}

enum WorkspacePresentation: String, CaseIterable {
    case compact = "Compact"
    case full = "Full hierarchy"
    case sessions = "Sessions"
}

struct WorkspaceWave: Identifiable {
    let id: WorkspaceNodeKey
    let roadmap: WaveRoadmap
    let sessions: [SessionRecord]
    let tasks: [WorkspaceTask]
}

/// A presentation of the two shared readings, never another inventory.
struct WorkspaceProjection {
    let waves: [WorkspaceWave]
    let unmatchedSessions: [SessionRecord]

    init(roadmaps: [WaveRoadmap], sessions: [SessionRecord], activeWorktrees: Set<String> = []) {
        var matched = Set<String>()
        func attached(to work: WorkReference?) -> [SessionRecord] {
            guard let work else { return [] }
            let records = sessions.filter { $0.work == work }
            matched.formUnion(records.map(\.id))
            return records
        }
        waves = roadmaps.map { wave in
            let repo = wave.wave.repo
            return WorkspaceWave(
                id: WorkspaceNodeKey(repo: repo, work: .wave(id: wave.wave.id)),
                roadmap: wave,
                sessions: {
                    let records = sessions.filter {
                        $0.work == .wave(id: wave.wave.id)
                            || ($0.work?.kind == .project && $0.waveId == wave.wave.id)
                    }
                    matched.formUnion(records.map(\.id))
                    return records
                }(),
                tasks: wave.tasks.items.sorted { $0.task.rank < $1.task.rank }.compactMap { task in
                    let records = attached(to: task.runtime.map { .task(id: $0.workId) })
                    let providerInCheckout = task.reference.workspace.map { activeWorktrees.contains($0.worktree) } ?? false
                    guard !task.task.completed || !records.isEmpty || providerInCheckout
                        || task.reference.workspace?.localExists == true else { return nil }
                    return WorkspaceTask(
                        id: WorkspaceNodeKey(repo: repo, work: .task(id: task.id)),
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
    func breadcrumb(selection: WorkReference?, sessionId: String?) -> WorkspaceBreadcrumb? {
        if let sessionId {
            for wave in waves {
                if let session = wave.sessions.first(where: { $0.id == sessionId }) {
                    return WorkspaceBreadcrumb(wave: wave, task: nil, session: session, siblings: wave.sessions)
                }
                if let task = wave.tasks.first(where: { $0.sessions.contains { $0.id == sessionId } }),
                   let session = task.sessions.first(where: { $0.id == sessionId }) {
                    return WorkspaceBreadcrumb(wave: wave, task: task, session: session, siblings: task.sessions)
                }
            }
            if let session = unmatchedSessions.first(where: { $0.id == sessionId }) {
                return WorkspaceBreadcrumb(wave: nil, task: nil, session: session, siblings: [session])
            }
        }
        guard let selection else { return nil }
        for wave in waves {
            if wave.id.work == selection {
                return WorkspaceBreadcrumb(wave: wave, task: nil, session: nil, siblings: wave.sessions)
            }
            if let task = wave.tasks.first(where: { $0.id.work == selection }) {
                return WorkspaceBreadcrumb(wave: wave, task: task, session: nil, siblings: task.sessions)
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
        return nil
    }
}

/// Wave → Task → Session. Missing ancestry stays missing rather than guessed.
struct WorkspaceBreadcrumb {
    let wave: WorkspaceWave?
    let task: WorkspaceTask?
    let session: SessionRecord?
    let siblings: [SessionRecord]
}

struct WorkspaceOutlineSubject {
    let key: WorkspaceNodeKey
    let title: String
}

/// Disposable visible rows. Planning and Session identities remain the source.
struct WorkspaceOutlineRow: Identifiable {
    enum Content {
        case work(WorkspaceOutlineSubject, hasChildren: Bool)
        case session(SessionRecord)
    }

    enum ID: Hashable {
        case work(WorkspaceNodeKey)
        case session(String)
    }

    let content: Content
    var detail: String?
    var depth: Int
    let ancestors: [WorkspaceOutlineSubject]

    var id: ID {
        switch content {
        case .work(let subject, _): .work(subject.key)
        case .session(let session): .session(session.id)
        }
    }

    var title: String {
        switch content {
        case .work(let subject, _): subject.title
        case .session(let session): session.title
        }
    }

    var session: SessionRecord? {
        guard case .session(let session) = content else { return nil }
        return session
    }

    var workKey: WorkspaceNodeKey? {
        guard case .work(let subject, _) = content else { return nil }
        return subject.key
    }
}

extension WorkspaceProjection {
    func outline(
        presentation: WorkspacePresentation, collapsed: Set<WorkspaceNodeKey>,
        search: String, planningReadable: Bool
    ) -> [WorkspaceOutlineRow] {
        let query = search.trimmingCharacters(in: .whitespacesAndNewlines)
        func matches(_ text: String) -> Bool {
            query.isEmpty || text.localizedCaseInsensitiveContains(query)
        }
        func expanded(_ key: WorkspaceNodeKey) -> Bool {
            !query.isEmpty || !collapsed.contains(key)
        }
        var rows: [WorkspaceOutlineRow] = []
        func appendSession(_ session: SessionRecord, depth: Int, ancestors: [WorkspaceOutlineSubject], include: Bool = false) {
            guard include || matches(session.title) || matches(session.detail)
                || ancestors.contains(where: { matches($0.title) }) else { return }
            rows.append(WorkspaceOutlineRow(
                content: .session(session),
                detail: ancestors.isEmpty ? session.workPath ?? "Repository or unavailable ancestry" : nil,
                depth: depth, ancestors: ancestors
            ))
        }
        func appendWork(_ subject: WorkspaceOutlineSubject, detail: String? = nil, depth: Int,
                        ancestors: [WorkspaceOutlineSubject], hasChildren: Bool) {
            rows.append(WorkspaceOutlineRow(
                content: .work(subject, hasChildren: hasChildren), detail: detail, depth: depth,
                ancestors: ancestors
            ))
        }
        for wave in waves {
            let waveSubject = WorkspaceOutlineSubject(key: wave.id, title: wave.roadmap.wave.name)
            let complete: Bool
            if case .available(_, false) = wave.roadmap.tasks {
                complete = planningReadable && wave.roadmap.unavailableTasks.isEmpty
            } else { complete = false }
            let flat = presentation == .sessions
            let waveHasChildren = !wave.tasks.isEmpty || !wave.sessions.isEmpty
            let omitWave = flat || (presentation == .compact && waves.count == 1 && complete
                && waveHasChildren && expanded(wave.id))
            let waveStart = rows.count
            if !omitWave { appendWork(waveSubject, depth: 0, ancestors: [], hasChildren: waveHasChildren) }
            if flat || expanded(wave.id) {
                let waveDepth = omitWave ? 0 : 1
                for session in wave.sessions { appendSession(session, depth: waveDepth, ancestors: [waveSubject]) }
                let ancestors = [waveSubject]
                for task in wave.tasks {
                    let taskSubject = WorkspaceOutlineSubject(key: task.id, title: task.task.task.name)
                    let taskMatches = matches(taskSubject.title) || matches(task.task.task.identifier)
                        || matches(waveSubject.title)
                        || task.sessions.contains(where: { matches($0.title) || matches($0.detail) })
                    guard taskMatches else { continue }
                    if !flat {
                        appendWork(taskSubject, detail: task.task.task.identifier, depth: waveDepth,
                                   ancestors: ancestors, hasChildren: !task.sessions.isEmpty)
                    }
                    if flat || expanded(task.id) {
                        for session in task.sessions {
                            appendSession(session, depth: flat ? 0 : waveDepth + 1,
                                          ancestors: ancestors + [taskSubject],
                                          include: matches(task.task.task.identifier))
                        }
                    }
                }
            }
            if !query.isEmpty, !omitWave, rows.count == waveStart + 1, !matches(waveSubject.title) { rows.removeLast() }
        }
        for session in unmatchedSessions { appendSession(session, depth: 0, ancestors: []) }
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
}

/// An in-place Session name edit. It belongs to exactly one Session; a
/// completion for another Session can never touch it.
struct SessionRenameDraft: Equatable {
    let sessionId: String
    var text: String
    var error: String?
    var submitting = false
}

/// Per-window/repository navigation; independent of terminal layout and liveness.
@MainActor
@Observable
final class WorkspaceNavigation {
    enum Content { case overview, details, terminals }
    var content: Content = .overview
    var presentation: WorkspacePresentation = .compact
    var selectedSessionId: String? {
        didSet {
            // Leaving a Session abandons an unsubmitted edit; a submitted one
            // still settles against its own Session.
            if let renaming, renaming.sessionId != selectedSessionId, !renaming.submitting {
                self.renaming = nil
            }
        }
    }
    var renaming: SessionRenameDraft?
    var repositoryCollapsed = false
    var collapsed: Set<WorkspaceNodeKey> = []
    var search = ""
    var selection: WorkReference?
    var selectedTaskEvidence: (wave: WaveRoadmap, task: RoadmapTask)?
    var listScrollOffset: CGFloat = 0
    var taskPanes: [String: (path: String, pane: PaneState)] = [:]

    func isExpanded(_ key: WorkspaceNodeKey) -> Bool {
        !search.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !collapsed.contains(key)
    }

    func toggle(_ key: WorkspaceNodeKey) {
        guard search.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        if !collapsed.insert(key).inserted { collapsed.remove(key) }
    }
}
