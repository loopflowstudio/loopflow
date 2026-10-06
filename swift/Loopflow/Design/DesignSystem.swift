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

/// Bundled faces (see `AppFontRegistration`): Cormorant Garamond Regular /
/// Medium / SemiBold, Lato Regular / Bold, JetBrains Mono Regular. Request only
/// those weights; `.semibold` or `.medium` on Lato and any weight on the mono
/// face are synthesized and render heavier than the design.
///
/// The workspace ramp has seven steps, serif on three of them and nothing
/// between 17 and 26 on purpose:
///
/// | step        | face          | size |
/// |-------------|---------------|------|
/// | display     | Cormorant 500 | 34   |
/// | title       | Cormorant 500 | 26   |
/// | lede        | Cormorant 500 | 17   |
/// | text        | Lato 400      | 13   |
/// | textStrong  | Lato 700      | 13   |
/// | meta        | Lato 400      | 11   |
/// | label       | Lato 700 caps | 11   |
/// | mono        | JetBrains     | 12   |
public enum Typography {
    public static let serifFamily = "Cormorant Garamond"
    public static let sansFamily = "Lato"
    public static let monoFamily = "JetBrains Mono"

    /// Wave page title.
    public static let display = sectionTitle(34)
    /// Task page title.
    public static let title = sectionTitle(26)
    /// Wave objective and other serif reading copy.
    public static let lede = sectionTitle(17)
    /// Everything else.
    public static let text = body(13)
    /// Row titles when selected, Session names.
    public static let textStrong = strong(13)
    /// Meta, ranks, IDs.
    public static let meta = caption(11)
    /// Section headings and table heads; callers add `.textCase(.uppercase)`
    /// and `.tracking(0.66)` (0.06em).
    public static let label = strong(11)
    /// Skill names, IDs, chips, terminal chrome.
    public static let mono = code(12)

    // Select bundled faces by name; family/weight matching can choose an
    // account-installed face that is unavailable inside the app sandbox.
    public static func heroTitle(_ size: CGFloat = 32) -> Font {
        .custom("CormorantGaramond-SemiBold", size: size)
    }

    public static func sectionTitle(_ size: CGFloat = 20) -> Font {
        .custom("CormorantGaramond-Medium", size: size)
    }

    public static func body(_ size: CGFloat = 14) -> Font {
        .custom(sansFamily, size: size)
    }

    public static func strong(_ size: CGFloat = 14) -> Font {
        .custom("Lato-Bold", size: size)
    }

    public static func caption(_ size: CGFloat = 12) -> Font {
        .custom(sansFamily, size: size)
    }

    public static func code(_ size: CGFloat = 13) -> Font {
        .custom(monoFamily, size: size)
    }
}

/// Motion tokens. Reduce-motion zeroes every one of them.
public enum DesignAnimation {
    /// Hover, chip and disclosure chevrons: 100 ms.
    public static func fast(_ reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .easeOut(duration: 0.1)
    }

    /// Panel open, row selection, pane focus dim: 160 ms.
    public static func standard(_ reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .easeInOut(duration: 0.16)
    }

    /// Sidebar collapse and split creation: 240 ms.
    public static func panel(_ reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .easeInOut(duration: 0.24)
    }

    /// Pane reorder only.
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
