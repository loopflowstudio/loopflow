import Foundation
import SwiftUI

public extension Color {
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

/// Opaque sRGB shared by native, SwiftUI and terminal consumers.
public struct ThemeRGB: Equatable, Hashable, Sendable {
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
        let first = luminance, second = other.luminance
        return (max(first, second) + 0.05) / (min(first, second) + 0.05)
    }
    var prefersLightText: Bool { contrast(Self(0xFFFFFF)) > contrast(Self(0)) }
    /// White or black, whichever reads better on this color.
    public var contrastingText: Self { Self(prefersLightText ? 0xFFFFFF : 0) }
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

public struct TerminalPalette: Equatable, Sendable {
    public var background: ThemeRGB
    public var foreground: ThemeRGB
    public var cursor: ThemeRGB
    public var cursorText: ThemeRGB
    public var selectionBackground: ThemeRGB
    public var selectionForeground: ThemeRGB
    public var indexed: [ThemeRGB]

    public static func standard(dark: Bool) -> Self {
        (dark ? ThemeDefinition.dark : ThemeDefinition.light).terminal
    }

    /// xterm's 256-color cube and gray ramp, which theme files rarely restate.
    public static let xtermExtended: [ThemeRGB] = (16..<256).map { index in
        if index >= 232 {
            let gray = UInt32(8 + 10 * (index - 232))
            return ThemeRGB(gray << 16 | gray << 8 | gray)
        }
        let levels: [UInt32] = [0, 95, 135, 175, 215, 255], cube = index - 16
        return ThemeRGB(levels[cube / 36] << 16 | levels[cube / 6 % 6] << 8 | levels[cube % 6])
    }

    public var readabilityErrors: [String] {
        (indexed.count == 256 ? [] : ["Terminal palette requires 256 indexed colors."]) +
        [("Default text", foreground, background), ("Selection text", selectionForeground, selectionBackground),
         ("Cursor text", cursorText, cursor)].compactMap { name, ink, base in
            let ratio = ink.contrast(base)
            return ratio < 4.5 ? String(format: "%@: %@ on %@ is %.2f:1; requires 4.5:1.", name, ink.css, base.css, ratio) : nil
        }
    }
}

/// A complete theme. Its name is its identity: config and files refer to it by name.
public struct ThemeDefinition: Identifiable, Hashable, Sendable {
    public let name: String
    public let palette: LoopflowPalette
    public let terminal: TerminalPalette
    public var id: String { name }
    public var isDark: Bool { palette.isDark }
    public var colorScheme: ColorScheme { isDark ? .dark : .light }
    public static let light = builtin("Loopflow Light")
    public static let dark = builtin("Loopflow Dark")
    /// The fixed catalogue: Loopflow's own pair first, then the families by name. Every family's
    /// themes start with its name, so both appearances list the families in one order.
    public static let builtins: [Self] = {
        guard let directory = Bundle.loopflowResources.url(forResource: "Themes", withExtension: nil)?
            .appendingPathComponent("Catalogue", isDirectory: true) else {
            preconditionFailure("Bundled themes are missing")
        }
        do {
            let themes = try ThemeFile.directory(directory).map { url in
                let theme = try ThemeFile.read(url, complete: true).theme
                guard theme.readabilityErrors.isEmpty else { throw ThemeError("\(theme.name): \(theme.readabilityErrors.joined(separator: "; "))") }
                return theme
            }
            return themes.sorted { first, second in
                let loopflow = (first.name.hasPrefix("Loopflow "), second.name.hasPrefix("Loopflow "))
                return loopflow.0 != loopflow.1 ? loopflow.0 : first.name < second.name
            }
        } catch { preconditionFailure("Invalid bundled theme: \(error)") }
    }()

    private static func builtin(_ name: String) -> Self {
        guard let theme = builtins.first(where: { $0.name == name }) else { preconditionFailure("Missing bundled theme \(name)") }
        return theme
    }
    public var readabilityErrors: [String] { terminal.readabilityErrors + palette.definition.readabilityErrors }
    public init(name: String, palette: LoopflowPalette, terminal: TerminalPalette) {
        self.name = name; self.palette = palette; self.terminal = terminal
    }
    /// Two themes are equal when they color the same.
    public static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.name == rhs.name && lhs.palette.definition == rhs.palette.definition && lhs.terminal == rhs.terminal
    }
    // The colors are only Equatable; the name spreads themes well enough.
    public func hash(into hasher: inout Hasher) { hasher.combine(name) }
}

private struct ThemeKey: EnvironmentKey { static let defaultValue = ThemeDefinition.light }
public extension EnvironmentValues {
    var theme: ThemeDefinition {
        get { self[ThemeKey.self] }
        set { self[ThemeKey.self] = newValue }
    }
}
