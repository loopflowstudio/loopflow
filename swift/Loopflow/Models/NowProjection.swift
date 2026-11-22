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

/// The row's spoken contract uses the exact shared reason rendered by the CLI.
public func taskConditionAccessibilityLabel(_ task: RoadmapTask) -> String {
    "\(task.task.identifier), \(task.task.name). \(task.condition.state.rawValue). "
        + "\(task.condition.reason). Next owner: \(task.nextMove.owner.rawValue)."
}

/// One flat NOW row: a Task lifted out of the tree, carrying its Wave and
/// Project only as context, plus the group it landed in.
public struct NowRow: Identifiable, Sendable, Hashable {
    public let wave: WaveSnapshot?
    public let task: RoadmapTask
    public let group: TaskConditionState

    public var id: String { task.id }

    public init(
        wave: WaveSnapshot?,
        task: RoadmapTask,
        group: TaskConditionState
    ) {
        self.wave = wave
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

/// Flatten the roadmap into the NOW shape: every Task that needs someone
/// across every visible Wave, bucketed by the word its row says, oldest-first within each bucket, with empty buckets
/// dropped. Age uses the Work record's `updated_at` (RFC3339, so lexical order
/// is chronological); NOW rows always have runtime evidence.
public func nowSections(from tasks: [RoadmapTask], waves: [WaveRoadmap]) -> [NowSection] {
    var rowsByGroup: [TaskConditionState: [NowRow]] = [:]
    for task in tasks {
        guard let group = task.need else { continue }
        let wave = waves.first { $0.wave.id == task.task.waveId }?.wave
        rowsByGroup[group, default: []].append(NowRow(wave: wave, task: task, group: group))
    }
    return [TaskConditionState.blocked, .waiting, .unknown].compactMap { group in
        guard let rows = rowsByGroup[group], !rows.isEmpty else { return nil }
        let sorted = rows.sorted {
            ($0.task.runtime?.updatedAt ?? "") < ($1.task.runtime?.updatedAt ?? "")
        }
        return NowSection(group: group, rows: sorted)
    }
}
