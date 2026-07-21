import SwiftUI

/// The operational lens shared by Wave, Project, and Task rows: a small recessed
/// glass light whose color is the shared attention grammar. Green = a live body
/// is advancing; red = unhealthy; blue = waiting for the user; black = off and
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

    /// The shared attention level, mapped 1:1. W2-123's Rust owns the level; the
    /// lens renders it, it never reconstructs it from status or process flags.
    public init(_ level: TaskAttentionLevel) {
        switch level {
        case .green: self = .green
        case .red: self = .red
        case .blue: self = .blue
        case .black: self = .black
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

    /// Task lens: the shared attention level and reason, verbatim. The API gives
    /// Tasks a Rust-owned `TaskAttentionSnapshot`, so the lens spends it directly
    /// and invents nothing.
    public static func forTask(_ attention: TaskAttentionSnapshot) -> WaveLens {
        WaveLens(color: WaveLensColor(attention.level), reason: attention.reason)
    }

    /// Project lens: derived only from shared runtime and the Project's Tasks'
    /// attention evidence — never a filesystem guess. A live Project body is
    /// advancing (green), but a Task that needs a hand still wins (red or blue),
    /// so a Project never hides its stuck work behind an advancing sibling. Among
    /// the remaining Tasks the most demanding evidence wins (red > blue > green >
    /// unknown > black), so unreadable Task evidence surfaces as unknown, not a
    /// silent black.
    public static func forProject(
        runtime: ProjectRuntimeSnapshot?,
        tasks: [WaveTaskWork]
    ) -> WaveLens {
        let folded = fold(tasks.map(\.attention))
        if let runtime, runtime.processAlive, runtime.status.isRunning {
            // A live body is advancing — unless a Task is calling for attention.
            if let folded, folded.color == .red || folded.color == .blue { return folded }
            return WaveLens(color: .green, reason: runtime.reason)
        }
        if let folded { return folded }
        return WaveLens(color: .black, reason: "Off · no active work")
    }

    /// Wave lens (list context): derived only from the shared runtime `lf ls`
    /// carries for every row — liveness, lifecycle status, and active-work counts.
    /// Per-Task attention is a focused `lf status` read, never fetched per row, so
    /// the list projects from the coarse runtime facts. An unregistered Wave has
    /// no such reading; see `WaveViewModel.lens`, which shows it as unknown rather
    /// than guessing from a local session probe.
    ///
    /// - green: the Wave listener answered its health probe.
    /// - blue: authored policy pauses new turns; listener evidence stays in the reason.
    /// - red: an active Run or planned work has no answering listener.
    /// - black: off and clean — no live body, no active work.
    public static func forWave(
        live: Bool,
        paused: Bool = false,
        status: WorkStatus,
        activeTasks: Int,
        activeProjects: Int
    ) -> WaveLens {
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
        if status.isRunning {
            return WaveLens(color: .red, reason: "Run active · Wave listener did not answer")
        }
        let outstanding = activeTasks + activeProjects
        if outstanding > 0 {
            let noun = outstanding == 1 ? "item" : "items"
            return WaveLens(
                color: .red,
                reason: "Stopped · \(outstanding) active \(noun) still expect work"
            )
        }
        return WaveLens(color: .black, reason: "Off · no active work")
    }

    /// Fold Task attention into the parent's single reading. Priority is
    /// red > blue > green > unknown > black: failure and user attention outrank motion, and unproven
    /// evidence outranks (never collapses into) an off-and-clean black. Returns
    /// nil when there are no Tasks to read.
    private static func fold(_ attentions: [TaskAttentionSnapshot]) -> WaveLens? {
        guard !attentions.isEmpty else { return nil }
        for level in [TaskAttentionLevel.red, .blue, .green, .unknown, .black] {
            if let hit = attentions.first(where: { $0.level == level }) {
                return WaveLens(color: WaveLensColor(level), reason: hit.reason)
            }
        }
        return nil
    }
}
