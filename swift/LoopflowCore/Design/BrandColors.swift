import SwiftUI

public extension Color {
    static let loopflowBurgundy = Color(hex: 0x722F37)
    static let loopflowBurgundyHover = Color(hex: 0x8B3D47)
    static let loopflowCream = Color(hex: 0xFAF8F5)
}

public struct LoopflowPalette: Sendable {
    public let background: Color
    public let surface: Color
    public let surfaceMuted: Color
    public let border: Color
    public let text: Color
    public let textSecondary: Color
    public let accent: Color
    public let accentHover: Color

    public init(
        background: Color,
        surface: Color,
        surfaceMuted: Color,
        border: Color,
        text: Color,
        textSecondary: Color,
        accent: Color,
        accentHover: Color
    ) {
        self.background = background
        self.surface = surface
        self.surfaceMuted = surfaceMuted
        self.border = border
        self.text = text
        self.textSecondary = textSecondary
        self.accent = accent
        self.accentHover = accentHover
    }

    public static let light = LoopflowPalette(
        background: Color(hex: 0xFAF8F5),
        surface: Color(hex: 0xFFFDFB),
        surfaceMuted: Color(hex: 0xF3EEE7),
        border: Color(hex: 0xE3DDD5),
        text: Color(hex: 0x1A1A1A),
        textSecondary: Color(hex: 0x6B6B6B),
        accent: .loopflowBurgundy,
        accentHover: .loopflowBurgundyHover
    )

    public static let dark = LoopflowPalette(
        background: Color(hex: 0x2B3036),
        surface: Color(hex: 0x343B44),
        surfaceMuted: Color(hex: 0x3C4550),
        border: Color(hex: 0x46505B),
        text: Color(hex: 0xF5F1EA),
        textSecondary: Color(hex: 0xC8C1B8),
        accent: .loopflowBurgundy,
        accentHover: .loopflowBurgundyHover
    )

    public static let deepWine = LoopflowPalette(
        background: Color(hex: 0x1E1215),
        surface: Color(hex: 0x2A1A20),
        surfaceMuted: Color(hex: 0x35222A),
        border: Color(hex: 0x4A3040),
        text: Color(hex: 0xF5EDE8),
        textSecondary: Color(hex: 0xC8B0A8),
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
