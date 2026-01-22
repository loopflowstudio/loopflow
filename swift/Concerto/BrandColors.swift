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

// MARK: - Status Colors (synced with VISUAL_DESIGN.md)

extension Color {
    static let statusSuccess = Color(hex: 0x2D6A4F)
    static let statusError = Color(hex: 0xB45309)
    static let statusWarning = Color(hex: 0xB0812A)
    static let statusInfo = Color.loopflowInfo
}

struct LoopflowPalette {
    let background: Color
    let surface: Color
    let surfaceMuted: Color
    let border: Color
    let text: Color
    let textSecondary: Color
    let accent: Color
    let accentHover: Color

    static func make(for scheme: ColorScheme) -> LoopflowPalette {
        switch scheme {
        case .dark:
            return LoopflowPalette(
                background: .loopflowSlate,
                surface: .loopflowSlateElevated,
                surfaceMuted: .loopflowSlateMuted,
                border: Color(hex: 0x46505B),
                text: .loopflowTextLight,
                textSecondary: .loopflowTextSecondaryLight,
                accent: .loopflowBurgundy,
                accentHover: .loopflowBurgundyHover
            )
        default:
            return LoopflowPalette(
                background: .loopflowCream,
                surface: .loopflowCreamElevated,
                surfaceMuted: .loopflowCreamMuted,
                border: Color(hex: 0xE3DDD5),
                text: .loopflowText,
                textSecondary: .loopflowTextSecondary,
                accent: .loopflowBurgundy,
                accentHover: .loopflowBurgundyHover
            )
        }
    }
}
