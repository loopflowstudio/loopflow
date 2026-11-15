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

/// NOW shows non-clear Task conditions from the planning content. Rust
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
    public let wave: WaveSnapshot?
    public let task: RoadmapTask
    public let group: TaskConditionState
    public let parentWave: String?
    public let section: TaskSection

    public var id: String { task.id }

    public init(
        wave: WaveSnapshot?,
        task: RoadmapTask,
        group: TaskConditionState,
        parentWave: String?,
        section: TaskSection
    ) {
        self.wave = wave
        self.task = task
        self.group = group
        self.parentWave = parentWave
        self.section = section
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

/// Join planning content to projected parent/lifecycle fields for the NOW shape:
/// bucketed by condition, oldest-first within each bucket, with empty buckets
/// dropped. Age uses the Work record's `updated_at` (RFC3339, so lexical order
/// is chronological); NOW rows always have runtime evidence.
public func nowSections(from tasks: [RoadmapTask], facts: [WorkMembershipPlan.Facts], waves: [WaveRoadmap]) -> [NowSection] {
    let fields = Dictionary(facts.filter { $0.work.kind == .task }.map { ($0.work.id, $0) }, uniquingKeysWith: { first, _ in first })
    var rowsByGroup: [TaskConditionState: [NowRow]] = [:]
    for task in tasks {
        guard let fact = fields[task.id], fact.lifecycle.known,
              ![TaskSection.done, .abandoned].contains(TaskSection(states: fact.lifecycle.states)),
              let group = nowGroup(for: task) else { continue }
        let wave = waves.first { $0.wave.id == fact.parent.wave }?.wave
        rowsByGroup[group, default: []].append(NowRow(wave: wave, task: task, group: group, parentWave: fact.parent.wave, section: TaskSection(states: fact.lifecycle.states)))
    }
    return [TaskConditionState.blocked, .waiting, .unknown].compactMap { group in
        guard let rows = rowsByGroup[group], !rows.isEmpty else { return nil }
        let sorted = rows.sorted {
            ($0.task.runtime?.updatedAt ?? "") < ($1.task.runtime?.updatedAt ?? "")
        }
        return NowSection(group: group, rows: sorted)
    }
}
