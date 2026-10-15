import SwiftUI

public extension Color {
    static let loopflowBurgundy = Color(hex: 0x722F37)
    static let loopflowBurgundyHover = Color(hex: 0x5E2630)
    static let loopflowCream = Color(hex: 0xFAF8F5)
}

public struct ThemeTone: Sendable {
    let inkRGB: ThemeRGB
    let fillRGB: ThemeRGB
    public var ink: Color { inkRGB.color }
    public var fill: Color { fillRGB.color }
    public var stroke: Color { ink }
    public var text: Color { ink }
}

/// The sole semantic mapping for application and native pane chrome.
public struct LoopflowPalette: Sendable {
    public let isDark: Bool
    public let background: Color
    public let surface: Color
    public let surfaceMuted: Color
    public let surfaceElement: Color
    public let border: Color
    public let borderStrong: Color
    public let borderActive: Color
    public let text: Color
    public let textSecondary: Color
    public let textTertiary: Color
    public let accent: Color
    public let accentHover: Color
    public let workingTone: ThemeTone
    public let waitingTone: ThemeTone
    public let failureTone: ThemeTone
    public let successTone: ThemeTone
    public let stoppedTone: ThemeTone
    public let emphasisInk: Color
    public let workSelectedFill: Color
    public let workSelectedText: Color
    public let workSelectedSecondaryText: Color
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
    public let terminalHover: Color
    public let terminalSelection: Color
    public let terminalFailure: Color

    private init(
        background: ThemeRGB,
        surface: ThemeRGB,
        surfaceMuted: ThemeRGB,
        surfaceElement: ThemeRGB,
        border: ThemeRGB,
        borderStrong: ThemeRGB,
        borderActive: ThemeRGB,
        text: ThemeRGB,
        textSecondary: ThemeRGB,
        textTertiary: ThemeRGB,
        accent: ThemeRGB,
        accentHover: ThemeRGB,
        dark: Bool, terminalForeground: ThemeRGB? = nil
    ) {
        self.background = background.color
        self.surface = surface.color
        self.surfaceMuted = surfaceMuted.color
        self.surfaceElement = surfaceElement.color
        self.border = border.color
        self.borderStrong = borderStrong.color
        self.borderActive = borderActive.color
        self.text = text.color
        self.textSecondary = textSecondary.color
        self.textTertiary = textTertiary.color
        self.accent = accent.color
        self.accentHover = accentHover.color

        isDark = dark
        let surfaces = [background, surface, surfaceMuted, surfaceElement]
        func tone(_ hex: UInt32) -> ThemeTone {
            let seed = ThemeRGB(hex)
            let endpoint = ThemeRGB(dark ? 0xFFFFFF : 0)
            var fill = background
            for step in stride(from: 8, through: 0, by: -1) {
                let candidate = background.mixed(with: seed, amount: Double(step) / 100)
                if endpoint.contrast(candidate) >= 4.52 { fill = candidate; break }
            }
            let ink = seed.readable(on: surfaces + [fill], dark: dark)
            return ThemeTone(inkRGB: ink, fillRGB: fill)
        }
        workingTone = tone(0x3774BC)
        waitingTone = tone(0xA97922)
        failureTone = tone(0xBC493F)
        successTone = tone(0x438356)
        stoppedTone = tone(textSecondary.hex)
        emphasisInk = ThemeRGB(dark ? 0xEDB8BF : 0x722F37).readable(on: surfaces, dark: dark).color
        // Ink on Paper marks Work destinations; file selection keeps its neutral edge.
        let selectedFill = ThemeRGB(0x7A333D).readable(on: surfaces, dark: dark, ratio: 3)
        let selectedDark = selectedFill.prefersLightText
        let selectedText = ThemeRGB(selectedDark ? 0xFFFAF3 : 0x2A2624)
            .readable(on: [selectedFill], dark: selectedDark)
        workSelectedFill = selectedFill.color
        workSelectedText = selectedText.color
        workSelectedSecondaryText = selectedText.mixed(with: selectedFill, amount: 0.15)
            .readable(on: [selectedFill], dark: selectedDark).color
        func overlay(_ ink: ThemeRGB, opacity: Double) -> Color {
            let foreground = terminalForeground ?? text
            for step in stride(from: Int(opacity * 1000), through: 0, by: -1) {
                let alpha = Double(step) / 1000
                if foreground.contrast(background.mixed(with: ink, amount: alpha)) >= 4.52 {
                    return ink.color.opacity(alpha)
                }
            }
            return .clear
        }
        terminalHover = overlay(text, opacity: 0.03)
        terminalSelection = overlay(text, opacity: 0.06)
        terminalFailure = overlay(failureTone.inkRGB, opacity: 0.06)
    }

    public static func derived(background: ThemeRGB, foreground: ThemeRGB) -> Self {
        let dark = background.prefersLightText
        let end = ThemeRGB(dark ? 0 : 0xFFFFFF)
        let surfaces = [background, background.mixed(with: end, amount: 0.18),
                        background.mixed(with: end, amount: 0.09), background.mixed(with: end, amount: 0.27)]
        let text = foreground.readable(on: surfaces, dark: dark)
        let secondary = foreground.mixed(with: background, amount: 0.28).readable(on: surfaces, dark: dark)
        let accent = ThemeRGB(dark ? 0xEDB8BF : 0x722F37).readable(on: surfaces, dark: dark)
        return Self(background: surfaces[0], surface: surfaces[1], surfaceMuted: surfaces[2], surfaceElement: surfaces[3],
                    border: background.mixed(with: secondary, amount: 0.3),
                    borderStrong: background.mixed(with: secondary, amount: 0.5), borderActive: accent,
                    text: text, textSecondary: secondary, textTertiary: secondary, accent: accent, accentHover: accent,
                    dark: dark, terminalForeground: foreground)
    }
    public static let light = Self(
        background: ThemeRGB(0xFAF8F5),
        surface: ThemeRGB(0xFFFDF9),
        surfaceMuted: ThemeRGB(0xF3EEE7),
        surfaceElement: ThemeRGB(0xECE6DD),
        border: ThemeRGB(0xE6DFD6),
        borderStrong: ThemeRGB(0xD6CCC0),
        borderActive: ThemeRGB(0xC2B5A6),
        text: ThemeRGB(0x2A2624),
        textSecondary: ThemeRGB(0x5F5752),
        textTertiary: ThemeRGB(0x685F58),
        accent: ThemeRGB(0x722F37),
        accentHover: ThemeRGB(0x5E2630),
        dark: false
    )
    public static let dark = Self(
        background: ThemeRGB(0x2B3036),
        surface: ThemeRGB(0x343B44),
        surfaceMuted: ThemeRGB(0x3C4550),
        surfaceElement: ThemeRGB(0x444E59),
        border: ThemeRGB(0x46505B),
        borderStrong: ThemeRGB(0x56606B),
        borderActive: ThemeRGB(0x6B7683),
        text: ThemeRGB(0xF5F1EA),
        textSecondary: ThemeRGB(0xC8C1B8),
        textTertiary: ThemeRGB(0xC7BFB7),
        accent: ThemeRGB(0x722F37),
        accentHover: ThemeRGB(0x5E2630),
        dark: true
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
