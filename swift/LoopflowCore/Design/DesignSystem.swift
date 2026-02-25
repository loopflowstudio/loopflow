import SwiftUI

public enum Spacing {
    public static let xxs: CGFloat = 2
    public static let xs: CGFloat = 4
    public static let sm: CGFloat = 8
    public static let md: CGFloat = 12
    public static let lg: CGFloat = 16
    public static let xl: CGFloat = 20
    public static let xxl: CGFloat = 24
    public static let xxxl: CGFloat = 32
}

public enum HitTarget {
    public static let minimum: CGFloat = 24
    public static let comfortable: CGFloat = 32
    public static let touch: CGFloat = 44
}

public enum ZIndex {
    public static let base: Double = 0
    public static let dropdown: Double = 100
    public static let modal: Double = 200
    public static let toast: Double = 300
    public static let tooltip: Double = 400
}

public enum CornerRadius {
    public static let sm: CGFloat = 4
    public static let md: CGFloat = 8
    public static let lg: CGFloat = 12
    public static let xl: CGFloat = 16
    public static let full: CGFloat = 9999
}

public enum Typography {
    public static let serifFamily = "Cormorant Garamond"
    public static let sansFamily = "Lato"
    public static let monoFamily = "JetBrains Mono"

    public static func heroTitle(_ size: CGFloat = 32) -> Font {
        .custom(serifFamily, size: size).weight(.semibold)
    }

    public static func sectionTitle(_ size: CGFloat = 20) -> Font {
        .custom(serifFamily, size: size).weight(.medium)
    }

    public static func body(_ size: CGFloat = 14) -> Font {
        .custom(sansFamily, size: size)
    }

    public static func caption(_ size: CGFloat = 12) -> Font {
        .custom(sansFamily, size: size)
    }

    public static func code(_ size: CGFloat = 13) -> Font {
        .custom(monoFamily, size: size)
    }
}

public enum DesignAnimation {
    public static func standard(_ reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .easeInOut(duration: 0.2)
    }

    public static func fast(_ reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .easeOut(duration: 0.1)
    }

    public static func spring(_ reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .spring(response: 0.3, dampingFraction: 0.7)
    }
}

public extension View {
    func accessibleButton(_ label: String, hint: String? = nil) -> some View {
        self
            .accessibilityLabel(label)
            .accessibilityHint(hint ?? "")
            .accessibilityAddTraits(.isButton)
    }

    func accessibleToggle(_ label: String, isOn: Bool) -> some View {
        self
            .accessibilityLabel(label)
            .accessibilityValue(isOn ? "On" : "Off")
            .accessibilityAddTraits(.isButton)
    }

    func minHitTarget() -> some View {
        frame(minWidth: HitTarget.minimum, minHeight: HitTarget.minimum)
    }

    func keyboardFocusRing(_ isFocused: Bool, cornerRadius: CGFloat = CornerRadius.md) -> some View {
        overlay(
            RoundedRectangle(cornerRadius: cornerRadius)
                .stroke(Color.accentColor, lineWidth: 2)
                .opacity(isFocused ? 1 : 0)
        )
    }
}

public struct DarkButtonStyle: ButtonStyle {
    @Environment(\.palette) private var palette

    public init() {}

    public func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Typography.body())
            .foregroundStyle(Color.loopflowCream)
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.md)
                    .fill(configuration.isPressed ? palette.accentHover : palette.accent)
            )
    }
}

public struct GhostButtonStyle: ButtonStyle {
    @Environment(\.palette) private var palette

    public init() {}

    public func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Typography.body())
            .foregroundStyle(palette.accent)
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.md)
                    .fill(configuration.isPressed ? palette.surfaceMuted : Color.clear)
            )
    }
}

public struct DestructiveButtonStyle: ButtonStyle {
    public init() {}

    public func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Typography.body())
            .foregroundStyle(Color.statusError)
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.md)
                    .strokeBorder(Color.statusError.opacity(configuration.isPressed ? 0.8 : 0.5), lineWidth: 1)
            )
    }
}
