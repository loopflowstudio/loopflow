import Foundation
import SwiftUI

/// Opaque sRGB shared by native, SwiftUI and terminal consumers.
public struct ThemeRGB: Codable, Equatable, Hashable, Sendable {
    public let hex: UInt32
    public init(_ hex: UInt32) { self.hex = hex & 0xFFFFFF }
    public var color: Color { Color(hex: UInt(hex)) }
    public var css: String { String(format: "#%06X", hex) }
    public var channels: [Double] {
        [Double((hex >> 16) & 255), Double((hex >> 8) & 255), Double(hex & 255)].map { $0 / 255 }
    }
    public var luminance: Double {
        let c = channels.map { $0 <= 0.04045 ? $0 / 12.92 : pow(($0 + 0.055) / 1.055, 2.4) }
        return c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722
    }
    public func contrast(_ other: Self) -> Double {
        (max(luminance, other.luminance) + 0.05) / (min(luminance, other.luminance) + 0.05)
    }
    public func mixed(with other: Self, amount: Double) -> Self {
        let c: [UInt32] = zip(channels, other.channels).map { first, second in
            let blended = first * (1.0 - amount) + second * amount
            return UInt32(blended * 255.0 + 0.5)
        }
        return Self(c[0] << 16 | c[1] << 8 | c[2])
    }
    public func readable(on surfaces: [Self], dark: Bool, ratio: Double = 4.5) -> Self {
        let end = Self(dark ? 0xFFFFFF : 0)
        for step in 0...255 {
            let value = mixed(with: end, amount: Double(step) / 255)
            if surfaces.allSatisfy({ value.contrast($0) >= ratio + 0.02 }) { return value }
        }
        return end
    }
}

public struct TerminalPalette: Codable, Equatable, Sendable {
    public var background: ThemeRGB
    public var foreground: ThemeRGB
    public var cursor: ThemeRGB
    public var cursorText: ThemeRGB
    public var selectionBackground: ThemeRGB
    public var selectionForeground: ThemeRGB
    public var indexed: [ThemeRGB]

    public static func standard(dark: Bool) -> Self {
        let background = ThemeRGB(dark ? 0x2B3036 : 0xFAF8F5)
        let foreground = ThemeRGB(dark ? 0xF5F1EA : 0x2A2624)
        let ansi: [UInt32] = dark
            ? [0x1A1816, 0xD4756A, 0x8B9A6B, 0xD4A574, 0x7B8FA8, 0xA67B93, 0x7FAFAF, 0xC8C1B8,
               0x7F766F, 0xE89888, 0xABB97B, 0xE8C594, 0x9BAFC8, 0xC69BB3, 0x9FCFCF, 0xF5F1EA]
            : [0x2A2624, 0xA02E25, 0x31633E, 0x79520D, 0x24579E, 0x722F37, 0x246568, 0xB5ADA3,
               0x685F58, 0xA02E25, 0x31633E, 0x79520D, 0x24579E, 0x722F37, 0x246568, 0xFFFDF9]
        var indexed = ansi.map(ThemeRGB.init)
        let levels: [UInt32] = [0, 95, 135, 175, 215, 255]
        for r in levels { for g in levels { for b in levels { indexed.append(ThemeRGB(r << 16 | g << 8 | b)) } } }
        for n in 0..<24 { let v = UInt32(8 + n * 10); indexed.append(ThemeRGB(v * 0x010101)) }
        return Self(background: background, foreground: foreground,
                    cursor: foreground, cursorText: background,
                    selectionBackground: foreground, selectionForeground: background, indexed: indexed)
    }

    public var readabilityErrors: [String] {
        [("Default text", foreground, background), ("Selection text", selectionForeground, selectionBackground),
         ("Cursor text", cursorText, cursor)].compactMap { name, ink, base in
            let ratio = ink.contrast(base)
            return ratio < 4.5 ? String(format: "%@: %@ on %@ is %.2f:1; requires 4.5:1.", name, ink.css, base.css, ratio) : nil
        }
    }
    public var ansiWarnings: [String] {
        indexed.prefix(16).enumerated().compactMap { index, color in
            color.contrast(background) < 4.5 ? "ANSI \(index): \(String(format: "%.2f", color.contrast(background))):1 (original retained)" : nil
        }
    }
}

public struct ThemeDefinition: Identifiable, Sendable {
    public let id: String
    public let name: String
    public let palette: LoopflowPalette
    public let terminal: TerminalPalette
    public var isDark: Bool { palette.isDark }
    public var colorScheme: ColorScheme { isDark ? .dark : .light }
    public static let light = Self(id: "light", name: "Light", palette: .light, terminal: .standard(dark: false))
    public static let dark = Self(id: "dark", name: "Dark", palette: .dark, terminal: .standard(dark: true))
    public static let builtins = [light, dark]
    public init(id: String, name: String, palette: LoopflowPalette, terminal: TerminalPalette) {
        self.id = id; self.name = name; self.palette = palette; self.terminal = terminal
    }
}

private struct ThemeKey: EnvironmentKey { static let defaultValue = ThemeDefinition.light }
public extension EnvironmentValues {
    var theme: ThemeDefinition {
        get { self[ThemeKey.self] }
        set { self[ThemeKey.self] = newValue }
    }
}
