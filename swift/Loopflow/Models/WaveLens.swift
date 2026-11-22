import SwiftUI

/// The operating state shared by Wave, Project, and Task rows, drawn as a
/// status mark whose color is the shared operating grammar. Green = a live body
/// is advancing; red = blocked; blue = waiting on another actor; black = off and
/// clean, nothing expected to run; unknown = evidence could not be read.
public enum WaveLensColor: String, Sendable, Hashable {
    case green
    case red
    case blue
    case black
    case unknown

    /// The status mark's tone. Black is off and clean, so it is the quiet
    /// neutral; unknown is warning so unproven never reads as off.
    public var tone: WorkTone {
        switch self {
        case .green: .done
        case .red: .failure
        case .blue: .running
        case .black: .neutral
        case .unknown: .warning
        }
    }

    /// The shared Task condition, mapped 1:1. Rust owns the condition; the lens
    /// renders it and never reconstructs it from status or process flags.
    public init(_ state: TaskConditionState) {
        switch state {
        case .waiting: self = .blue
        case .blocked: self = .red
        case .clear: self = .black
        case .unknown: self = .unknown
        }
    }
}

/// A rendered lens value: its color plus the reason it holds. The reason is what
/// VoiceOver announces, so the state is legible without seeing the color. Wave,
/// Project, and Task rows all render this one value.
public struct WaveLens: Sendable, Hashable {
    public let color: WaveLensColor
    public let reason: String

    public init(color: WaveLensColor, reason: String) {
        self.color = color
        self.reason = reason
    }

    /// Task lens: the shared condition and reason, verbatim. The API gives Tasks
    /// a Rust-owned `TaskConditionSnapshot`, so the lens spends it directly
    /// and invents nothing.
    public static func forTask(_ condition: TaskConditionSnapshot) -> WaveLens {
        WaveLens(color: WaveLensColor(condition.state), reason: condition.reason)
    }

    /// Project lens: derived only from its Tasks' shared condition evidence. The
    /// most demanding evidence wins (blocked > waiting > unknown > clear),
    /// so unreadable Task evidence surfaces as unknown, not a silent black.
    public static func forTasks(tasks: [RoadmapTask]) -> WaveLens {
        let folded = fold(tasks.map(\.condition))
        if let folded { return folded }
        return WaveLens(color: .black, reason: "Off · no active work")
    }

    /// Counts do not establish Task health. Detailed conditions come from status.
    public static func forWave(activeTasks: Int) -> WaveLens {
        return WaveLens(color: activeTasks > 0 ? .unknown : .black,
                        reason: activeTasks > 0 ? "\(activeTasks) active Tasks" : "No active Tasks")
    }

    /// Fold Task conditions into the parent's single reading. Priority is
    /// blocked > waiting > unknown > clear: recovery outranks an external wait,
    /// which outranks unproven evidence and a clear Task. Returns
    /// nil when there are no Tasks to read.
    private static func fold(_ conditions: [TaskConditionSnapshot]) -> WaveLens? {
        guard !conditions.isEmpty else { return nil }
        for state in [
            TaskConditionState.blocked,
            .waiting,
            .unknown,
            .clear,
        ] {
            if let hit = conditions.first(where: { $0.state == state }) {
                return WaveLens(color: WaveLensColor(state), reason: hit.reason)
            }
        }
        return nil
    }
}
