import AppKit
import Loopflow

extension ThemeRGB {
    var nsColor: NSColor {
        let c = channels
        return NSColor(srgbRed: c[0], green: c[1], blue: c[2], alpha: 1)
    }
}

extension ThemeDefinition {
    /// Only normalized values enter Ghostty, never imported commands or paths.
    var ghosttyOverlay: String {
        let t = terminal
        return """
        theme =
        background = \(t.background.css)
        foreground = \(t.foreground.css)
        cursor-color = \(t.cursor.css)
        cursor-text = \(t.cursorText.css)
        selection-background = \(t.selectionBackground.css)
        selection-foreground = \(t.selectionForeground.css)
        search-background = \(t.selectionBackground.css)
        search-foreground = \(t.selectionForeground.css)
        search-selected-background = \(t.selectionBackground.css)
        search-selected-foreground = \(t.selectionForeground.css)
        background-opacity = 1
        background-image =
        cursor-opacity = 1
        \(t.indexed.enumerated().map { "palette = \($0.offset)=\($0.element.css)" }.joined(separator: "\n"))

        """
    }
}

#if GHOSTTY_ENABLED
import GhosttyKit

@MainActor
enum GhosttyThemeConfiguration {
    static func load(_ text: String, into config: ghostty_config_t) throws {
        let path = terminalTemporaryDirectory().appendingPathComponent("loopflow-ghostty-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: path) }
        try Data(text.utf8).write(to: path)
        path.path.withCString { ghostty_config_load_file(config, $0) }
    }

    static func candidate(_ theme: ThemeDefinition, baseline: ghostty_config_t) throws -> ghostty_config_t {
        guard let config = ghostty_config_clone(baseline) else { throw ThemeError("Could not allocate terminal appearance.") }
        do {
            let previousDiagnostics = ghostty_config_diagnostics_count(config)
            try load(theme.ghosttyOverlay, into: config)
            ghostty_config_finalize(config)
            let count = ghostty_config_diagnostics_count(config)
            guard count == previousDiagnostics else {
                let messages = (previousDiagnostics..<count).compactMap { index -> String? in
                    let diagnostic = ghostty_config_get_diagnostic(config, index)
                    return diagnostic.message.map { String(cString: $0) }
                }
                throw ThemeError("Terminal appearance: " + messages.joined(separator: "; "))
            }
            try validate(theme, config: config)
            return config
        } catch { ghostty_config_free(config); throw error }
    }

    static func validate(_ theme: ThemeDefinition, config: ghostty_config_t) throws {
        for (key, expected) in [("background", theme.terminal.background), ("foreground", theme.terminal.foreground)] {
            var color = ghostty_config_color_s()
            let found = key.withCString { ghostty_config_get(config, &color, $0, UInt(key.utf8.count)) }
            guard found, ThemeRGB(UInt32(color.r) << 16 | UInt32(color.g) << 8 | UInt32(color.b)) == expected else {
                throw ThemeError("Terminal \(key) did not accept the selected appearance.")
            }
        }
        var palette = ghostty_config_palette_s()
        guard ghostty_config_get(config, &palette, "palette", 7) else { throw ThemeError("Terminal palette readback failed.") }
        let actual = withUnsafeBytes(of: palette.colors) { bytes in
            Array(bytes.bindMemory(to: ghostty_config_color_s.self)).map { ThemeRGB(UInt32($0.r) << 16 | UInt32($0.g) << 8 | UInt32($0.b)) }
        }
        guard actual == theme.terminal.indexed else { throw ThemeError("Terminal indexed colors did not accept the selected appearance.") }
    }
}
#endif
