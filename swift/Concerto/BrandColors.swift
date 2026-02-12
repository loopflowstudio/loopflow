// Brand colors from loopflow.studio

import SwiftUI

extension Color {
    static let loopflowBurgundy = Color(hex: 0x722F37)
    static let loopflowBurgundyHover = Color(hex: 0x8B3D47)
    static let loopflowCream = Color(hex: 0xFAF8F5)
    static let loopflowCreamElevated = Color(hex: 0xFFFDFB)
    static let loopflowCreamMuted = Color(hex: 0xF3EEE7)
    static let loopflowSlate = Color(hex: 0x2B3036)
    static let loopflowSlateElevated = Color(hex: 0x343B44)
    static let loopflowSlateMuted = Color(hex: 0x3C4550)
    static let loopflowText = Color(hex: 0x1A1A1A)
    static let loopflowTextSecondary = Color(hex: 0x6B6B6B)
    static let loopflowTextLight = Color(hex: 0xF5F1EA)
    static let loopflowTextSecondaryLight = Color(hex: 0xC8C1B8)
    static let loopflowInfo = Color(hex: 0x0AB3CC)

    init(hex: UInt, alpha: Double = 1) {
        self.init(
            .sRGB,
            red: Double((hex >> 16) & 0xff) / 255,
            green: Double((hex >> 08) & 0xff) / 255,
            blue: Double((hex >> 00) & 0xff) / 255,
            opacity: alpha
        )
    }
}

// Status colors are defined in LoopflowCore/Models/StatusColors.swift

struct LoopflowPalette {
    let background: Color
    let surface: Color
    let surfaceMuted: Color
    let border: Color
    let text: Color
    let textSecondary: Color
    let accent: Color
    let accentHover: Color

    static let light = LoopflowPalette(
        background: .loopflowCream,
        surface: .loopflowCreamElevated,
        surfaceMuted: .loopflowCreamMuted,
        border: Color(hex: 0xE3DDD5),
        text: .loopflowText,
        textSecondary: .loopflowTextSecondary,
        accent: .loopflowBurgundy,
        accentHover: .loopflowBurgundyHover
    )

    static let dark = LoopflowPalette(
        background: .loopflowSlate,
        surface: .loopflowSlateElevated,
        surfaceMuted: .loopflowSlateMuted,
        border: Color(hex: 0x46505B),
        text: .loopflowTextLight,
        textSecondary: .loopflowTextSecondaryLight,
        accent: .loopflowBurgundy,
        accentHover: .loopflowBurgundyHover
    )

    static let deepWine = LoopflowPalette(
        background: Color(hex: 0x1E1215),
        surface: Color(hex: 0x2A1A20),
        surfaceMuted: Color(hex: 0x35222A),
        border: Color(hex: 0x4A3040),
        text: Color(hex: 0xF5EDE8),
        textSecondary: Color(hex: 0xC8B0A8),
        accent: Color(hex: 0x8B2252),
        accentHover: Color(hex: 0xA52D63)
    )

    static func make(for scheme: ColorScheme) -> LoopflowPalette {
        scheme == .dark ? .dark : .light
    }
}

// MARK: - Palette Environment Key

struct PaletteKey: EnvironmentKey {
    static let defaultValue = LoopflowPalette.light
}

extension EnvironmentValues {
    var palette: LoopflowPalette {
        get { self[PaletteKey.self] }
        set { self[PaletteKey.self] = newValue }
    }
}
