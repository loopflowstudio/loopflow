// The terminal region's colors. The surface is warm charcoal in the same hue
// family as the cream canvas, so the dark Session region reads as the page
// rather than a panel; the frame around it stays on `LoopflowPalette`.
// Ghostty's config string and every SwiftUI surface read the same hexes.

import AppKit
import Loopflow
import SwiftUI

enum TerminalPalette {
    static let backgroundHex: UInt = 0x24211F
    static let foregroundHex: UInt = 0xEDE7DF
    /// Cursor and prompt rose; also the selection accent.
    static let accentHex: UInt = 0xD9959D
    static let selectionHex: UInt = 0x4A443F

    static let background = Color(hex: backgroundHex)
    static let foreground = Color(hex: foregroundHex)
    static let accent = Color(hex: accentHex)
    /// Pane strip labels: ≥ 4.5:1 on the background.
    static let dim = Color(hex: 0xA39B93)
    /// The 1pt line between panes and under a pane strip.
    static let divider = Color(hex: 0xF0EBE4).opacity(0.12)
    /// Hover fill for strip glyphs.
    static let hover = Color(hex: 0xF0EBE4).opacity(0.08)

    static let nsBackground = NSColor(
        red: CGFloat((backgroundHex >> 16) & 0xFF) / 255,
        green: CGFloat((backgroundHex >> 8) & 0xFF) / 255,
        blue: CGFloat(backgroundHex & 0xFF) / 255,
        alpha: 1
    )

    /// Pane-strip state dots: `WorkTone`'s dark inks, fixed here because
    /// the strip is always dark whatever the window appearance.
    static func stateDot(_ tone: WorkTone) -> Color {
        switch tone {
        case .running: Color(hex: 0x86B0EA)
        case .done: Color(hex: 0x8DC79C)
        case .human: Color(hex: 0xE3B866)
        case .blocked: Color(hex: 0xF08B80)
        case .stopped: Color(hex: 0xB5ADA5)
        case .neutral: divider
        }
    }

    /// `#RRGGBB` for Ghostty's config file syntax.
    static func css(_ hex: UInt) -> String {
        String(format: "#%06X", hex)
    }
}
