import Loopflow
import SwiftUI

/// The one button: 24 pt high, `text`, radius `control`. Neutral by default;
/// one `.accent` per surface for what the surface is for; `.destructive` for an
/// action that ends the work.
struct LoopflowButtonStyle: ButtonStyle {
    enum Role { case neutral, accent, destructive }

    var role: Role = .neutral
    @Environment(\.palette) private var palette
    @Environment(\.isEnabled) private var isEnabled
    @State private var hovering = false

    init(_ role: Role = .neutral) { self.role = role }

    func makeBody(configuration: Configuration) -> some View {
        let shape = RoundedRectangle(cornerRadius: CornerRadius.control)
        return configuration.label
            .font(role == .accent ? Typography.textStrong : Typography.text)
            .foregroundStyle(ink)
            .lineLimit(1)
            .padding(.horizontal, Spacing.sm)
            .frame(minHeight: HitTarget.minimum)
            .background(shape.fill(fill(pressed: configuration.isPressed)))
            .overlay(shape.strokeBorder(stroke, lineWidth: 1))
            .contentShape(shape)
            .onHover { hovering = $0 }
            .workFocusRing()
    }

    private var ink: Color {
        guard isEnabled else { return palette.textTertiary }
        return switch role {
        case .neutral: palette.actionInk
        case .accent: palette.accentText
        case .destructive: palette.failure
        }
    }

    private var stroke: Color {
        guard isEnabled else { return palette.border }
        return switch role {
        case .neutral: palette.borderStrong
        case .accent: palette.accent
        case .destructive: palette.failure
        }
    }

    private func fill(pressed: Bool) -> Color {
        guard isEnabled else { return .clear }
        if role == .accent { return pressed ? palette.accentPressed : (hovering ? palette.accentHover : palette.accent) }
        return pressed ? palette.surfaceMuted : (hovering ? palette.surfaceElement : palette.surface)
    }
}
