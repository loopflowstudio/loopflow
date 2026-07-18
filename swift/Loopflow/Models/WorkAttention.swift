import Foundation

/// The NOW lens: the live-and-stuck frontier across every Wave, grouped by who
/// must move next. A display grouping over the Rust-owned attention signal in
/// the one `lf roadmap` read, re-shaped into a flat, cross-wave list. It
/// invents no Work or process state.
public enum NowGroup: String, CaseIterable, Sendable, Hashable {
    case readyForReview
    case needsInput
    case working
    case stopped
    case failed
    case unknown

    public var title: String {
        switch self {
        case .readyForReview: "Ready for review"
        case .needsInput: "Needs input"
        case .working: "Working"
        case .stopped: "Stopped"
        case .failed: "Failed"
        case .unknown: "Unknown"
        }
    }
}

/// Which NOW group a Task belongs to, or `nil` when it is not a NOW row.
///
/// Rust owns whether the row is advancing, needs attention, is quiet, or could
/// not be proven. Swift only gives red rows a useful reading group; it never
/// reconstructs the attention level from status and process flags.
public func nowGroup(for task: RoadmapTask) -> NowGroup? {
    switch task.attention.level {
    case .blue:
        return .readyForReview
    case .green:
        return .working
    case .black:
        return nil
    case .unknown:
        return .unknown
    case .red:
        switch task.attention.nextOwner {
        case .review, .ci: return .readyForReview
        case .user: return .needsInput
        default: break
        }
        return .stopped
    }
}

/// The row's spoken contract uses the exact shared reason rendered by the CLI.
public func taskAttentionAccessibilityLabel(_ task: RoadmapTask) -> String {
    "\(task.task.identifier), \(task.task.name). \(task.attention.level.rawValue). "
        + "\(task.attention.reason). Next owner: \(task.attention.nextOwner.rawValue)."
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
/// dropped. Age uses the Work record's `updated_at` (RFC3339, so lexicographic
/// compare is chronological); NOW rows always have runtime evidence.
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
            ($0.task.runtime?.updatedAt ?? "") < ($1.task.runtime?.updatedAt ?? "")
        }
        return NowSection(group: group, rows: sorted)
    }
}
