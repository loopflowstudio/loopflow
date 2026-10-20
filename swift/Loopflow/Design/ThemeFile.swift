import Foundation

public struct ThemeError: LocalizedError, Sendable {
    public let message: String
    public init(_ message: String) { self.message = message }
    public var errorDescription: String? { message }
}

/// A theme file in Ghostty syntax. Ghostty's RGB keys give the terminal; optional
/// `loopflow-` keys give app roles, which otherwise derive once through
/// `GhosttyThemeMapping`. Built-ins and your themes share this one format.
public struct ThemeFile: Sendable {
    public let theme: ThemeDefinition
    public let diagnostics: [String]
    public static let maximumBytes = 256 * 1024

    /// Every app role a file can set, as `loopflow-<key>`.
    public static let appRoles: [(key: String, path: WritableKeyPath<PaletteDefinition, ThemeRGB> & Sendable)] = [
        ("background", \.background), ("surface", \.surface), ("surface-muted", \.surfaceMuted),
        ("surface-element", \.surfaceElement), ("border", \.border), ("border-strong", \.borderStrong),
        ("border-active", \.borderActive), ("shadow", \.shadow), ("text", \.text),
        ("text-secondary", \.textSecondary), ("text-tertiary", \.textTertiary), ("emphasis-ink", \.emphasisInk),
        ("accent", \.accent), ("accent-hover", \.accentHover), ("accent-pressed", \.accentPressed),
        ("accent-text", \.accentText), ("accent-ink", \.accentInk), ("work-selected-fill", \.workSelectedFill),
        ("work-selected-text", \.workSelectedText), ("work-selected-secondary-text", \.workSelectedSecondaryText),
        ("selection-fill", \.selectionFill.rgb),
        ("working-ink", \.workingTone.inkRGB), ("working-fill", \.workingTone.fillRGB),
        ("warning-ink", \.warningTone.inkRGB), ("warning-fill", \.warningTone.fillRGB),
        ("available-ink", \.availableTone.inkRGB), ("available-fill", \.availableTone.fillRGB),
        ("failure-ink", \.failureTone.inkRGB), ("failure-fill", \.failureTone.fillRGB),
        ("success-ink", \.successTone.inkRGB), ("success-fill", \.successTone.fillRGB),
        ("stopped-ink", \.stoppedTone.inkRGB), ("stopped-fill", \.stoppedTone.fillRGB),
    ]
    /// Keys an earlier release wrote. `waiting` was one role for warnings and
    /// availability; its colors stay with the warning role it mostly served.
    public static let retiredRoles = ["loopflow-waiting-ink": "loopflow-warning-ink",
                                      "loopflow-waiting-fill": "loopflow-warning-fill"]
    private static let terminalKeys: [(key: String, path: WritableKeyPath<TerminalPalette, ThemeRGB> & Sendable)] = [
        ("background", \.background), ("foreground", \.foreground), ("cursor-color", \.cursor),
        ("cursor-text", \.cursorText), ("selection-background", \.selectionBackground),
        ("selection-foreground", \.selectionForeground),
    ]

    /// Theme files in a directory: every visible regular file, named by its filename.
    public static func directory(_ url: URL) throws -> [URL] {
        try FileManager.default.contentsOfDirectory(at: url, includingPropertiesForKeys: [.isRegularFileKey],
                                                    options: [.skipsHiddenFiles])
            .filter { (try? $0.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true }
            .sorted { $0.lastPathComponent < $1.lastPathComponent }
    }

    /// Rename retired role keys in your theme files, in place. Returns the files changed.
    public static func migrateRetiredRoles(in directory: URL) throws -> [URL] {
        guard FileManager.default.fileExists(atPath: directory.path) else { return [] }
        return try Self.directory(directory).filter { url in
            guard let source = try? String(contentsOf: url, encoding: .utf8) else { return false }
            var changed = false
            let lines = source.components(separatedBy: "\n").map { line in
                let key = line.split(separator: "=", maxSplits: 1).first.map { $0.trimmingCharacters(in: .whitespaces) }
                guard let key, let renamed = retiredRoles[key], let range = line.range(of: key) else { return line }
                changed = true
                return line.replacingCharacters(in: range, with: renamed)
            }
            if changed { try Data(lines.joined(separator: "\n").utf8).write(to: url, options: .atomic) }
            return changed
        }
    }

    public static func read(_ url: URL, complete: Bool = false) throws -> Self {
        let file = try FileHandle(forReadingFrom: url)
        defer { try? file.close() }
        let data = try file.read(upToCount: maximumBytes + 1) ?? Data()
        return try parse(data, name: url.lastPathComponent, complete: complete)
    }

    /// Parse one theme. `complete` requires every terminal and app value, as built-ins do.
    public static func parse(_ data: Data, name: String, complete: Bool = false) throws -> Self {
        guard data.count <= maximumBytes else { throw ThemeError("Theme exceeds 256 KiB.") }
        guard let source = String(data: data, encoding: .utf8) else { throw ThemeError("Theme must be UTF-8 text.") }
        var colors: [String: ThemeRGB] = [:]
        var roles: [String: ThemeRGB] = [:]
        var indexed: [Int: ThemeRGB] = [:]
        var appearance: Bool?
        var opacity: Double?
        var diagnostics: [String] = []
        func rgb(_ value: String, line: Int, key: String) throws -> ThemeRGB {
            let value = value.hasPrefix("#") ? String(value.dropFirst()) : value
            guard value.count == 6, value.allSatisfy({ $0.isHexDigit && $0.isASCII }),
                  let hex = UInt32(value, radix: 16) else {
                throw ThemeError("Line \(line), \(key): supply six-digit RGB (for example #AABBCC); named, quoted and conditional colors are unsupported.")
            }
            return ThemeRGB(hex)
        }
        func repeated<Value>(_ existing: Value?, line: Int, key: String) {
            if existing != nil { diagnostics.append("Line \(line): repeated \(key); final value used.") }
        }
        for (offset, raw) in source.components(separatedBy: .newlines).enumerated() {
            let line = offset + 1
            let text = raw.trimmingCharacters(in: .whitespaces)
            if text.isEmpty || text.hasPrefix("#") { continue }
            let parts = text.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
            let key = String(parts[0]).trimmingCharacters(in: .whitespaces)
            guard parts.count == 2 else { throw ThemeError("Line \(line): expected key = value.") }
            let value = String(parts[1]).trimmingCharacters(in: .whitespaces)
            if terminalKeys.contains(where: { $0.key == key }) {
                repeated(colors[key], line: line, key: key)
                colors[key] = try rgb(value, line: line, key: key)
            } else if key == "palette" {
                let pair = value.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
                let indexText = String(pair[0]).trimmingCharacters(in: .whitespaces)
                guard pair.count == 2, !indexText.isEmpty, indexText.allSatisfy({ $0.isASCII && $0.isNumber }),
                      let index = Int(indexText), (0...255).contains(index) else {
                    throw ThemeError("Line \(line), palette: expected decimal index 0–255 = RGB.")
                }
                repeated(indexed[index], line: line, key: "palette \(index)")
                indexed[index] = try rgb(String(pair[1]).trimmingCharacters(in: .whitespaces), line: line, key: "palette \(index)")
            } else if key == "loopflow-appearance" {
                guard ["light", "dark"].contains(value) else { throw ThemeError("Line \(line), \(key): use light or dark.") }
                repeated(appearance, line: line, key: key)
                appearance = value == "dark"
            } else if key == "loopflow-selection-opacity" {
                guard let number = Double(value), (0...1).contains(number) else {
                    throw ThemeError("Line \(line), \(key): use a number from 0 to 1.")
                }
                repeated(opacity, line: line, key: key)
                opacity = number
            } else if key.hasPrefix("loopflow-"), appRoles.contains(where: { "loopflow-\($0.key)" == key }) {
                repeated(roles[key], line: line, key: key)
                roles[key] = try rgb(value, line: line, key: key)
            } else if let renamed = retiredRoles[key] {
                diagnostics.append("Line \(line): \(key) is now \(renamed); not applied.")
            } else {
                diagnostics.append("Line \(line): omitted \(key) (themes import only colors).")
            }
        }
        guard let background = colors["background"], let foreground = colors["foreground"] else {
            throw ThemeError("Theme requires background and foreground RGB colors.")
        }
        if complete {
            var missing: [String] = terminalKeys.map(\.key).filter { colors[$0] == nil }
            missing += (0..<16).filter { indexed[$0] == nil }.map { "palette \($0)" }
            missing += appRoles.map { "loopflow-\($0.key)" }.filter { roles[$0] == nil }
            if appearance == nil { missing.append("loopflow-appearance") }
            if opacity == nil { missing.append("loopflow-selection-opacity") }
            guard missing.isEmpty else { throw ThemeError("\(name) is incomplete: \(missing.joined(separator: ", ")).") }
        }
        let dark = appearance ?? background.prefersLightText
        var terminal = TerminalPalette(
            background: background, foreground: foreground,
            cursor: colors["cursor-color"] ?? foreground, cursorText: colors["cursor-text"] ?? background,
            selectionBackground: colors["selection-background"] ?? foreground,
            selectionForeground: colors["selection-foreground"] ?? background,
            // A complete file supplies ANSI 0–15; others fall back to the matching Loopflow preset.
            indexed: (complete ? Array(repeating: background, count: 16) : Array(TerminalPalette.standard(dark: dark).indexed.prefix(16)))
                + TerminalPalette.xtermExtended
        )
        for (index, value) in indexed { terminal.indexed[index] = value }
        var palette = GhosttyThemeMapping.palette(terminal)
        palette.isDark = dark
        for role in appRoles { if let value = roles["loopflow-\(role.key)"] { palette[keyPath: role.path] = value } }
        if let opacity { palette.selectionFill.opacity = opacity }
        if !complete {
            let derived = terminalKeys.map(\.key).filter { colors[$0] == nil }
            diagnostics.append("Supplied: \(colors.keys.sorted().joined(separator: ", ")); \(indexed.count) indexed colors; \(roles.count) app roles.")
            diagnostics.append("Derived: \((derived + (roles.count < appRoles.count ? ["remaining app roles"] : [])).joined(separator: ", ")).")
        }
        return Self(theme: ThemeDefinition(name: name, palette: LoopflowPalette(palette), terminal: terminal), diagnostics: diagnostics)
    }

    /// The complete file for a theme, readable by `parse(_:name:complete:)` and by Ghostty for its colors.
    public static func text(_ theme: ThemeDefinition) -> String {
        let palette = theme.palette.definition
        let terminal = terminalKeys.map { "\($0.key) = \(theme.terminal[keyPath: $0.path].css)" }
        let indexed = theme.terminal.indexed.enumerated().filter { index, color in
            index < 16 || TerminalPalette.xtermExtended[index - 16] != color
        }.map { "palette = \($0.offset)=\($0.element.css)" }
        let roles = appRoles.map { "loopflow-\($0.key) = \(palette[keyPath: $0.path].css)" }
        var lines: [String] = ["# \(theme.name)"] + terminal + indexed
        lines.append("loopflow-appearance = \(palette.isDark ? "dark" : "light")")
        lines += roles
        lines.append("loopflow-selection-opacity = \(palette.selectionFill.opacity)")
        return lines.joined(separator: "\n") + "\n"
    }
}
