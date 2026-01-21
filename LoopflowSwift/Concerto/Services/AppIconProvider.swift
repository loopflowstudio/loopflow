// Provides app icons by looking up installed applications.

import AppKit
import SwiftUI
import LoopflowCore

struct AppIconProvider {
    static func icon(for app: AppIdentifier) -> NSImage? {
        let workspace = NSWorkspace.shared

        // Try bundle identifier first
        if let bundleID = app.bundleIdentifier,
           let url = workspace.urlForApplication(withBundleIdentifier: bundleID) {
            return workspace.icon(forFile: url.path)
        }

        // Fallback to app name in /Applications
        let appPath = "/Applications/\(app.appName).app"
        if FileManager.default.fileExists(atPath: appPath) {
            return workspace.icon(forFile: appPath)
        }

        return nil
    }

    static func iconImage(for app: AppIdentifier, size: CGFloat = 24) -> Image {
        if let nsImage = icon(for: app) {
            nsImage.size = NSSize(width: size, height: size)
            return Image(nsImage: nsImage)
        }
        return Image(systemName: app.fallbackSystemImage)
    }
}

enum AppIdentifier {
    case cursor
    case warp
    case vscode
    case iterm
    case terminal
    case zed
    case kitty
    case github

    init(ide: IDEApp) {
        switch ide {
        case .cursor: self = .cursor
        case .vscode: self = .vscode
        case .zed: self = .zed
        }
    }

    init(terminal: TerminalApp) {
        switch terminal {
        case .warp: self = .warp
        case .iterm: self = .iterm
        case .terminal: self = .terminal
        case .kitty: self = .kitty
        }
    }

    var bundleIdentifier: String? {
        switch self {
        case .cursor: return "com.todesktop.230313mzl4w4u92"
        case .warp: return "dev.warp.Warp-Stable"
        case .vscode: return "com.microsoft.VSCode"
        case .iterm: return "com.googlecode.iterm2"
        case .terminal: return "com.apple.Terminal"
        case .zed: return "dev.zed.Zed"
        case .kitty: return "net.kovidgoyal.kitty"
        case .github: return nil
        }
    }

    var appName: String {
        switch self {
        case .cursor: return "Cursor"
        case .warp: return "Warp"
        case .vscode: return "Visual Studio Code"
        case .iterm: return "iTerm"
        case .terminal: return "Terminal"
        case .zed: return "Zed"
        case .kitty: return "kitty"
        case .github: return "GitHub"
        }
    }

    var fallbackSystemImage: String {
        switch self {
        case .cursor, .vscode, .zed: return "cursorarrow.rays"
        case .warp, .iterm, .terminal, .kitty: return "terminal"
        case .github: return "arrow.up.right.square"
        }
    }
}
