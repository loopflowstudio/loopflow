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

struct WorkspaceProject: Identifiable {
    let id: WorkspaceNodeKey
    let project: RoadmapProject
    let sessions: [SessionRecord]
    let tasks: [WorkspaceTask]
}

struct WorkspaceWave: Identifiable {
    let id: WorkspaceNodeKey
    let roadmap: WaveRoadmap
    let sessions: [SessionRecord]
    let projects: [WorkspaceProject]
}

/// A presentation of the two shared readings, never another inventory.
struct WorkspaceProjection {
    let waves: [WorkspaceWave]
    let unmatchedSessions: [SessionRecord]

    init(roadmaps: [WaveRoadmap], sessions: [SessionRecord]) {
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
                sessions: attached(to: .wave(id: wave.wave.id)),
                projects: wave.projects.items.map { project in
                    WorkspaceProject(
                        id: WorkspaceNodeKey(repo: repo, work: .project(id: project.id)),
                        project: project,
                        sessions: attached(to: project.runtime.map { .project(id: $0.workId) }),
                        tasks: project.tasks.sorted { $0.task.rank < $1.task.rank }.compactMap { task in
                            let records = attached(to: task.runtime.map { .task(id: $0.workId) })
                            guard !task.task.completed || !records.isEmpty else { return nil }
                            return WorkspaceTask(
                                id: WorkspaceNodeKey(repo: repo, work: .task(id: task.id)),
                                task: task,
                                sessions: records
                            )
                        }
                    )
                }
            )
        }
        // Missing planning and unattributed Sessions remain reachable, including
        // mandatory human boundaries and multiple conversations on one subject.
        unmatchedSessions = sessions.filter { !matched.contains($0.id) }
    }

    func sessions(for work: WorkReference?) -> [SessionRecord] {
        guard let work else { return unmatchedSessions }
        for wave in waves {
            if wave.id.work == work { return wave.sessions }
            for project in wave.projects {
                if project.id.work == work { return project.sessions }
                if let task = project.tasks.first(where: { $0.id.work == work }) {
                    return task.sessions
                }
            }
        }
        return []
    }

    func subject(for sessionId: String) -> WorkReference? {
        for wave in waves {
            if wave.sessions.contains(where: { $0.id == sessionId }) { return wave.id.work }
            for project in wave.projects {
                if project.sessions.contains(where: { $0.id == sessionId }) { return project.id.work }
                if let task = project.tasks.first(where: { $0.sessions.contains { $0.id == sessionId } }) {
                    return task.id.work
                }
            }
        }
        return nil
    }
}

/// Per-window/repository navigation; independent of terminal layout and liveness.
@MainActor
@Observable
final class WorkspaceNavigation {
    enum Content { case overview, details, terminals }
    var content: Content = .overview
    var showsList = false
    var collapsed: Set<WorkspaceNodeKey> = []
    var search = ""
    var selection: WorkReference?

    func isExpanded(_ key: WorkspaceNodeKey) -> Bool {
        !search.isEmpty || !collapsed.contains(key)
    }

    func toggle(_ key: WorkspaceNodeKey) {
        guard search.isEmpty else { return }
        if !collapsed.insert(key).inserted { collapsed.remove(key) }
    }
}
