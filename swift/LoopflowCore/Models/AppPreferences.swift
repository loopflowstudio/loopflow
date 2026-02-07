// AppPreferences - shared app-level preferences for terminal and IDE.

import Foundation

public enum TerminalApp: String, Sendable, CaseIterable {
    case warp
    case iterm
    case terminal
    case kitty

    public var displayName: String {
        switch self {
        case .warp: return "Warp"
        case .iterm: return "iTerm"
        case .terminal: return "Terminal"
        case .kitty: return "Kitty"
        }
    }
}

public enum IDEApp: String, Sendable, CaseIterable {
    case cursor
    case vscode
    case zed

    public var displayName: String {
        switch self {
        case .cursor: return "Cursor"
        case .vscode: return "VS Code"
        case .zed: return "Zed"
        }
    }
}
