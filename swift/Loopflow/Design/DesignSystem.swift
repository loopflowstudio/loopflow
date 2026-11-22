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
    /// A button's or a row's height on this platform.
    #if os(iOS)
    public static let control: CGFloat = touch
    #else
    public static let control: CGFloat = minimum
    #endif
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
/// The ramp (DESIGN.md), serif on two steps and applied only with `.serif(_:)`.
/// The step names are the same on each platform; iPhone steps follow Dynamic Type.
///
/// | step        | face                | Mac | iPhone |
/// |-------------|---------------------|-----|--------|
/// | display     | Old Standard        | 34  | 34     |
/// | title       | Old Standard        | 26  | 26     |
/// | lede        | Atkinson 600        | 17  | 20     |
/// | text        | Atkinson 400        | 13  | 17     |
/// | textStrong  | Atkinson 600        | 13  | 17     |
/// | meta        | Atkinson 400        | 11  | 13     |
/// | mono        | Spline Sans Mono    | 12  | 14     |
/// | terminal    | Spline Sans Mono    | 13  | 12     |
public enum Typography {
    public static let serifFamily = "Old Standard TT"
    public static let sansFamily = "Atkinson Hyperlegible Next"
    public static let monoFamily = "Spline Sans Mono"

    // Select bundled faces by PostScript name; family/weight matching can choose
    // an account-installed face that is unavailable inside the app sandbox.
    // AppKit and UIKit fields name them too.
    public static let serifFace = "OldStandardTT-Regular"
    public static let sansFace = "AtkinsonHyperlegibleNext-Regular"
    public static let sansStrongFace = "AtkinsonHyperlegibleNext-SemiBold"
    public static let monoFace = "SplineSansMono-Regular"

    /// A document's first heading; the text typed in New Task.
    public static let lede = step(sansStrongFace, mac: 17, iPhone: 20, .title3)
    /// Everything else.
    public static let text = step(sansFace, mac: 13, iPhone: 17, .body)
    /// The selected row's title, a pane's name.
    public static let textStrong = step(sansStrongFace, mac: 13, iPhone: 17, .body)
    /// Counts, times, secondary facts.
    public static let meta = step(sansFace, mac: 11, iPhone: 13, .footnote)
    /// Inline IDs, paths, commands.
    public static let mono = step(monoFace, mac: 12, iPhone: 14, .subheadline)
    /// Ghostty's `font-size` default; the person's configuration replaces it on the Mac.
    #if os(iOS)
    public static let terminalSize: CGFloat = 12
    #else
    public static let terminalSize: CGFloat = 13
    #endif

    /// A sans or mono step: fixed on the Mac, following Dynamic Type's `style` on iPhone.
    private static func step(_ face: String, mac: CGFloat, iPhone: CGFloat, _ style: Font.TextStyle) -> Font {
        #if os(iOS)
        .custom(face, size: iPhone, relativeTo: style)
        #else
        .custom(face, fixedSize: mac)
        #endif
    }
}

/// The two serif steps. They are not `Font` values: serif is set only through
/// `.serif(_:)`, which applies the face with common ligatures off ("fl" stays two
/// letters in "Loopflow") and its −3% tracking together.
public enum SerifStep: Sendable {
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

    var textStyle: Font.TextStyle {
        switch self {
        case .display: .largeTitle
        case .title: .title
        }
    }
}

public extension View {
    /// The serif face at a ramp step, ligatures off and tracked −3%.
    func serif(_ step: SerifStep) -> some View {
        modifier(Serif(step))
    }
}

private struct Serif: ViewModifier {
    let step: SerifStep
    /// Dynamic Type's factor for the step; 1 on the Mac, which has none.
    @ScaledMetric private var scale: CGFloat

    init(_ step: SerifStep) {
        self.step = step
        _scale = ScaledMetric(wrappedValue: 1, relativeTo: step.textStyle)
    }

    func body(content: Content) -> some View {
        let size = step.size * scale
        let descriptor = CTFontDescriptorCreateCopyWithFeature(
            CTFontDescriptorCreateWithNameAndSize(Typography.serifFace as CFString, size),
            NSNumber(value: kLigaturesType),
            NSNumber(value: kCommonLigaturesOffSelector)
        )
        content.font(Font(CTFontCreateWithFontDescriptor(descriptor, size, nil))).tracking(-0.03 * size)
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

