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
public struct ThemeRGB: Codable, Equatable, Hashable, Sendable {
    public let hex: UInt32
    public init(_ hex: UInt32) { self.hex = hex & 0xFFFFFF }
    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let value = try container.decode(UInt32.self, forKey: .hex)
        guard value <= 0xFFFFFF else { throw ThemeError("Saved RGB color exceeds six digits.") }
        hex = value
    }
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
        (dark ? ThemeDefinition.dark : ThemeDefinition.light).terminal
    }

    public var readabilityErrors: [String] {
        (indexed.count == 256 ? [] : ["Terminal palette requires 256 indexed colors."]) +
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

    /// Estimate Ghostty's default faint opacity; personal settings and cell backgrounds can differ.
    public var dimWarnings: [String] {
        ([("Default", foreground)] + indexed.prefix(16).enumerated().map { ("ANSI \($0.offset)", $0.element) })
            .compactMap { name, color in
                let ratio = color.mixed(with: background, amount: 1 - 128.0 / 255).contrast(background)
                return ratio < 4.5
                    ? String(format: "Dim %@: %.2f:1 at 50%% opacity (estimate; original retained)", name, ratio)
                    : nil
            }
    }
}

public struct ThemeDefinition: Codable, Identifiable, Sendable {
    public let id: String
    public let name: String
    public let palette: LoopflowPalette
    public let terminal: TerminalPalette
    public var isDark: Bool { palette.isDark }
    public var colorScheme: ColorScheme { isDark ? .dark : .light }
    public static let slotCount = 24
    public static let light = builtin("light")
    public static let dark = builtin("dark")
    public static let builtins: [Self] = {
        guard let directory = Bundle.loopflowResources.url(forResource: "Themes", withExtension: nil) else {
            preconditionFailure("Bundled themes are missing")
        }
        do {
            let files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
                .filter { $0.pathExtension == "json" }
            let themes = try files.map { url in
                let theme = try JSONDecoder().decode(Self.self, from: Data(contentsOf: url))
                guard theme.readabilityErrors.isEmpty else { throw ThemeError("\(theme.name): \(theme.readabilityErrors.joined(separator: "; "))") }
                return theme
            }
            precondition(themes.count == slotCount && Set(themes.map(\.id)).count == slotCount, "Bundled theme library requires 24 unique slots")
            return themes.sorted { first, second in
                let order = ["light": 0, "dark": 1, "neo-light": 2, "hearth": 3]
                if order[first.id] != order[second.id] { return (order[first.id] ?? 4) < (order[second.id] ?? 4) }
                return first.name < second.name
            }
        } catch { preconditionFailure("Invalid bundled theme: \(error)") }
    }()
    /// One-time migration inputs; retired entries never enlarge the catalogue.
    public static func retiredBuiltin(_ id: String) throws -> Self? {
        let names = ["bundled:Catppuccin Frappe": "Catppuccin Frappe", "bundled:Catppuccin Macchiato": "Catppuccin Macchiato"]
        guard let name = names[id] else { return nil }
        guard let directory = Bundle.loopflowResources.url(forResource: "Themes", withExtension: nil) else {
            throw ThemeError("Bundled migration palettes are missing.")
        }
        return try JSONDecoder().decode(Self.self, from: Data(contentsOf: directory.appendingPathComponent("Migrations/\(name).json")))
    }

    private static func builtin(_ id: String) -> Self {
        guard let theme = builtins.first(where: { $0.id == id }) else { preconditionFailure("Missing bundled theme \(id)") }
        return theme
    }
    public var readabilityErrors: [String] { terminal.readabilityErrors + palette.definition.readabilityErrors }
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
