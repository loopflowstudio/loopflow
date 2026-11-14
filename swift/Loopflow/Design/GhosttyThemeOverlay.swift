import Foundation

public extension TerminalPalette {
    /// The colors Loopflow manages in Ghostty; generation and readback cover the same keys.
    var ghosttyColors: [(String, ThemeRGB)] {
        [
            ("background", background), ("foreground", foreground),
            ("cursor-color", cursor), ("cursor-text", cursorText),
            ("selection-background", selectionBackground), ("selection-foreground", selectionForeground),
            ("search-background", selectionBackground), ("search-foreground", selectionForeground),
            ("search-selected-background", selectionBackground), ("search-selected-foreground", selectionForeground),
        ]
    }
}

public extension ThemeDefinition {
    /// Only normalized values enter Ghostty, never imported commands or paths.
    var ghosttyOverlay: String {
        return """
        theme =
        \(terminal.ghosttyColors.map { "\($0.0) = \($0.1.css)" }.joined(separator: "\n"))
        background-opacity = 1
        background-image =
        cursor-opacity = 1
        bold-color =
        window-colorspace = srgb
        \(terminal.indexed.enumerated().map { "palette = \($0.offset)=\($0.element.css)" }.joined(separator: "\n"))

        """
    }
}
