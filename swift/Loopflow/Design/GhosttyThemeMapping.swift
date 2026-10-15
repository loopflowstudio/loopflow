import Foundation

/// Convert terminal colors into the complete app contract at the import boundary.
/// State still selects semantic roles; ANSI values supply their colors, never state.
public enum GhosttyThemeMapping {
    public static func palette(_ terminal: TerminalPalette) -> PaletteDefinition {
        let background = terminal.background
        let foreground = terminal.foreground
        let dark = background.prefersLightText
        let end = ThemeRGB(dark ? 0 : 0xFFFFFF)
        let surfaces = [background, background.mixed(with: end, amount: 0.18),
                        background.mixed(with: end, amount: 0.09), background.mixed(with: end, amount: 0.27)]
        let text = foreground.readable(on: surfaces, dark: dark)
        let secondary = foreground.mixed(with: background, amount: 0.28).readable(on: surfaces, dark: dark)
        let accent = terminal.indexed[4].readable(on: surfaces, dark: dark)
        let accentHover = accent.mixed(with: foreground, amount: 0.12).readable(on: surfaces, dark: dark)
        let accentText = background.readable(on: [accent, accentHover], dark: accent.prefersLightText)
        func tone(_ seed: ThemeRGB) -> ThemeTone {
            let endpoint = ThemeRGB(dark ? 0xFFFFFF : 0)
            var fill = background
            for step in stride(from: 8, through: 0, by: -1) {
                let candidate = background.mixed(with: seed, amount: Double(step) / 100)
                if endpoint.contrast(candidate) >= 4.52 { fill = candidate; break }
            }
            return ThemeTone(inkRGB: seed.readable(on: surfaces + [fill], dark: dark), fillRGB: fill)
        }
        let failure = tone(terminal.indexed[1])
        let selectedFill = terminal.selectionBackground.readable(on: surfaces, dark: dark, ratio: 3)
        let selectedDark = selectedFill.prefersLightText
        let selectedText = terminal.selectionForeground.readable(on: [selectedFill], dark: selectedDark)
        func overlay(_ ink: ThemeRGB, opacity: Double) -> ThemePaint {
            for step in stride(from: Int(opacity * 1000), through: 0, by: -1) {
                let alpha = Double(step) / 1000
                // Leave room for native color conversion and 8-bit compositing.
                if foreground.contrast(background.mixed(with: ink, amount: alpha)) >= 4.55 {
                    return ThemePaint(rgb: ink, opacity: alpha)
                }
            }
            return ThemePaint(rgb: background, opacity: 0)
        }
        return PaletteDefinition(
            isDark: dark, background: background, surface: surfaces[1], surfaceMuted: surfaces[2], surfaceElement: surfaces[3],
            border: background.mixed(with: secondary, amount: 0.3),
            borderStrong: background.mixed(with: secondary, amount: 0.5), borderActive: accent,
            text: text, textSecondary: secondary, textTertiary: secondary,
            accent: accent, accentHover: accentHover, accentText: accentText, emphasisInk: accent,
            workSelectedFill: selectedFill, workSelectedText: selectedText,
            workSelectedSecondaryText: selectedText.mixed(with: selectedFill, amount: 0.15).readable(on: [selectedFill], dark: selectedDark),
            shadow: terminal.indexed[0], selectionFill: ThemePaint(rgb: surfaces[1], opacity: 0.72),
            workingTone: tone(terminal.indexed[4]), waitingTone: tone(terminal.indexed[3]),
            failureTone: failure, successTone: tone(terminal.indexed[2]), stoppedTone: tone(secondary),
            terminalHover: overlay(text, opacity: 0.03), terminalSelection: overlay(text, opacity: 0.06),
            terminalFailure: overlay(failure.inkRGB, opacity: 0.06)
        )
    }
}
