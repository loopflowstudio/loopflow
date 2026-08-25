// The Podium's one instrument: a vertical LED fader that is simultaneously the
// state lens (fill color) and the on/off switch (press spins the agent up or
// down). There is
// no separate lamp dot and no adjacent lifecycle button — the rail is the
// whole vocabulary, at every altitude from the Podium bar down to a task row.

import Loopflow
import SwiftUI

/// The four operating states the fader can show. Blocked and needs-attention
/// are collapsed into `.waiting` for now; the blockings Ask queue will refine
/// that split when it lands.
enum FaderPhase: Equatable {
    case off
    case starting
    case producing
    case waiting

    var color: Color {
        switch self {
        case .off: Color(hex: 0x3A3A3A)
        case .starting: WaveLensColor.unknown.glow
        case .producing: WaveLensColor.green.glow
        case .waiting: WaveLensColor.red.glow
        }
    }

    var label: String {
        switch self {
        case .off: "Off"
        case .starting: "Starting"
        case .producing: "Producing"
        case .waiting: "Waiting on you"
        }
    }

    /// The one press verb the phase owns, or nil when the press would have no
    /// legal move (the caller may still withhold the action entirely).
    var verb: String? {
        switch self {
        case .off: "Start"
        case .starting, .producing: "Stop"
        case .waiting: "Resolve"
        }
    }
}

/// Collapse exact process activity and the human-attention channel into one phase.
/// A human-owed stop wins over sibling activity: one red task turns its wave's
/// whole column red. `agentRunning` distinguishes a spun-up-but-quiet agent
/// (starting) from no agent at all (off).
enum ConsoleSignal {
    static func phase(
        humanStop: Bool,
        agentRunning: Bool,
        signal: PodiumSignalState
    ) -> FaderPhase {
        if humanStop { return .waiting }
        switch signal {
        case .producing: return .producing
        case .blocked: return .waiting
        case .waiting: return .starting
        case .off, .unknown: return agentRunning ? .starting : .off
        }
    }

    /// How a press starts Task Work, per the server's recommended move.
    /// Nil means the fader is display-only at rest (completed Task, or a Task
    /// whose only move is not a lifecycle start).
    enum TaskStart: Equatable {
        case run
        case resume
    }

    static func taskStart(_ task: RoadmapTask) -> TaskStart? {
        switch roadmapTaskAction(task) {
        case .run: .run
        case .resume: .resume
        case .openPr, .none: nil
        }
    }
}

struct FaderSwitch: View {
    let phase: FaderPhase
    var width: CGFloat = 15
    var height: CGFloat = 34
    /// Hover/help naming for the press; nil renders a display-only state rail.
    var verb: String?
    var isBusy: Bool = false
    let accessibilityId: String
    let accessibilityLabel: String
    var action: (() -> Void)?

    var body: some View {
        Group {
            if let action, verb != nil, !isBusy {
                Button(action: action) { rail }
                    .buttonStyle(.plain)
            } else {
                rail
            }
        }
        .overlay {
            if isBusy {
                ProgressView().controlSize(.mini)
            }
        }
        .help(helpText)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(accessibilityLabel)
        .accessibilityValue(accessibilityValue)
        .accessibilityHint(verb ?? "")
        .accessibilityIdentifier(accessibilityId)
    }

    private var rail: some View {
        Canvas { context, size in
            let rail = CGRect(origin: .zero, size: size)
            let radius = size.width / 2
            context.fill(
                Path(roundedRect: rail, cornerRadius: radius),
                with: .color(Color.black.opacity(0.34))
            )
            context.stroke(
                Path(roundedRect: rail.insetBy(dx: 0.5, dy: 0.5), cornerRadius: radius),
                with: .color(ringColor),
                lineWidth: 1
            )

            let inset = rail.insetBy(dx: 3, dy: 3)
            let fillHeight = fillHeight(available: inset.height)
            if fillHeight > 0 {
                drawLadder(
                    context: context,
                    inset: inset,
                    fillHeight: fillHeight
                )
            }

            if phase == .off, size.width >= 14 {
                let glyph = context.resolve(
                    Text(Image(systemName: "power"))
                        .font(.system(size: size.width * 0.5, weight: .semibold))
                        .foregroundStyle(Color.white.opacity(0.5))
                )
                context.draw(glyph, at: CGPoint(x: rail.midX, y: rail.midY))
            }
        }
        .frame(width: width, height: height)
        .shadow(color: glowColor, radius: 3)
        .contentShape(Rectangle())
    }

    /// Live phases fill the state rail; off keeps it empty.
    private func fillHeight(available: CGFloat) -> CGFloat {
        guard phase != .off else { return 0 }
        return available
    }

    private func drawLadder(context: GraphicsContext, inset: CGRect, fillHeight: CGFloat) {
        let color = phase.color
        if width < 14 {
            // Pip size: one continuous sliver — precision is the first casualty.
            let fill = CGRect(
                x: inset.minX, y: inset.maxY - fillHeight,
                width: inset.width, height: fillHeight
            )
            context.fill(
                Path(roundedRect: fill, cornerRadius: inset.width / 2),
                with: .color(color)
            )
            return
        }
        let led: CGFloat = 3
        var y = inset.maxY
        while y - led >= inset.maxY - fillHeight {
            let segment = CGRect(x: inset.minX, y: y - led, width: inset.width, height: led)
            let distanceUp = (inset.maxY - y + led) / max(inset.height, 1)
            context.fill(
                Path(roundedRect: segment, cornerRadius: 1),
                with: .color(color.opacity(1 - 0.4 * distanceUp))
            )
            y -= ledStep
        }
    }

    private var ledStep: CGFloat { width < 14 ? 4 : 5 }

    private var ringColor: Color {
        phase == .off ? Color.white.opacity(0.28) : phase.color.opacity(0.8)
    }

    private var glowColor: Color {
        phase == .off ? .clear : phase.color.opacity(0.6)
    }

    private var helpText: String {
        guard let verb else { return phase.label }
        return "\(verb) — \(phase.label)"
    }

    private var accessibilityValue: String {
        phase.label
    }
}
