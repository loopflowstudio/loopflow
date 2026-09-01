import Foundation

public extension TaskConditionState {
    var title: String {
        switch self {
        case .waiting: "Waiting"
        case .blocked: "Blocked"
        case .clear: "Clear"
        case .unknown: "Unknown"
        }
    }
}

/// NOW shows non-clear Task conditions from the one `lf roadmap` read. Rust
/// owns the condition; Swift only omits clear rows and groups the rest.
public func nowGroup(for task: RoadmapTask) -> TaskConditionState? {
    switch task.condition.state {
    case .waiting: return .waiting
    case .blocked: return .blocked
    case .unknown: return .unknown
    case .clear:
        return nil
    }
}

/// The row's spoken contract uses the exact shared reason rendered by the CLI.
public func taskConditionAccessibilityLabel(_ task: RoadmapTask) -> String {
    "\(task.task.identifier), \(task.task.name). \(task.condition.state.rawValue). "
        + "\(task.condition.reason). Next owner: \(task.nextMove.owner.rawValue)."
}

/// One flat NOW row: a Task lifted out of the tree, carrying its Wave and
/// Project only as context, plus the group it landed in.
public struct NowRow: Identifiable, Sendable, Hashable {
    public let wave: WaveSnapshot
    public let projectName: String
    public let task: RoadmapTask
    public let group: TaskConditionState

    public var id: String { "\(wave.id):\(task.id)" }

    public init(
        wave: WaveSnapshot,
        projectName: String,
        task: RoadmapTask,
        group: TaskConditionState
    ) {
        self.wave = wave
        self.projectName = projectName
        self.task = task
        self.group = group
    }
}

public struct NowSection: Identifiable, Sendable, Hashable {
    public let group: TaskConditionState
    public let rows: [NowRow]

    public var id: String { group.rawValue }

    public init(group: TaskConditionState, rows: [NowRow]) {
        self.group = group
        self.rows = rows
    }
}

/// Flatten the roadmap into the NOW shape: every Task across every visible Wave,
/// bucketed by condition, oldest-first within each bucket, with empty buckets
/// dropped. Age uses the Work record's `updated_at` (RFC3339, so lexical order
/// is chronological); NOW rows always have runtime evidence.
public func nowSections(from waves: [WaveRoadmap]) -> [NowSection] {
    var rowsByGroup: [TaskConditionState: [NowRow]] = [:]
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
    return [TaskConditionState.blocked, .waiting, .unknown].compactMap { group in
        guard let rows = rowsByGroup[group], !rows.isEmpty else { return nil }
        let sorted = rows.sorted {
            ($0.task.runtime?.updatedAt ?? "") < ($1.task.runtime?.updatedAt ?? "")
        }
        return NowSection(group: group, rows: sorted)
    }
}
