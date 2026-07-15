import Foundation

/// The NOW lens: the live-and-stuck frontier across every Wave, grouped by who
/// must move next. A pure projection over the one `lf roadmap` read — the same
/// shared fields the ROADMAP tree renders, re-shaped into a flat, cross-wave,
/// attention-grouped list. It invents no session state; if the CLI ever wants
/// the same flat view it reuses `nowGroup(for:)` verbatim.
public enum NowGroup: String, CaseIterable, Sendable, Hashable {
    case readyForReview
    case needsInput
    case working
    case stopped
    case failed

    public var title: String {
        switch self {
        case .readyForReview: "Ready for review"
        case .needsInput: "Needs input"
        case .working: "Working"
        case .stopped: "Stopped"
        case .failed: "Failed"
        }
    }
}

/// Which NOW group a Task belongs to, or `nil` when it is not a NOW row.
///
/// NOW is the frontier of live-or-stuck work: it excludes Tasks nobody has
/// started (`runtime == nil` — those are Available in ROADMAP) and terminal
/// Sessions (completed/abandoned — those are done). The order of the checks is
/// the priority: a Task with an open PR reads as *Ready for review* even though
/// its Session is idle, because the PR is the thing waiting on a human.
public func nowGroup(for task: RoadmapTask) -> NowGroup? {
    if task.task.completed { return nil }
    guard let runtime = task.runtime else { return nil }
    if runtime.status == .completed || runtime.status == .abandoned { return nil }

    if let pr = task.activePr, pr.phase != .merged, pr.phase != .abandoned {
        return .readyForReview
    }
    switch task.nextMove.owner {
    case .review: return .readyForReview
    case .human: return .needsInput
    default: break
    }
    if runtime.status == .failed { return .failed }
    if runtime.processAlive && (runtime.status == .starting || runtime.status == .running) {
        return .working
    }
    // A Session that exists, is non-terminal, but is not advancing itself: a
    // dead or idle process the user can resume. A dead process is not a dead
    // task — the row stays actionable and says why it stopped.
    return .stopped
}

/// One flat NOW row: a Task lifted out of the tree, carrying its Wave and
/// Project only as context, plus the group it landed in.
public struct NowRow: Identifiable, Sendable, Hashable {
    public let wave: WaveSnapshot
    public let projectName: String
    public let task: RoadmapTask
    public let group: NowGroup

    public var id: String { "\(wave.id):\(task.id)" }

    public init(wave: WaveSnapshot, projectName: String, task: RoadmapTask, group: NowGroup) {
        self.wave = wave
        self.projectName = projectName
        self.task = task
        self.group = group
    }
}

public struct NowSection: Identifiable, Sendable, Hashable {
    public let group: NowGroup
    public let rows: [NowRow]

    public var id: String { group.rawValue }

    public init(group: NowGroup, rows: [NowRow]) {
        self.group = group
        self.rows = rows
    }
}

/// Flatten the roadmap into the NOW shape: every Task across every visible Wave,
/// bucketed by attention, oldest-first within each bucket, empty buckets
/// dropped. Age is the Session's `status_at` (RFC3339, so lexicographic compare
/// is chronological); NOW rows always have a runtime, so no timestamp is
/// missing.
public func nowSections(from waves: [WaveRoadmap]) -> [NowSection] {
    var rowsByGroup: [NowGroup: [NowRow]] = [:]
    for wave in waves {
        for project in wave.projects.items {
            for task in project.tasks {
                guard let group = nowGroup(for: task) else { continue }
                rowsByGroup[group, default: []].append(
                    NowRow(
                        wave: wave.wave,
                        projectName: project.project.name,
                        task: task,
                        group: group
                    )
                )
            }
        }
    }
    return NowGroup.allCases.compactMap { group in
        guard let rows = rowsByGroup[group], !rows.isEmpty else { return nil }
        let sorted = rows.sorted {
            ($0.task.runtime?.statusAt ?? "") < ($1.task.runtime?.statusAt ?? "")
        }
        return NowSection(group: group, rows: sorted)
    }
}
