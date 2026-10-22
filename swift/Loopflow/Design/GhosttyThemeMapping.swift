import Foundation

/// Convert terminal colors into the complete app contract at the import boundary.
/// State selects semantic roles; ANSI values supply seeds, never activity.
public enum GhosttyThemeMapping {
    public static func palette(_ terminal: TerminalPalette) -> PaletteDefinition {
        let background = terminal.background
        let dark = background.prefersLightText
        let textEnd = ThemeRGB(dark ? 0xFFFFFF : 0)
        func quiet(_ color: ThemeRGB, retaining amount: Double) -> ThemeRGB {
            let channels = color.channels
            let gray = UInt32((channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722) * 255)
            return ThemeRGB(gray << 16 | gray << 8 | gray).mixed(with: color, amount: amount)
        }
        func surface(_ amount: Double) -> ThemeRGB {
            // Near middle gray, retain one readable text polarity across the ladder.
            for step in stride(from: Int(amount * 1000), through: 0, by: -1) {
                let value = background.mixed(with: textEnd, amount: Double(step) / 1000)
                if textEnd.contrast(value) >= 4.55 { return value }
            }
            return background
        }
        let surfaces = [background, surface(0.025), surface(0.045), surface(0.07)]
        let foreground = quiet(terminal.foreground, retaining: 0.15)
        let text = foreground.readable(on: surfaces, dark: dark)
        let secondary = foreground.mixed(with: background, amount: 0.22).readable(on: surfaces, dark: dark)
        let tertiary = foreground.mixed(with: background, amount: 0.38).readable(on: surfaces, dark: dark)
        let seed = quiet(terminal.indexed[4], retaining: 0.55)
        let accentInk = seed.readable(on: surfaces, dark: dark)
        // A solid control needs its own ramp; readable accent text is not its fill.
        let accentText = ThemeRGB(0xFAF8F5)
        let accent = seed.readable(on: [accentText], dark: false)
        let accentHover = accent.mixed(with: ThemeRGB(0), amount: 0.12)
        let accentPressed = accent.mixed(with: ThemeRGB(0), amount: 0.24)
        func tone(_ seed: ThemeRGB) -> ThemeTone {
            let seed = quiet(seed, retaining: 0.6)
            var fill = background
            for step in stride(from: 7, through: 0, by: -1) {
                let candidate = background.mixed(with: seed, amount: Double(step) / 100)
                if textEnd.contrast(candidate) >= 4.55 { fill = candidate; break }
            }
            return ThemeTone(inkRGB: seed.readable(on: surfaces + [fill], dark: dark), fillRGB: fill)
        }
        // Large Work rows keep the accepted filled geometry, independently of terminal selection.
        let selectedFill = background.mixed(with: seed, amount: 0.16)
            .readable(on: surfaces, dark: dark, ratio: 3)
        let selectedDark = selectedFill.prefersLightText
        let selectedText = foreground.readable(on: [selectedFill], dark: selectedDark)
        return PaletteDefinition(
            isDark: dark, background: background, surface: surfaces[1], surfaceMuted: surfaces[2], surfaceElement: surfaces[3],
            border: background.mixed(with: secondary, amount: 0.22),
            borderStrong: background.mixed(with: secondary, amount: 0.4), borderActive: accentInk,
            text: text, textSecondary: secondary, textTertiary: tertiary,
            accent: accent, accentHover: accentHover, accentPressed: accentPressed, accentInk: accentInk,
            accentText: accentText, emphasisInk: text,
            workSelectedFill: selectedFill, workSelectedText: selectedText,
            workSelectedSecondaryText: selectedText.mixed(with: selectedFill, amount: 0.16).readable(on: [selectedFill], dark: selectedDark),
            shadow: terminal.indexed[0], selectionFill: ThemePaint(rgb: surfaces[1], opacity: 0.72),
            workingTone: tone(terminal.indexed[4]), warningTone: tone(terminal.indexed[3]),
            availableTone: tone(terminal.indexed[5]),
            failureTone: tone(terminal.indexed[1]), successTone: tone(terminal.indexed[2]), stoppedTone: tone(secondary)
        )
    }
}
