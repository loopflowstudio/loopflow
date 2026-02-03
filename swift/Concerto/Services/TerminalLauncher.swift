// Service for launching terminal sessions.

import AppKit
import ApplicationServices
import Foundation
import LoopflowCore

struct TerminalLauncher {
    enum LaunchError: LocalizedError {
        case unsupportedTerminal(String)
        case launchFailed(String)
        case accessibilityDenied

        var errorDescription: String? {
            switch self {
            case .unsupportedTerminal(let name): return "Unsupported terminal: \(name)"
            case .launchFailed(let msg): return "Failed to launch: \(msg)"
            case .accessibilityDenied: return "Accessibility permission required. Please grant access in System Settings → Privacy & Security → Accessibility, then try again."
            }
        }
    }

    /// Check if we have Accessibility permissions, prompting if not
    private func ensureAccessibilityPermission() throws {
        // Use the string key directly to avoid concurrency issues with the constant
        let options = ["AXTrustedCheckOptionPrompt": true] as CFDictionary
        let trusted = AXIsProcessTrustedWithOptions(options)

        if !trusted {
            throw LaunchError.accessibilityDenied
        }
    }

    func launchTerminal(_ terminal: TerminalApp, at path: URL, command: String? = nil) throws {
        switch terminal {
        case .warp:
            try launchWarp(at: path, command: command)
        case .iterm:
            try launchITerm(at: path, command: command)
        case .terminal:
            try launchTerminalApp(at: path, command: command)
        case .kitty:
            try launchKitty(at: path, command: command)
        }
    }

    func openInIDE(_ ide: IDEApp, at path: URL, workspace: String? = nil) throws {
        switch ide {
        case .cursor:
            try openCursor(at: path, workspace: workspace)
        case .vscode:
            try openVSCode(at: path, workspace: workspace)
        case .zed:
            try openZed(at: path)
        }
    }

    func openInFinder(at path: URL) {
        NSWorkspace.shared.selectFile(nil, inFileViewerRootedAtPath: path.path())
    }

    func openURL(_ url: URL) {
        NSWorkspace.shared.open(url)
    }

    /// Launch a loopflow step in the terminal at the given repo path.
    func launchStep(_ step: String, terminal: TerminalApp, at repo: URL) throws {
        try launchTerminal(terminal, at: repo, command: "lf \(step)")
    }

    // MARK: - Terminal Launchers

    private func launchWarp(at path: URL, command: String?) throws {
        // Warp requires UI scripting via Accessibility - check permission first
        try ensureAccessibilityPermission()

        // Warp doesn't have proper AppleScript support or a command URL parameter
        // Use AppleScript UI scripting: open new tab, type command, press Enter
        let script: String

        if let cmd = command {
            let fullCommand = "cd '\(escapeShellSingleQuotes(path.path()))' && \(cmd)"
            let escapedForAppleScript = escapeForAppleScriptString(fullCommand)

            script = """
            tell application "Warp"
                activate
            end tell

            delay 0.3

            tell application "System Events"
                tell process "Warp"
                    keystroke "t" using command down
                end tell
            end tell

            delay 0.5

            tell application "System Events"
                tell process "Warp"
                    keystroke "\(escapedForAppleScript)"
                    key code 36
                end tell
            end tell
            """
        } else {
            let fullCommand = "cd '\(escapeShellSingleQuotes(path.path()))'"
            let escapedForAppleScript = escapeForAppleScriptString(fullCommand)

            script = """
            tell application "Warp"
                activate
            end tell

            delay 0.3

            tell application "System Events"
                tell process "Warp"
                    keystroke "t" using command down
                end tell
            end tell

            delay 0.5

            tell application "System Events"
                tell process "Warp"
                    keystroke "\(escapedForAppleScript)"
                    key code 36
                end tell
            end tell
            """
        }

        try runAppleScript(script)
    }

    private func launchITerm(at path: URL, command: String?) throws {
        let script: String
        if let cmd = command {
            let fullCommand = "cd '\(escapeShellSingleQuotes(path.path()))' && \(cmd)"
            let escaped = escapeForAppleScriptString(fullCommand)
            script = """
            tell application "iTerm"
                activate
                create window with default profile
                tell current session of current window
                    write text "\(escaped)"
                end tell
            end tell
            """
        } else {
            let fullCommand = "cd '\(escapeShellSingleQuotes(path.path()))'"
            let escaped = escapeForAppleScriptString(fullCommand)
            script = """
            tell application "iTerm"
                activate
                create window with default profile
                tell current session of current window
                    write text "\(escaped)"
                end tell
            end tell
            """
        }

        try runAppleScript(script)
    }

    private func launchTerminalApp(at path: URL, command: String?) throws {
        let script: String
        if let cmd = command {
            let fullCommand = "cd '\(escapeShellSingleQuotes(path.path()))' && \(cmd)"
            let escaped = escapeForAppleScriptString(fullCommand)
            script = """
            tell application "Terminal"
                activate
                do script "\(escaped)"
            end tell
            """
        } else {
            let fullCommand = "cd '\(escapeShellSingleQuotes(path.path()))'"
            let escaped = escapeForAppleScriptString(fullCommand)
            script = """
            tell application "Terminal"
                activate
                do script "\(escaped)"
            end tell
            """
        }

        try runAppleScript(script)
    }

    private func launchKitty(at path: URL, command: String?) throws {
        guard let executableURL = findExecutable("kitty") else {
            throw LaunchError.launchFailed("kitty not found. Install from https://sw.kovidgoyal.net/kitty/")
        }

        var args = ["--single-instance", "--directory", path.path()]
        if let cmd = command {
            args.append(contentsOf: ["--", "zsh", "-c", cmd])
        }

        let process = Process()
        process.executableURL = executableURL
        process.arguments = args
        try process.run()
    }

    // MARK: - IDE Launchers

    private func openCursor(at path: URL, workspace: String?) throws {
        guard let executableURL = findExecutable("cursor") else {
            throw LaunchError.launchFailed("Cursor not found. Install from https://cursor.com")
        }

        let targetPath: String
        if let ws = workspace {
            targetPath = path.appendingPathComponent(ws).path()
        } else if let ws = findCodeWorkspace(in: path) {
            targetPath = ws.path()
        } else {
            targetPath = path.path()
        }

        let process = Process()
        process.executableURL = executableURL
        process.arguments = [targetPath]
        try process.run()
    }

    private func openVSCode(at path: URL, workspace: String?) throws {
        guard let executableURL = findExecutable("code") else {
            throw LaunchError.launchFailed("VS Code not found. Install from https://code.visualstudio.com")
        }

        let targetPath: String
        if let ws = workspace {
            targetPath = path.appendingPathComponent(ws).path()
        } else if let ws = findCodeWorkspace(in: path) {
            targetPath = ws.path()
        } else {
            targetPath = path.path()
        }

        let process = Process()
        process.executableURL = executableURL
        process.arguments = [targetPath]
        try process.run()
    }

    private func openZed(at path: URL) throws {
        guard let executableURL = findExecutable("zed") else {
            throw LaunchError.launchFailed("Zed not found. Install from https://zed.dev")
        }

        let process = Process()
        process.executableURL = executableURL
        process.arguments = [path.path()]
        try process.run()
    }

    // MARK: - Helpers

    private func runAppleScript(_ script: String) throws {
        guard let appleScript = NSAppleScript(source: script) else {
            throw LaunchError.launchFailed("Failed to create AppleScript")
        }

        var error: NSDictionary?
        appleScript.executeAndReturnError(&error)

        if let err = error {
            throw LaunchError.launchFailed(err.description)
        }
    }

    private func findCodeWorkspace(in directory: URL) -> URL? {
        let fm = FileManager.default
        guard let contents = try? fm.contentsOfDirectory(atPath: directory.path()) else {
            return nil
        }

        if let workspace = contents.first(where: { $0.hasSuffix(".code-workspace") }) {
            return directory.appendingPathComponent(workspace)
        }
        return nil
    }

    private func findExecutable(_ name: String) -> URL? {
        // Use login shell to resolve PATH properly
        let process = Process()
        let pipe = Pipe()
        process.executableURL = URL(fileURLWithPath: "/bin/zsh")
        process.arguments = ["-l", "-c", "which \(name)"]
        process.standardOutput = pipe
        process.standardError = FileHandle.nullDevice

        do {
            try process.run()
            process.waitUntilExit()
            if process.terminationStatus == 0 {
                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                if let path = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
                   !path.isEmpty {
                    return URL(fileURLWithPath: path)
                }
            }
        } catch {
            // Fall through to return nil
        }
        return nil
    }

    /// Escape single quotes for shell: ' becomes '\''
    private func escapeShellSingleQuotes(_ string: String) -> String {
        string.replacingOccurrences(of: "'", with: "'\\''")
    }

    /// Escape for AppleScript double-quoted string: \ becomes \\, " becomes \"
    private func escapeForAppleScriptString(_ string: String) -> String {
        string
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
    }
}
