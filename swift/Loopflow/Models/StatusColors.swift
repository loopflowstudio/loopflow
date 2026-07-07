// Status colors synced with VISUAL_DESIGN.md.
// Warm, organic tones that pair with the burgundy/cream palette.

import SwiftUI

extension Color {
    public static let statusSuccess = Color(hex: 0x2D6A4F)
    public static let statusError = Color(hex: 0xB45309)
    public static let statusWarning = Color(hex: 0xB0812A)
    public static let statusInfo = Color(hex: 0x0AB3CC)
    public static let statusNeutral = Color(hex: 0x8B8B8B)

    public init(hex: UInt, alpha: Double = 1) {
        self.init(
            .sRGB,
            red: Double((hex >> 16) & 0xff) / 255,
            green: Double((hex >> 08) & 0xff) / 255,
            blue: Double((hex >> 00) & 0xff) / 255,
            opacity: alpha
        )
    }
}
