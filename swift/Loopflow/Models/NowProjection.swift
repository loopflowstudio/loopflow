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

/// What a Task's row speaks: its ID and name, then, when it needs someone, the
/// word its row says, the shared reason the CLI prints and who moves next.
public func taskConditionAccessibilityLabel(_ task: RoadmapTask) -> String {
    let name = "\(task.task.identifier), \(task.task.name)."
    guard let need = task.need else { return name }
    return "\(name) \(need.title). \(task.condition.reason). Next owner: \(task.nextMove.owner.rawValue)."
}

/// One flat NOW row: a Task lifted out of the tree, carrying its Wave only as
/// context.
public struct NowRow: Identifiable, Sendable, Hashable {
    public let wave: WaveSnapshot?
    public let task: RoadmapTask

    public var id: String { task.id }

    public init(wave: WaveSnapshot?, task: RoadmapTask) {
        self.wave = wave
        self.task = task
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
/// across every visible Wave, bucketed by the word its row says, breakpoints
/// first and oldest-first among peers, with empty buckets dropped. Age uses
/// the Work record's `updated_at` (RFC3339, so lexical order is chronological);
/// NOW rows always have runtime evidence.
public func nowSections(from tasks: [RoadmapTask], waves: [WaveRoadmap]) -> [NowSection] {
    var rowsByGroup: [TaskConditionState: [NowRow]] = [:]
    for task in tasks {
        guard let group = task.need else { continue }
        let wave = waves.first { $0.wave.id == task.task.waveId }?.wave
        rowsByGroup[group, default: []].append(NowRow(wave: wave, task: task))
    }
    return [TaskConditionState.blocked, .waiting, .unknown].compactMap { group in
        guard let rows = rowsByGroup[group], !rows.isEmpty else { return nil }
        let sorted = rows.sorted {
            let lhsBreakpoint = $0.task.selectionStates.contains(.breakpoint)
            let rhsBreakpoint = $1.task.selectionStates.contains(.breakpoint)
            if lhsBreakpoint != rhsBreakpoint { return lhsBreakpoint }
            return ($0.task.runtime?.updatedAt ?? "") < ($1.task.runtime?.updatedAt ?? "")
        }
        return NowSection(group: group, rows: sorted)
    }
}
