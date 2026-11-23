import SwiftUI

/// Tone shared by plan chips, Session state, status dots, and Graph nodes.
public enum WorkTone: Sendable {
    case running, done, available, warning, failure, stopped, neutral

    public func ink(in palette: LoopflowPalette) -> Color {
        switch self {
        case .stopped: palette.textSecondary
        case .neutral: palette.textTertiary
        default: style(in: palette).ink
        }
    }

    private func style(in palette: LoopflowPalette) -> ThemeTone {
        switch self {
        case .running: palette.workingTone
        case .done: palette.successTone
        case .available: palette.availableTone
        case .warning: palette.warningTone
        case .failure: palette.failureTone
        case .stopped, .neutral: palette.stoppedTone
        }
    }
    public func fill(in palette: LoopflowPalette) -> Color { self == .neutral ? .clear : style(in: palette).fill }
    public func stroke(in palette: LoopflowPalette) -> Color { style(in: palette).stroke }
    public func text(in palette: LoopflowPalette) -> Color { style(in: palette).text }
}
