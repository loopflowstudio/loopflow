import SwiftUI

/// The operational lens shared by Waves (and, once W2-123 lands, Projects and
/// Tasks): a small recessed glass light whose color combines desired intent with
/// observed liveness. Green = a live body is advancing; red = attention or
/// recovery is required; black = off and clean, nothing expected to run.
public enum WaveLensColor: String, Sendable, Hashable {
    case green
    case red
    case black

    /// The lit glass tone. Restrained, not a generic health palette — the HAL
    /// allusion stays implicit. Black is near-black glass rather than a bright dot.
    public var glow: Color {
        switch self {
        case .green: Color(hex: 0x3FA96A)
        case .red: Color(hex: 0xC0392B)
        case .black: Color(hex: 0x2A2A2A)
        }
    }

    /// True when the lens is lit (green/red). Off lenses render as dark glass
    /// with no inner glow.
    public var isLit: Bool { self != .black }
}

/// A rendered lens value: its color plus the reason it holds. The reason is what
/// VoiceOver announces, so the state is legible without seeing the color.
public struct WaveLens: Sendable, Hashable {
    public let color: WaveLensColor
    public let reason: String

    public init(color: WaveLensColor, reason: String) {
        self.color = color
        self.reason = reason
    }

    /// PROVISIONAL wave-level projection. W2-123 owns the final green/red/black
    /// semantics from shared liveness, next-owner, and unsettled-progress
    /// evidence; until that field exists this maps the facts `lf ls` already
    /// carries. Replace only this function when the shared projection lands —
    /// the lens rendering and every row stay put.
    ///
    /// - green: a live body is advancing.
    /// - red: no live body, but active work still expects execution.
    /// - black: off and clean — no live body, no active work.
    public static func provisional(
        live: Bool,
        status: WaveStatus,
        activeTasks: Int,
        activeProjects: Int
    ) -> WaveLens {
        let hasLiveBody = live || status == .running
        if hasLiveBody {
            return WaveLens(color: .green, reason: "Running · a body is advancing")
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
}
