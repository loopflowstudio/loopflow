import SwiftUI
import CoreText

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
    /// Buttons, fields, chips.
    public static let control: CGFloat = 4
    /// Rows and their selection.
    public static let row: CGFloat = 8
    /// Panes and cards.
    public static let pane: CGFloat = 12
    /// Popovers, palettes, sheets.
    public static let floating: CGFloat = 16
}

/// Bundled faces (see `AppFontRegistration`): Old Standard TT Regular, Atkinson
/// Hyperlegible Next Regular / Italic / SemiBold / SemiBold Italic, Spline Sans
/// Mono Regular / Bold / Italic / Bold Italic. Request only those; any other
/// weight is synthesized by CoreText and renders heavier than the design.
///
/// The ramp (DESIGN.md), serif on two steps and applied only with `.serif(_:)`:
///
/// | step        | face                | size |
/// |-------------|---------------------|------|
/// | display     | Old Standard        | 34   |
/// | title       | Old Standard        | 26   |
/// | lede        | Atkinson 600        | 17   |
/// | text        | Atkinson 400        | 13   |
/// | textStrong  | Atkinson 600        | 13   |
/// | meta        | Atkinson 400        | 11   |
/// | mono        | Spline Sans Mono    | 12   |
/// | terminal    | Spline Sans Mono    | 13   |
public enum Typography {
    public static let serifFamily = "Old Standard TT"
    public static let sansFamily = "Atkinson Hyperlegible Next"
    public static let monoFamily = "Spline Sans Mono"

    // Select bundled faces by PostScript name; family/weight matching can choose
    // an account-installed face that is unavailable inside the app sandbox.
    // iPhone and AppKit fields name them too.
    public static let serifFace = "OldStandardTT-Regular"
    public static let sansFace = "AtkinsonHyperlegibleNext-Regular"
    public static let sansStrongFace = "AtkinsonHyperlegibleNext-SemiBold"
    public static let monoFace = "SplineSansMono-Regular"

    /// A document's first heading; the text typed in New Task.
    public static let lede = Font.custom(sansStrongFace, fixedSize: 17)
    /// Everything else.
    public static let text = Font.custom(sansFace, fixedSize: 13)
    /// The selected row's title, a pane's name.
    public static let textStrong = Font.custom(sansStrongFace, fixedSize: 13)
    /// Counts, times, secondary facts.
    public static let meta = Font.custom(sansFace, fixedSize: 11)
    /// Inline IDs, paths, commands.
    public static let mono = Font.custom(monoFace, fixedSize: 12)
    /// Ghostty's `font-size` default; the person's configuration replaces it.
    public static let terminalSize: CGFloat = 13
}

/// The two serif steps. They are not `Font` values: serif is set only through
/// `.serif(_:)`, which applies the face with common ligatures off ("fl" stays two
/// letters in "Loopflow") and its −3% tracking together.
public enum SerifStep: CaseIterable, Sendable {
    /// Wave name, "What do you want?".
    case display
    /// Task title in the Brief, dashboard name, state titles.
    case title

    public var size: CGFloat {
        switch self {
        case .display: 34
        case .title: 26
        }
    }

    var font: Font {
        let descriptor = CTFontDescriptorCreateCopyWithFeature(
            CTFontDescriptorCreateWithNameAndSize(Typography.serifFace as CFString, size),
            NSNumber(value: kLigaturesType),
            NSNumber(value: kCommonLigaturesOffSelector)
        )
        return Font(CTFontCreateWithFontDescriptor(descriptor, size, nil))
    }
}

public extension View {
    /// The serif face at a ramp step, ligatures off and tracked −3%.
    func serif(_ step: SerifStep) -> some View {
        font(step.font).tracking(-0.03 * step.size)
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

    func keyboardFocusRing(_ isFocused: Bool, cornerRadius: CGFloat = CornerRadius.row) -> some View {
        modifier(KeyboardFocusRing(isFocused: isFocused, cornerRadius: cornerRadius))
    }
}

private struct KeyboardFocusRing: ViewModifier {
    let isFocused: Bool
    let cornerRadius: CGFloat
    @Environment(\.palette) private var palette

    func body(content: Content) -> some View {
        content.overlay {
            RoundedRectangle(cornerRadius: cornerRadius)
                .strokeBorder(palette.keyboardFocus, lineWidth: 2)
                .padding(-3)
                .opacity(isFocused ? 1 : 0)
                .allowsHitTesting(false)
        }
    }
}

public struct DarkButtonStyle: ButtonStyle {
    @Environment(\.isFocused) private var focused
    @Environment(\.palette) private var palette

    @State private var hovering = false

    public init() {}

    public func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Typography.text)
            .foregroundStyle(palette.accentText)
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.row)
                    .fill(configuration.isPressed ? palette.accentPressed : (hovering ? palette.accentHover : palette.accent))
            )
            .onHover { hovering = $0 }
            .keyboardFocusRing(focused)
    }
}

public struct GhostButtonStyle: ButtonStyle {
    @Environment(\.isFocused) private var focused
    @Environment(\.palette) private var palette

    public init() {}

    public func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Typography.text)
            .foregroundStyle(palette.actionInk)
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.row)
                    .fill(configuration.isPressed ? palette.surfaceMuted : Color.clear)
            )
            .keyboardFocusRing(focused)
    }
}

public struct DestructiveButtonStyle: ButtonStyle {
    @Environment(\.palette) private var palette
    @Environment(\.isFocused) private var focused
    public init() {}

    public func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Typography.text)
            .foregroundStyle(palette.failure)
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.row)
                    .strokeBorder(palette.failure.opacity(configuration.isPressed ? 0.8 : 0.5), lineWidth: 1)
            )
            .keyboardFocusRing(focused)
    }
}
