import SwiftUI

/// The operational lens shared by Wave, Project, and Task rows: a small recessed
/// glass light whose color is the shared operating grammar. Green = a live body
/// is advancing; red = blocked; blue = waiting on another actor; black = off and
/// clean, nothing expected to run; unknown = evidence could not be read.
public enum WaveLensColor: String, Sendable, Hashable {
    case green
    case red
    case blue
    case black
    case unknown

    /// The lit glass tone. Restrained, not a generic health palette — the HAL
    /// allusion stays implicit. Black is near-black glass rather than a bright dot;
    /// unknown is a dim amber so unproven never reads as off.
    public var glow: Color {
        switch self {
        case .green: Color(hex: 0x3FA96A)
        case .red: Color(hex: 0xC0392B)
        case .blue: Color(hex: 0x3578C8)
        case .black: Color(hex: 0x2A2A2A)
        case .unknown: Color(hex: 0xB0862B)
        }
    }

    /// True when the lens carries an inner glow (green/red/blue/unknown). Only the off
    /// black lens renders as dark glass with no light.
    public var isLit: Bool { self != .black }

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
    public static func forProject(tasks: [WaveTaskWork]) -> WaveLens {
        let folded = fold(tasks.map(\.condition))
        if let folded { return folded }
        return WaveLens(color: .black, reason: "Off · no active work")
    }

    /// Wave lens (list context): derived only from the shared runtime `lf ls`
    /// carries for every row — liveness, lifecycle status, and active-work counts.
    /// Per-Task condition is a focused `lf status` read, never fetched per row, so
    /// the list projects from the coarse runtime facts. An unregistered Wave has
    /// no such reading; see `WaveViewModel.lens`, which shows it as unknown rather
    /// than guessing from a local session probe.
    ///
    /// - green: canonical Work is advancing and the Wave listener answered, or
    ///   the listener answered while Work claims no current body.
    /// - blue: authored policy pauses new turns; listener evidence stays in the reason.
    /// - red: enabled and observed liveness have not converged.
    /// - black: disabled and no listener remains.
    public static func forWave(
        live: Bool,
        paused: Bool = false,
        enabled: Bool = true,
        activeTasks: Int,
        activeProjects: Int
    ) -> WaveLens {
        if !enabled {
            return live
                ? WaveLens(color: .red, reason: "Disabled · listener still answered")
                : WaveLens(color: .black, reason: "Disabled on this Home")
        }
        if paused {
            return WaveLens(
                color: .blue,
                reason: live
                    ? "Paused · listener is serving and queueing input"
                    : "Paused · listener is stopped"
            )
        }
        if live {
            return WaveLens(color: .green, reason: "Listening · Wave listener answered")
        }
        let outstanding = activeTasks + activeProjects
        if outstanding > 0 {
            let noun = outstanding == 1 ? "item" : "items"
            return WaveLens(
                color: .red,
                reason: "Stopped · \(outstanding) active \(noun) still expect work"
            )
        }
        return WaveLens(color: .red, reason: "Expected live · Wave listener did not answer")
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
