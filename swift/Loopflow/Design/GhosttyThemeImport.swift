import Foundation

public struct ImportedTheme: Codable, Identifiable, Equatable, Sendable {
    public let id: String
    public let name: String
    public let terminal: TerminalPalette
    public let diagnostics: [String]
    public var definition: ThemeDefinition {
        ThemeDefinition(id: id, name: name,
                        palette: .derived(background: terminal.background, foreground: terminal.foreground),
                        terminal: terminal)
    }
}

public struct ThemeError: LocalizedError, Sendable {
    public let message: String
    public init(_ message: String) { self.message = message }
    public var errorDescription: String? { message }
}

public enum GhosttyThemeImport {
    public static let maximumBytes = 256 * 1024

    public static func read(_ url: URL) throws -> ImportedTheme {
        let file = try FileHandle(forReadingFrom: url)
        defer { try? file.close() }
        let data = try file.read(upToCount: maximumBytes + 1) ?? Data()
        return try parse(data, name: url.deletingPathExtension().lastPathComponent)
    }

    public static func parse(_ data: Data, name: String) throws -> ImportedTheme {
        guard data.count <= maximumBytes else { throw ThemeError("Theme exceeds 256 KiB.") }
        guard let source = String(data: data, encoding: .utf8) else { throw ThemeError("Theme must be UTF-8 text.") }
        let keys = ["background", "foreground", "cursor-color", "cursor-text", "selection-background", "selection-foreground"]
        var colors: [String: ThemeRGB] = [:]
        var indexed: [Int: ThemeRGB] = [:]
        var diagnostics: [String] = []
        func rgb(_ value: String, line: Int, key: String) throws -> ThemeRGB {
            let value = value.hasPrefix("#") ? String(value.dropFirst()) : value
            guard value.count == 6, value.allSatisfy({ $0.isHexDigit && $0.isASCII }),
                  let hex = UInt32(value, radix: 16) else {
                throw ThemeError("Line \(line), \(key): supply six-digit RGB (for example #AABBCC); named, quoted and conditional colors are unsupported.")
            }
            return ThemeRGB(hex)
        }
        for (offset, raw) in source.components(separatedBy: .newlines).enumerated() {
            let line = offset + 1
            let text = raw.trimmingCharacters(in: .whitespaces)
            if text.isEmpty || text.hasPrefix("#") { continue }
            let parts = text.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
            let key = String(parts[0]).trimmingCharacters(in: .whitespaces)
            guard parts.count == 2 else { throw ThemeError("Line \(line): expected key = value.") }
            let value = String(parts[1]).trimmingCharacters(in: .whitespaces)
            if keys.contains(key) {
                if colors[key] != nil { diagnostics.append("Line \(line): repeated \(key); final value used.") }
                colors[key] = try rgb(value, line: line, key: key)
            } else if key == "palette" {
                let pair = value.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
                let indexText = String(pair[0]).trimmingCharacters(in: .whitespaces)
                guard pair.count == 2, !indexText.isEmpty, indexText.allSatisfy({ $0.isASCII && $0.isNumber }),
                      let index = Int(indexText), (0...255).contains(index) else {
                    throw ThemeError("Line \(line), palette: expected decimal index 0–255 = RGB.")
                }
                if indexed[index] != nil { diagnostics.append("Line \(line): repeated palette \(index); final value used.") }
                indexed[index] = try rgb(String(pair[1]).trimmingCharacters(in: .whitespaces), line: line, key: "palette \(index)")
            } else {
                diagnostics.append("Line \(line): omitted \(key) (only RGB colors are imported; use a concrete theme file).")
            }
        }
        guard let background = colors["background"], let foreground = colors["foreground"] else {
            throw ThemeError("Theme requires background and foreground RGB colors.")
        }
        let dark = background.contrast(ThemeRGB(0xFFFFFF)) > background.contrast(ThemeRGB(0))
        var terminal = TerminalPalette.standard(dark: dark)
        terminal.background = background; terminal.foreground = foreground
        terminal.cursor = colors["cursor-color"] ?? foreground
        terminal.cursorText = colors["cursor-text"] ?? background
        terminal.selectionBackground = colors["selection-background"] ?? foreground
        terminal.selectionForeground = colors["selection-foreground"] ?? background
        for (index, value) in indexed { terminal.indexed[index] = value }
        diagnostics.append("Supplied: \(colors.keys.sorted().joined(separator: ", ")); \(indexed.count) indexed colors.")
        diagnostics.append("Derived: \(keys.filter { colors[$0] == nil }.joined(separator: ", ")); \(256 - indexed.count) default indexed colors; application semantic roles.")
        return ImportedTheme(id: UUID().uuidString, name: name, terminal: terminal, diagnostics: diagnostics)
    }
}
