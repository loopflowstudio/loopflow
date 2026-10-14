// Workspace roles come from LoopflowPalette. Selection, keyboard focus and
// program activity remain independent; terminal ANSI has its own owner.

import Loopflow
import SwiftUI

/// Every workspace section heading: small tracked caps, one system.
struct WorkSectionHeading<Trailing: View>: View {
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

extension WorkSectionHeading where Trailing == EmptyView {
    init(_ title: String, count: Int? = nil) {
        self.init(title: title, count: count) { EmptyView() }
    }
}

/// Tone shared by plan chips, Session state, status dots, and Flow nodes.
enum WorkTone {
    case running, done, waiting, failure, stopped, neutral

    func ink(in palette: LoopflowPalette) -> Color {
        switch self {
        case .running: palette.working
        case .done: palette.success
        case .waiting: palette.waiting
        case .failure: palette.failure
        case .stopped: palette.textSecondary
        case .neutral: palette.textTertiary
        }
    }

    /// Opaque fill so a tone keeps its hue over a loop tint.
    func fill(in palette: LoopflowPalette) -> Color {
        switch self {
        case .running: Color(hex: palette.isDark ? 0x2C3E57 : 0xE7EFFA)
        case .done: Color(hex: palette.isDark ? 0x2D4735 : 0xD6EBD9)
        case .waiting: Color(hex: palette.isDark ? 0x544321 : 0xF8E3AE)
        case .failure: Color(hex: palette.isDark ? 0x5A2E2B : 0xFBE6E3)
        case .stopped: Color(hex: palette.isDark ? 0x3E434A : 0xEEEAE5)
        case .neutral: .clear
        }
    }

    func stroke(in palette: LoopflowPalette) -> Color {
        switch self {
        case .running, .failure, .stopped: ink(in: palette)
        case .done: Color(hex: palette.isDark ? 0x5E8E6A : 0x9CC4A3)
        case .waiting: Color(hex: palette.isDark ? 0x9A7A36 : 0xDDB763)
        case .neutral: Color(hex: palette.isDark ? 0x56606B : 0xD6CCC0)
        }
    }

    /// Text on `fill`.
    func text(in palette: LoopflowPalette) -> Color {
        switch self {
        case .running: Color(hex: palette.isDark ? 0xC9DCF5 : 0x1C4480)
        case .done: Color(hex: palette.isDark ? 0xC4E6CC : 0x28523A)
        case .waiting: Color(hex: palette.isDark ? 0xF4DDA6 : 0x6F4C0C)
        case .failure: Color(hex: palette.isDark ? 0xF8CBC5 : 0x8A2A22)
        case .stopped, .neutral: Color(hex: palette.isDark ? 0xD2CBC3 : 0x5F5752)
        }
    }
}

extension TaskExecutionState {
    /// Shared by Flow status, plan chips and navigation dots. Quiet states have no chip.
    var presentation: (label: String?, tone: WorkTone) {
        switch self {
        case .running, .starting: ("Running", .running)
        case .blocked: ("Blocked", .waiting)
        case .stalled: ("Stalled", .waiting)
        case .idle, .unknown: (nil, .stopped)
        }
    }
}

/// Compact state label with an aligned column footprint.
struct WorkChip: View {
    let text: String
    let tone: WorkTone
    @Environment(\.palette) private var palette

    var body: some View {
        Text(text)
            .font(Typography.strong(10.5))
            .tracking(0.2)
            .foregroundStyle(tone == .neutral ? tone.ink(in: palette) : tone.text(in: palette))
            .lineLimit(1)
            .padding(.horizontal, 7)
            .padding(.vertical, 1)
            .background(tone.fill(in: palette), in: RoundedRectangle(cornerRadius: 4))
            .overlay {
                if tone == .neutral {
                    RoundedRectangle(cornerRadius: 4).strokeBorder(tone.stroke(in: palette), lineWidth: 1)
                }
            }
    }
}

extension View {
    /// A framed surface one ladder step above the canvas, with a hairline and
    /// no shadow: the only framed surfaces are Flow, Sessions, and plans.
    func workPanel(padding: CGFloat = 0) -> some View {
        modifier(WorkPanel(padding: padding))
    }
}

private struct WorkPanel: ViewModifier {
    let padding: CGFloat
    @Environment(\.palette) private var palette

    func body(content: Content) -> some View {
        content
            .padding(padding)
            .background(palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.lg))
            .overlay(RoundedRectangle(cornerRadius: CornerRadius.lg).strokeBorder(palette.border))
    }
}

/// Ordinary actions stay neutral; only a deliberately emphasized action uses burgundy.
struct WorkOutlineButtonStyle: ButtonStyle {
    var emphasized = false
    @Environment(\.palette) private var palette
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Typography.body(12.5))
            .foregroundStyle(isEnabled ? (emphasized ? palette.emphasisInk : palette.actionInk) : palette.textTertiary)
            .padding(.horizontal, 11)
            .padding(.vertical, 4)
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .fill(configuration.isPressed ? palette.actionHover : palette.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 6)
                    .strokeBorder(isEnabled && emphasized ? palette.emphasisInk : palette.borderStrong, lineWidth: 1)
            )
            .contentShape(RoundedRectangle(cornerRadius: 6))
            .workFocusRing()
    }
}

/// Collapsed heading for a demand-read Task section (Comments, Session history):
/// chevron, caps title, optional count, and the read's progress or failure.
struct WorkDisclosureHeading: View {
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
            .buttonStyle(WorkPlainButtonStyle())
            .help(help)
            .accessibilityLabel(accessibilityLabel)
            .accessibilityIdentifier("\(identifier)-toggle")
            if inFlight {
                ProgressView().controlSize(.mini)
            } else if let hasLastGood = failure {
                Text(hasLastGood ? "May be out of date" : "Unavailable")
                    .font(Typography.strong(10.5))
                    .foregroundStyle(WorkTone.waiting.text(in: palette))
                    .padding(.horizontal, 7)
                    .padding(.vertical, 1)
                    .background(WorkTone.waiting.fill(in: palette), in: RoundedRectangle(cornerRadius: 4))
                    .accessibilityIdentifier("\(identifier)-stale")
                Button("Retry", action: retry)
                    .buttonStyle(WorkOutlineButtonStyle())
                    .controlSize(.small)
                    .accessibilityIdentifier("\(identifier)-retry")
            }
        }
    }
}

/// Focus is an outline independent of a row's selection fill/edge and error ink.
private struct WorkFocusRing: ViewModifier {
    @Environment(\.isFocused) private var focused
    func body(content: Content) -> some View {
        content.keyboardFocusRing(focused, cornerRadius: 4)
    }
}

extension View {
    func workFocusRing() -> some View { modifier(WorkFocusRing()) }

    func workSelection(_ selected: Bool) -> some View {
        modifier(WorkSelection(selected: selected))
    }
}

private struct WorkSelection: ViewModifier {
    let selected: Bool
    @Environment(\.palette) private var palette

    func body(content: Content) -> some View {
        content
            .background(selected ? palette.selectionFill : .clear, in: RoundedRectangle(cornerRadius: 6))
            .overlay(alignment: .leading) {
                if selected {
                    RoundedRectangle(cornerRadius: 1).fill(palette.selectionEdge)
                        .frame(width: 2).padding(.vertical, 4)
                }
            }
    }
}

struct WorkPlainButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.workFocusRing()
    }
}

struct WorkLinkStyle: ButtonStyle {
    @Environment(\.palette) private var palette
    @Environment(\.isFocused) private var focused
    @State private var hovering = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(hovering || focused ? palette.linkHover : palette.linkInk)
            .underline(hovering || focused)
            .workFocusRing()
            .onHover { hovering = $0 }
    }
}

extension ProgramStatusState {
    var workTone: WorkTone {
        switch self {
        case .working: .running
        case .blocked: .waiting
        case .error: .failure
        case .done: .done
        case .idle, .clear: .neutral
        }
    }
}
