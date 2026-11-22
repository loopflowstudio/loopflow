import SwiftUI

/// The one button: as tall as the platform's control (24 pt on the Mac, 44 pt
/// on iPhone), `text`, radius `control`. Neutral by default; one `.accent` per
/// surface for what the surface is for; `.destructive` for an action that ends
/// the work. On iPhone the accent button fills the width it is offered.
public struct LoopflowButtonStyle: ButtonStyle {
    public enum Role: Sendable { case neutral, accent, destructive }

    var role: Role = .neutral
    @Environment(\.palette) private var palette
    @Environment(\.isEnabled) private var isEnabled
    @Environment(\.isFocused) private var focused
    @State private var hovering = false

    public init(_ role: Role = .neutral) { self.role = role }

    #if os(iOS)
    private static let accentFills = true
    #else
    private static let accentFills = false
    #endif

    public func makeBody(configuration: Configuration) -> some View {
        let shape = RoundedRectangle(cornerRadius: CornerRadius.control)
        return configuration.label
            .font(role == .accent ? Typography.textStrong : Typography.text)
            .foregroundStyle(ink)
            .lineLimit(1)
            // A third of the height: 8 pt on the Mac.
            .padding(.horizontal, HitTarget.control / 3)
            .frame(maxWidth: role == .accent && Self.accentFills ? .infinity : nil, minHeight: HitTarget.control)
            .background(shape.fill(fill(pressed: configuration.isPressed)))
            .overlay(shape.strokeBorder(stroke, lineWidth: 1))
            .contentShape(shape)
            .onHover { hovering = $0 }
            .keyboardFocusRing(focused, cornerRadius: CornerRadius.control)
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
