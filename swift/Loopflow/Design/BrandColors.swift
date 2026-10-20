import SwiftUI

/// A complete theme record. Every field is required when decoding; no view defaults.
public struct PaletteDefinition: Equatable, Sendable {
    public var isDark: Bool
    public var background: ThemeRGB
    public var surface: ThemeRGB
    public var surfaceMuted: ThemeRGB
    public var surfaceElement: ThemeRGB
    public var border: ThemeRGB
    public var borderStrong: ThemeRGB
    public var borderActive: ThemeRGB
    public var text: ThemeRGB
    public var textSecondary: ThemeRGB
    public var textTertiary: ThemeRGB
    public var accent: ThemeRGB
    public var accentHover: ThemeRGB
    public var accentPressed: ThemeRGB
    public var accentInk: ThemeRGB
    public var accentText: ThemeRGB
    public var emphasisInk: ThemeRGB
    public var workSelectedFill: ThemeRGB
    public var workSelectedText: ThemeRGB
    public var workSelectedSecondaryText: ThemeRGB
    public var shadow: ThemeRGB
    public var selectionFill: ThemePaint
    public var workingTone: ThemeTone
    public var warningTone: ThemeTone
    public var availableTone: ThemeTone
    public var failureTone: ThemeTone
    public var successTone: ThemeTone
    public var stoppedTone: ThemeTone
}

public struct ThemePaint: Equatable, Sendable {
    public var rgb: ThemeRGB
    public var opacity: Double
    public var color: Color { rgb.color.opacity(opacity) }
}

public struct ThemeTone: Equatable, Sendable {
    public var inkRGB: ThemeRGB
    public var fillRGB: ThemeRGB
    public var ink: Color { inkRGB.color }
    public var fill: Color { fillRGB.color }
    public var stroke: Color { ink }
    public var text: Color { ink }
}

/// SwiftUI projection of a complete theme. Aliases deliberately share one role.
public struct LoopflowPalette: Sendable {
    public let definition: PaletteDefinition
    public init(_ definition: PaletteDefinition) { self.definition = definition }
    public var isDark: Bool { definition.isDark }
    public var titlebarColorScheme: ColorScheme { definition.accent.prefersLightText ? .dark : .light }
    public var titlebarBackground: Color {
        (isDark ? definition.accent.mixed(with: definition.background, amount: 0.65) : definition.accent).color
    }
    public var background: Color { definition.background.color }
    public var surface: Color { definition.surface.color }
    public var surfaceMuted: Color { definition.surfaceMuted.color }
    public var surfaceElement: Color { definition.surfaceElement.color }
    public var border: Color { definition.border.color }
    public var borderStrong: Color { definition.borderStrong.color }
    public var borderActive: Color { definition.borderActive.color }
    public var text: Color { definition.text.color }
    public var textSecondary: Color { definition.textSecondary.color }
    public var textTertiary: Color { definition.textTertiary.color }
    public var accent: Color { definition.accent.color }
    public var accentHover: Color { definition.accentHover.color }
    public var accentPressed: Color { definition.accentPressed.color }
    public var accentInk: Color { definition.accentInk.color }
    public var accentText: Color { definition.accentText.color }
    public var emphasisInk: Color { definition.emphasisInk.color }
    public var workSelectedFill: Color { definition.workSelectedFill.color }
    public var workSelectedText: Color { definition.workSelectedText.color }
    public var workSelectedSecondaryText: Color { definition.workSelectedSecondaryText.color }
    public var shadow: Color { definition.shadow.color }
    public var workingTone: ThemeTone { definition.workingTone }
    public var warningTone: ThemeTone { definition.warningTone }
    public var availableTone: ThemeTone { definition.availableTone }
    public var failureTone: ThemeTone { definition.failureTone }
    public var successTone: ThemeTone { definition.successTone }
    public var stoppedTone: ThemeTone { definition.stoppedTone }
    public var working: Color { workingTone.ink }
    public var warning: Color { warningTone.ink }
    /// A conversation available for you to work in; never a warning.
    public var available: Color { availableTone.ink }
    public var failure: Color { failureTone.ink }
    public var success: Color { successTone.ink }
    public var selectionFill: Color { definition.selectionFill.color }
    public var selectionEdge: Color { textSecondary }
    public var keyboardFocus: Color { accentInk }
    public var linkInk: Color { textSecondary }
    public var linkHover: Color { text }
    public var actionInk: Color { textSecondary }
    public var actionHover: Color { surfaceElement }
    public static var light: Self { ThemeDefinition.light.palette }
    public static var dark: Self { ThemeDefinition.dark.palette }
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

extension PaletteDefinition {
    public var readabilityErrors: [String] {
        let surfaces = [background, surface, surfaceMuted, surfaceElement]
        var errors: [String] = []
        func check(_ label: String, _ ink: ThemeRGB, on base: ThemeRGB, ratio: Double = 4.5) {
            if ink.contrast(base) < ratio { errors.append("App \(label) requires \(ratio):1 contrast.") }
        }
        let opacity = min(1, max(0, selectionFill.opacity))
        let selectedSurfaces = surfaces.map { $0.mixed(with: selectionFill.rgb, amount: opacity) }
        for base in surfaces + selectedSurfaces {
            for ink in [text, textSecondary, textTertiary, emphasisInk, accentInk, workingTone.inkRGB,
                        warningTone.inkRGB, availableTone.inkRGB, failureTone.inkRGB, successTone.inkRGB] {
                check("text", ink, on: base)
            }
            // The neutral selection fill is bounded by selectionEdge (textSecondary).
            check("Work selection edge", textSecondary, on: base, ratio: 3)
        }
        check("selected text", workSelectedText, on: workSelectedFill)
        check("selected secondary text", workSelectedSecondaryText, on: workSelectedFill)
        check("button text", accentText, on: accent)
        check("hover button text", accentText, on: accentHover)
        check("pressed button text", accentText, on: accentPressed)
        for tone in [workingTone, warningTone, availableTone, failureTone, successTone, stoppedTone] {
            check("status text", tone.inkRGB, on: tone.fillRGB)
        }
        for paint in [selectionFill] where !(0...1).contains(paint.opacity) {
            errors.append("App paint opacity must be between 0 and 1.")
        }
        return errors
    }
}
