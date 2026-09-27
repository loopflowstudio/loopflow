// Shared visual language for the workspace (polish direction D): A's sidebar,
// B's dense center structure, C's warm surfaces. Colors come from the active
// `LoopflowPalette` ramp; depth is a surface ladder joined by hairlines, and
// blue belongs to running work and loop regions only.

import AppKit
import Loopflow
import SwiftUI

extension LoopflowPalette {
    /// The sidebar's selected row: a lifted tint, marked by the accent edge.
    var selectionTint: Color { surface.opacity(0.72) }
    /// Accent used as text or a hairline. Burgundy reads on cream; on the dark
    /// palettes it falls below text contrast, so it lifts to a rose.
    var accentInk: Color { .adaptive(light: 0x722F37, dark: 0xD9959D) }
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
                .font(Typography.label)
                .tracking(0.66)
                .foregroundStyle(palette.textTertiary)
            if let count {
                Text("\(count)")
                    .font(Typography.meta)
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
        case .running: adaptive(0x2F6BC0, 0x86B0EA)
        case .done: adaptive(0x3E7A4E, 0x8DC79C)
        case .human: adaptive(0x9A6B12, 0xE3B866)
        case .blocked: adaptive(0xB23A30, 0xF08B80)
        case .stopped: adaptive(0x6F6862, 0xB5ADA5)
        case .neutral: adaptive(0x6F665F, 0xA39B93)
        }
    }

    /// Opaque fill so a tone keeps its hue over a loop tint.
    var fill: Color {
        switch self {
        case .running: adaptive(0xE7EFFA, 0x2C3E57)
        case .done: adaptive(0xD6EBD9, 0x2D4735)
        case .human: adaptive(0xF8E3AE, 0x544321)
        case .blocked: adaptive(0xFBE6E3, 0x5A2E2B)
        case .stopped: adaptive(0xEEEAE5, 0x3E434A)
        case .neutral: .clear
        }
    }

    var stroke: Color {
        switch self {
        case .running, .blocked, .stopped: ink
        case .done: adaptive(0x9CC4A3, 0x5E8E6A)
        case .human: adaptive(0xDDB763, 0x9A7A36)
        case .neutral: adaptive(0xD6CCC0, 0x56606B)
        }
    }

    /// Text on `fill`.
    var text: Color {
        switch self {
        case .running: adaptive(0x1C4480, 0xC9DCF5)
        case .done: adaptive(0x28523A, 0xC4E6CC)
        case .human: adaptive(0x6F4C0C, 0xF4DDA6)
        case .blocked: adaptive(0x8A2A22, 0xF8CBC5)
        case .stopped, .neutral: adaptive(0x5F5752, 0xD2CBC3)
        }
    }

    private func adaptive(_ light: UInt, _ dark: UInt) -> Color {
        Color.adaptive(light: light, dark: dark)
    }
}

extension Color {
    /// Resolves against the view's effective appearance, so one workspace
    /// color serves the light and dark palettes.
    static func adaptive(light: UInt, dark: UInt) -> Color {
        Color(nsColor: NSColor(name: nil) { appearance in
            let isDark = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
            return NSColor(Color(hex: isDark ? dark : light))
        })
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
    /// A framed surface one ladder step above the canvas, with a hairline and
    /// no shadow: the only framed surfaces are Flow, Sessions, and plans.
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
            .overlay(RoundedRectangle(cornerRadius: CornerRadius.lg).strokeBorder(palette.border))
    }
}

/// Primary actions as an accent outline: burgundy marks the action, never fills.
struct WorkspaceOutlineButtonStyle: ButtonStyle {
    @Environment(\.palette) private var palette
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Typography.body(12.5))
            .foregroundStyle(isEnabled ? palette.accentInk : palette.textTertiary)
            .padding(.horizontal, 11)
            .padding(.vertical, 4)
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .fill(configuration.isPressed ? palette.accentInk.opacity(0.10) : palette.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 6)
                    .strokeBorder(isEnabled ? palette.accentInk : palette.borderStrong, lineWidth: 1)
            )
            .contentShape(RoundedRectangle(cornerRadius: 6))
    }
}

/// Collapsed heading for a demand-read Task section (Comments, Recent runs):
/// chevron, caps title, optional count, and the read's progress or failure.
struct WorkspaceDisclosureHeading: View {
    let title: String
    let count: Int?
    let expanded: Bool
    let inFlight: Bool
    /// Present when the latest read failed; true when an earlier value remains.
    let failure: Bool?
    let identifier: String
    let accessibilityLabel: String
    let help: String
    let toggle: () -> Void
    let retry: () -> Void
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.sm) {
            Button(action: toggle) {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                    Image(systemName: "chevron.right")
                        .font(.system(size: 9, weight: .bold))
                        .rotationEffect(.degrees(expanded ? 90 : 0))
                    Text(title)
                        .textCase(.uppercase)
                        .font(Typography.label)
                        .tracking(0.66)
                    if let count {
                        Text("\(count)")
                            .font(Typography.meta)
                            .monospacedDigit()
                    }
                }
                .foregroundStyle(palette.textTertiary)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help(help)
            .accessibilityLabel(accessibilityLabel)
            .accessibilityIdentifier("\(identifier)-toggle")
            if inFlight {
                ProgressView().controlSize(.mini)
            } else if let hasLastGood = failure {
                Text(hasLastGood ? "May be out of date" : "Unavailable")
                    .font(Typography.caption(10.5).weight(.bold))
                    .foregroundStyle(WorkspaceTone.human.text)
                    .padding(.horizontal, 7)
                    .padding(.vertical, 1)
                    .background(WorkspaceTone.human.fill, in: RoundedRectangle(cornerRadius: 4))
                    .accessibilityIdentifier("\(identifier)-stale")
                Button("Retry", action: retry)
                    .buttonStyle(WorkspaceOutlineButtonStyle())
                    .controlSize(.small)
                    .accessibilityIdentifier("\(identifier)-retry")
            }
        }
    }
}
