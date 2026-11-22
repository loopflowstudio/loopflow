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
                .font(Typography.textStrong)
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

extension TaskExecutionState {
    /// Shared by Graph status, plan chips and navigation dots. Quiet states have no chip.
    var presentation: (label: String?, tone: WorkTone) {
        switch self {
        case .running, .starting: ("Running", .running)
        case .blocked: ("Blocked", .warning)
        case .stalled: ("Stalled", .warning)
        case .waiting: ("Waiting", .stopped)
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
            .font(Typography.textStrong)
            .tracking(0.2)
            .foregroundStyle(tone == .neutral ? tone.ink(in: palette) : tone.text(in: palette))
            .lineLimit(1)
            .padding(.horizontal, 7)
            .padding(.vertical, 1)
            .background(tone.fill(in: palette), in: RoundedRectangle(cornerRadius: CornerRadius.control))
            .overlay {
                if tone == .neutral {
                    RoundedRectangle(cornerRadius: CornerRadius.control).strokeBorder(tone.stroke(in: palette), lineWidth: 1)
                }
            }
    }
}

extension View {
    /// A framed surface one ladder step above the canvas, with a hairline and
    /// no shadow: the only framed surfaces are Graph, Sessions, and plans.
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
            .background(palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.pane))
            .overlay(RoundedRectangle(cornerRadius: CornerRadius.pane).strokeBorder(palette.border))
    }
}

/// Ordinary actions stay neutral; a deliberately emphasized action uses the theme accent.
struct WorkOutlineButtonStyle: ButtonStyle {
    var emphasized = false
    /// Red, for an action that ends the work.
    var destructive = false
    @Environment(\.palette) private var palette
    @Environment(\.isEnabled) private var isEnabled

    @State private var hovering = false

    private var ink: Color { destructive ? palette.failure : (emphasized ? palette.accentInk : palette.actionInk) }

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Typography.text)
            .foregroundStyle(isEnabled ? ink : palette.textTertiary)
            .padding(.horizontal, 11)
            .padding(.vertical, 4)
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.control)
                    .fill(configuration.isPressed ? palette.surfaceElement : (hovering ? palette.surfaceMuted : palette.surface))
            )
            .overlay(
                RoundedRectangle(cornerRadius: CornerRadius.control)
                    .strokeBorder(isEnabled && (emphasized || destructive) ? ink : palette.borderStrong, lineWidth: 1)
            )
            .contentShape(RoundedRectangle(cornerRadius: CornerRadius.control))
            .onHover { hovering = $0 }
            .workFocusRing()
    }
}

/// Marks a Task whose auto setting is on, on its board card and sidebar row.
struct AutoBadge: View {
    var body: some View {
        WorkChip(text: "Auto", tone: .done)
            .help("Auto is on: the run takes the interactive steps and lands the Task")
            .accessibilityLabel("Auto on")
    }
}

/// Collapsed heading for a demand-read Task section (Comments, Session history):
/// chevron, caps title, optional count, and the read's progress.
struct WorkDisclosureHeading: View {
    let title: String
    let count: Int?
    let expanded: Bool
    let inFlight: Bool
    let identifier: String
    let accessibilityLabel: String
    let help: String
    let toggle: () -> Void
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.sm) {
            Button(action: toggle) {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                    Image(systemName: "chevron.right")
                        .font(Typography.meta).fontWeight(.bold)
                        .rotationEffect(.degrees(expanded ? 90 : 0))
                    Text(title)
                        .font(Typography.textStrong)
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
            }
        }
    }
}

/// Focus is an outline independent of a row's selection fill/edge and error ink.
private struct WorkFocusRing: ViewModifier {
    @Environment(\.isFocused) private var focused
    func body(content: Content) -> some View {
        content.keyboardFocusRing(focused, cornerRadius: CornerRadius.control)
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
            .background(selected ? palette.selectionFill : .clear, in: RoundedRectangle(cornerRadius: CornerRadius.row))
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
        case .blocked: .warning
        case .error: .failure
        case .done: .done
        case .idle: .available
        case .clear: .neutral
        }
    }
}

extension View {
    func workDestinationSelection(_ selected: Bool) -> some View {
        modifier(WorkDestinationSelection(selected: selected))
    }
}

private struct WorkDestinationSelection: ViewModifier {
    let selected: Bool
    @Environment(\.palette) private var palette
    @State private var hovering = false
    func body(content: Content) -> some View {
        content
            .background(selected ? palette.workSelectedFill : (hovering ? palette.actionHover : .clear),
                        in: RoundedRectangle(cornerRadius: CornerRadius.row))
            .overlay(RoundedRectangle(cornerRadius: CornerRadius.row).strokeBorder(selected ? palette.selectionEdge : .clear))
            .onHover { hovering = $0 }
            .accessibilityAddTraits(selected ? [.isSelected] : [])
    }
}
