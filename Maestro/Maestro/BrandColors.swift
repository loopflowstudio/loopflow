// Brand colors from loopflow.studio

import SwiftUI

extension Color {
    static let loopflowBurgundy = Color(hex: 0x722f37)
    static let loopflowBurgundyHover = Color(hex: 0x8b3d47)
    static let loopflowCream = Color(hex: 0xFAF8F5)
    static let loopflowText = Color(hex: 0x1a1a1a)
    static let loopflowTextSecondary = Color(hex: 0x6b6b6b)

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
