// Brand colors from loopflow.studio

import SwiftUI

extension Color {
    static let loopflowBurgundy = Color(hex: 0x722F37)
    static let loopflowBurgundyHover = Color(hex: 0x8B3D47)
    static let loopflowCream = Color(hex: 0xFAF8F5)
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
        background: Color(hex: 0xFAF8F5),
        surface: Color(hex: 0xFFFDFB),
        surfaceMuted: Color(hex: 0xF3EEE7),
        border: Color(hex: 0xE3DDD5),
        text: Color(hex: 0x1A1A1A),
        textSecondary: Color(hex: 0x6B6B6B),
        accent: .loopflowBurgundy,
        accentHover: .loopflowBurgundyHover
    )

    static let dark = LoopflowPalette(
        background: Color(hex: 0x2B3036),
        surface: Color(hex: 0x343B44),
        surfaceMuted: Color(hex: 0x3C4550),
        border: Color(hex: 0x46505B),
        text: Color(hex: 0xF5F1EA),
        textSecondary: Color(hex: 0xC8C1B8),
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
