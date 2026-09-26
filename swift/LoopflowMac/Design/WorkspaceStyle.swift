// Shared visual language for the workspace (polish direction D): A's sidebar,
// B's dense center structure, C's warm surfaces. Tokens derive from the active
// palette so every appearance keeps its own contrast.

import Loopflow
import SwiftUI

extension LoopflowPalette {
    /// Quiet labels, ranks, and IDs: one step below secondary text.
    var textTertiary: Color { textSecondary.opacity(0.78) }
    /// Hairlines between chips and the outline of neutral controls.
    var borderStrong: Color { text.opacity(0.16) }
    /// The sidebar's selected row: a lifted tint, marked by the accent edge.
    var selectionTint: Color { surface.opacity(0.72) }
}

/// Every workspace section heading: small tracked caps, one system.
struct WorkspaceSectionHeading<Trailing: View>: View {
    let title: String
    var count: Int?
    @ViewBuilder var trailing: Trailing

    @Environment(\.palette) private var palette

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
            Text(title)
                .textCase(.uppercase)
                .font(Typography.caption(11).weight(.bold))
                .tracking(0.66)
                .foregroundStyle(palette.textTertiary)
            if let count {
                Text("\(count)")
                    .font(Typography.caption(11))
                    .monospacedDigit()
                    .foregroundStyle(palette.textTertiary)
            }
            Spacer(minLength: Spacing.sm)
            trailing
        }
        .accessibilityElement(children: .contain)
    }
}

extension WorkspaceSectionHeading where Trailing == EmptyView {
    init(_ title: String, count: Int? = nil) {
        self.init(title: title, count: count) { EmptyView() }
    }
}

/// Tone shared by plan chips, Session state, status dots, and Flow nodes.
enum WorkspaceTone {
    case running, done, human, blocked, stopped, neutral

    var ink: Color {
        switch self {
        case .running: Color(hex: 0x2F6BC0)
        case .done: Color(hex: 0x3E7A4E)
        case .human: Color(hex: 0x9A6B12)
        case .blocked: Color(hex: 0xB23A30)
        case .stopped: Color(hex: 0x6F6862)
        case .neutral: Color(hex: 0x8C837C)
        }
    }

    /// Opaque fill so a tone keeps its hue over a loop tint.
    var fill: Color {
        switch self {
        case .running: Color(hex: 0xE7EFFA)
        case .done: Color(hex: 0xD6EBD9)
        case .human: Color(hex: 0xF8E3AE)
        case .blocked: Color(hex: 0xFBE6E3)
        case .stopped: Color(hex: 0xEEEAE5)
        case .neutral: .clear
        }
    }

    var stroke: Color {
        switch self {
        case .running: ink
        case .done: Color(hex: 0x9CC4A3)
        case .human: Color(hex: 0xDDB763)
        case .blocked: ink
        case .stopped: ink
        case .neutral: Color(hex: 0xD6CCC0)
        }
    }

    /// Text on `fill`.
    var text: Color {
        switch self {
        case .running: Color(hex: 0x1C4480)
        case .done: Color(hex: 0x28523A)
        case .human: Color(hex: 0x6F4C0C)
        case .blocked: Color(hex: 0x8A2A22)
        case .stopped, .neutral: Color(hex: 0x5F5752)
        }
    }
}

/// Compact state label with an aligned column footprint.
struct WorkspaceChip: View {
    let text: String
    let tone: WorkspaceTone

    var body: some View {
        Text(text)
            .font(Typography.caption(10.5).weight(.bold))
            .tracking(0.2)
            .foregroundStyle(tone == .neutral ? tone.ink : tone.text)
            .lineLimit(1)
            .padding(.horizontal, 7)
            .padding(.vertical, 1)
            .background(tone.fill, in: RoundedRectangle(cornerRadius: 4))
            .overlay {
                if tone == .neutral {
                    RoundedRectangle(cornerRadius: 4).strokeBorder(tone.stroke, lineWidth: 1)
                }
            }
    }
}

extension View {
    /// A soft warm card: the only framed surfaces are Flow, Sessions, and plans.
    func workspacePanel(padding: CGFloat = 0) -> some View {
        modifier(WorkspacePanel(padding: padding))
    }
}

private struct WorkspacePanel: ViewModifier {
    let padding: CGFloat
    @Environment(\.palette) private var palette

    func body(content: Content) -> some View {
        content
            .padding(padding)
            .background(palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.lg))
            .overlay(RoundedRectangle(cornerRadius: CornerRadius.lg).strokeBorder(palette.border.opacity(0.7)))
            .shadow(color: Color.black.opacity(0.05), radius: 9, y: 5)
    }
}

/// Primary actions as an accent outline: burgundy marks the action, never fills.
struct WorkspaceOutlineButtonStyle: ButtonStyle {
    @Environment(\.palette) private var palette
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Typography.body(12.5))
            .foregroundStyle(isEnabled ? palette.accent : palette.textTertiary)
            .padding(.horizontal, 11)
            .padding(.vertical, 4)
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .fill(configuration.isPressed ? palette.accent.opacity(0.10) : palette.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 6)
                    .strokeBorder(isEnabled ? palette.accent : palette.borderStrong, lineWidth: 1)
            )
            .contentShape(RoundedRectangle(cornerRadius: 6))
    }
}
