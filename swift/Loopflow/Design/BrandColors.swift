import SwiftUI

public extension Color {
    static let loopflowBurgundy = Color(hex: 0x722F37)
    static let loopflowBurgundyHover = Color(hex: 0x5E2630)
    static let loopflowCream = Color(hex: 0xFAF8F5)
}

public struct ThemeTone: Sendable {
    public let ink: Color
    public let fill: Color
    public let stroke: Color
    public let text: Color
}

/// The sole semantic mapping for application and native pane chrome.
public struct LoopflowPalette: Sendable {
    public let isDark: Bool
    private let values: [ThemeRGB]
    public var background: Color { values[0].color }
    public var surface: Color { values[1].color }
    public var surfaceMuted: Color { values[2].color }
    public var surfaceElement: Color { values[3].color }
    public var border: Color { values[4].color }
    public var borderStrong: Color { values[5].color }
    public var borderActive: Color { values[6].color }
    public var text: Color { values[7].color }
    public var textSecondary: Color { values[8].color }
    public var textTertiary: Color { values[9].color }
    public var accent: Color { values[10].color }
    public var accentHover: Color { values[11].color }
    public let workingTone: ThemeTone
    public let waitingTone: ThemeTone
    public let failureTone: ThemeTone
    public let successTone: ThemeTone
    public let stoppedTone: ThemeTone
    public let emphasisInk: Color
    public var working: Color { workingTone.ink }
    public var waiting: Color { waitingTone.ink }
    public var failure: Color { failureTone.ink }
    public var success: Color { successTone.ink }
    public var selectionFill: Color { surface.opacity(0.72) }
    public var selectionEdge: Color { textSecondary }
    public var keyboardFocus: Color { emphasisInk }
    public var linkInk: Color { textSecondary }
    public var linkHover: Color { text }
    public var actionInk: Color { textSecondary }
    public var actionHover: Color { surfaceElement }
    public var terminalHover: Color { text.opacity(0.03) }
    public var terminalSelection: Color { text.opacity(0.06) }
    public var terminalFailure: Color { failure.opacity(0.06) }

    private init(values: [ThemeRGB], dark: Bool) {
        self.values = values
        isDark = dark
        let surfaces = Array(values.prefix(4))
        func tone(_ hex: UInt32) -> ThemeTone {
            let seed = ThemeRGB(hex)
            let fill = values[0].mixed(with: seed, amount: 0.08)
            let ink = seed.readable(on: surfaces + [fill], dark: dark)
            return ThemeTone(ink: ink.color, fill: fill.color, stroke: ink.color, text: ink.color)
        }
        workingTone = tone(0x3774BC)
        waitingTone = tone(0xA97922)
        failureTone = tone(0xBC493F)
        successTone = tone(0x438356)
        stoppedTone = tone(values[8].hex)
        emphasisInk = ThemeRGB(dark ? 0xEDB8BF : 0x722F37).readable(on: surfaces, dark: dark).color
    }

    public static func derived(background: ThemeRGB, foreground: ThemeRGB) -> Self {
        let dark = background.contrast(ThemeRGB(0xFFFFFF)) > background.contrast(ThemeRGB(0))
        let end = ThemeRGB(dark ? 0 : 0xFFFFFF)
        let surfaces = [background, background.mixed(with: end, amount: 0.18),
                        background.mixed(with: end, amount: 0.09), background.mixed(with: end, amount: 0.27)]
        let text = foreground.readable(on: surfaces, dark: dark)
        let secondary = foreground.mixed(with: background, amount: 0.28).readable(on: surfaces, dark: dark)
        let accent = ThemeRGB(dark ? 0xEDB8BF : 0x722F37).readable(on: surfaces, dark: dark)
        return Self(values: surfaces + [secondary, secondary, accent, text, secondary, secondary, accent, accent], dark: dark)
    }
    public static let light = Self(values: [0xFAF8F5, 0xFFFDF9, 0xF3EEE7, 0xECE6DD, 0xE6DFD6, 0xD6CCC0, 0xC2B5A6, 0x2A2624, 0x5F5752, 0x685F58, 0x722F37, 0x5E2630].map(ThemeRGB.init), dark: false)
    public static let dark = Self(values: [0x2B3036, 0x343B44, 0x3C4550, 0x444E59, 0x46505B, 0x56606B, 0x6B7683, 0xF5F1EA, 0xC8C1B8, 0xC7BFB7, 0x722F37, 0x5E2630].map(ThemeRGB.init), dark: true)
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
