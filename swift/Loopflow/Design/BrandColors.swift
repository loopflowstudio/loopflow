import SwiftUI

/// A complete theme record. Every field is required when decoding; no view defaults.
public struct PaletteDefinition: Codable, Equatable, Sendable {
    public let isDark: Bool
    public let background: ThemeRGB
    public let surface: ThemeRGB
    public let surfaceMuted: ThemeRGB
    public let surfaceElement: ThemeRGB
    public let border: ThemeRGB
    public let borderStrong: ThemeRGB
    public let borderActive: ThemeRGB
    public let text: ThemeRGB
    public let textSecondary: ThemeRGB
    public let textTertiary: ThemeRGB
    public let accent: ThemeRGB
    public let accentHover: ThemeRGB
    public let accentText: ThemeRGB
    public let emphasisInk: ThemeRGB
    public let workSelectedFill: ThemeRGB
    public let workSelectedText: ThemeRGB
    public let workSelectedSecondaryText: ThemeRGB
    public let shadow: ThemeRGB
    public let selectionFill: ThemePaint
    public let workingTone: ThemeTone
    public let waitingTone: ThemeTone
    public let failureTone: ThemeTone
    public let successTone: ThemeTone
    public let stoppedTone: ThemeTone
    public let terminalHover: ThemePaint
    public let terminalSelection: ThemePaint
    public let terminalFailure: ThemePaint
}

public struct ThemePaint: Codable, Equatable, Sendable {
    public let rgb: ThemeRGB
    public let opacity: Double
    public var color: Color { rgb.color.opacity(opacity) }
}

public struct ThemeTone: Codable, Equatable, Sendable {
    let inkRGB: ThemeRGB
    let fillRGB: ThemeRGB
    public var ink: Color { inkRGB.color }
    public var fill: Color { fillRGB.color }
    public var stroke: Color { ink }
    public var text: Color { ink }
}

/// SwiftUI projection of a complete theme. Aliases deliberately share one role.
public struct LoopflowPalette: Codable, Sendable {
    public let definition: PaletteDefinition
    public init(_ definition: PaletteDefinition) { self.definition = definition }
    public init(from decoder: any Decoder) throws { definition = try PaletteDefinition(from: decoder) }
    public func encode(to encoder: any Encoder) throws { try definition.encode(to: encoder) }
    public var isDark: Bool { definition.isDark }
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
    public var accentText: Color { definition.accentText.color }
    public var emphasisInk: Color { definition.emphasisInk.color }
    public var workSelectedFill: Color { definition.workSelectedFill.color }
    public var workSelectedText: Color { definition.workSelectedText.color }
    public var workSelectedSecondaryText: Color { definition.workSelectedSecondaryText.color }
    public var shadow: Color { definition.shadow.color }
    public var terminalHover: Color { definition.terminalHover.color }
    public var terminalSelection: Color { definition.terminalSelection.color }
    public var terminalFailure: Color { definition.terminalFailure.color }
    public var workingTone: ThemeTone { definition.workingTone }
    public var waitingTone: ThemeTone { definition.waitingTone }
    public var failureTone: ThemeTone { definition.failureTone }
    public var successTone: ThemeTone { definition.successTone }
    public var stoppedTone: ThemeTone { definition.stoppedTone }
    public var working: Color { workingTone.ink }
    public var waiting: Color { waitingTone.ink }
    public var failure: Color { failureTone.ink }
    public var success: Color { successTone.ink }
    public var selectionFill: Color { definition.selectionFill.color }
    public var selectionEdge: Color { textSecondary }
    public var keyboardFocus: Color { emphasisInk }
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
        for base in surfaces {
            for ink in [text, textSecondary, textTertiary, emphasisInk, workingTone.inkRGB,
                        waitingTone.inkRGB, failureTone.inkRGB, successTone.inkRGB] {
                check("text", ink, on: base)
            }
            check("Work selection", workSelectedFill, on: base, ratio: 3)
        }
        check("selected text", workSelectedText, on: workSelectedFill)
        check("selected secondary text", workSelectedSecondaryText, on: workSelectedFill)
        check("button text", accentText, on: accent)
        check("pressed button text", accentText, on: accentHover)
        for tone in [workingTone, waitingTone, failureTone, successTone, stoppedTone] {
            check("status text", tone.inkRGB, on: tone.fillRGB)
        }
        for paint in [selectionFill, terminalHover, terminalSelection, terminalFailure] where !(0...1).contains(paint.opacity) {
            errors.append("App paint opacity must be between 0 and 1.")
        }
        return errors
    }
}
