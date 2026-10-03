import SwiftUI

public extension Color {
    static let loopflowBurgundy = Color(hex: 0x722F37)
    static let loopflowBurgundyHover = Color(hex: 0x5E2630)
    static let loopflowCream = Color(hex: 0xFAF8F5)
}

/// One warm ramp per appearance. Surfaces climb a ladder (background →
/// surfaceMuted → surfaceElement → surface) joined by hairlines; depth never
/// comes from shadows. Neutrals keep low chroma so burgundy stays the only
/// accent, and blue is reserved for running work and loop regions.
public struct LoopflowPalette: Sendable {
    /// Canvas behind every page.
    public let background: Color
    /// Cards, plan panels and inputs: the lightest step.
    public let surface: Color
    /// Sidebar, toolbar and notice base: one step below the canvas.
    public let surfaceMuted: Color
    /// Hover, pressed and inline code: one step below the sidebar.
    public let surfaceElement: Color
    /// Hairlines between sections and around panels.
    public let border: Color
    /// Chip and input outlines.
    public let borderStrong: Color
    /// Focused input and hover outline.
    public let borderActive: Color
    public let text: Color
    public let textSecondary: Color
    /// Quiet labels, ranks and IDs; ≥ 4.5:1 on every surface step, including hover.
    public let textTertiary: Color
    public let accent: Color
    public let accentHover: Color

    public init(
        background: Color,
        surface: Color,
        surfaceMuted: Color,
        surfaceElement: Color,
        border: Color,
        borderStrong: Color,
        borderActive: Color,
        text: Color,
        textSecondary: Color,
        textTertiary: Color,
        accent: Color,
        accentHover: Color
    ) {
        self.background = background
        self.surface = surface
        self.surfaceMuted = surfaceMuted
        self.surfaceElement = surfaceElement
        self.border = border
        self.borderStrong = borderStrong
        self.borderActive = borderActive
        self.text = text
        self.textSecondary = textSecondary
        self.textTertiary = textTertiary
        self.accent = accent
        self.accentHover = accentHover
    }

    public static let light = LoopflowPalette(
        background: Color(hex: 0xFAF8F5),
        surface: Color(hex: 0xFFFDF9),
        surfaceMuted: Color(hex: 0xF3EEE7),
        surfaceElement: Color(hex: 0xECE6DD),
        border: Color(hex: 0xE6DFD6),
        borderStrong: Color(hex: 0xD6CCC0),
        borderActive: Color(hex: 0xC2B5A6),
        text: Color(hex: 0x2A2624),
        textSecondary: Color(hex: 0x5F5752),
        textTertiary: Color(hex: 0x6F665F),
        accent: .loopflowBurgundy,
        accentHover: .loopflowBurgundyHover
    )

    // Dark palettes compile against the same shape; they are not yet designed.
    public static let dark = LoopflowPalette(
        background: Color(hex: 0x2B3036),
        surface: Color(hex: 0x343B44),
        surfaceMuted: Color(hex: 0x3C4550),
        surfaceElement: Color(hex: 0x444E59),
        border: Color(hex: 0x46505B),
        borderStrong: Color(hex: 0x56606B),
        borderActive: Color(hex: 0x6B7683),
        text: Color(hex: 0xF5F1EA),
        textSecondary: Color(hex: 0xC8C1B8),
        textTertiary: Color(hex: 0xA39B93),
        accent: .loopflowBurgundy,
        accentHover: .loopflowBurgundyHover
    )

    public static let deepWine = LoopflowPalette(
        background: Color(hex: 0x1E1215),
        surface: Color(hex: 0x2A1A20),
        surfaceMuted: Color(hex: 0x35222A),
        surfaceElement: Color(hex: 0x402A33),
        border: Color(hex: 0x4A3040),
        borderStrong: Color(hex: 0x5A3C4C),
        borderActive: Color(hex: 0x6E4A5C),
        text: Color(hex: 0xF5EDE8),
        textSecondary: Color(hex: 0xC8B0A8),
        textTertiary: Color(hex: 0xA8908A),
        accent: Color(hex: 0x8B2252),
        accentHover: Color(hex: 0xA52D63)
    )
}

public struct PaletteKey: EnvironmentKey {
    public static let defaultValue = LoopflowPalette.light
}

public extension EnvironmentValues {
    var palette: LoopflowPalette {
        get { self[PaletteKey.self] }
        set { self[PaletteKey.self] = newValue }
    }
}
