// Service for launching terminal sessions.

import AppKit
import Foundation

struct TerminalLauncher {
    enum LaunchError: LocalizedError {
        case unsupportedTerminal(String)
        case launchFailed(String)

        var errorDescription: String? {
            switch self {
            case .unsupportedTerminal(let name): return "Unsupported terminal: \(name)"
            case .launchFailed(let msg): return "Failed to launch: \(msg)"
            }
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

    // MARK: - Terminal Launchers

    private func launchWarp(at path: URL, command: String?) throws {
        var urlString = "warp://action/new_tab?path=\(path.path().addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? path.path())"

        if let cmd = command {
            let encodedCmd = cmd.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? cmd
            urlString += "&command=\(encodedCmd)"
        }

        guard let url = URL(string: urlString) else {
            throw LaunchError.launchFailed("Invalid Warp URL")
        }

        NSWorkspace.shared.open(url)
    }

    private func launchITerm(at path: URL, command: String?) throws {
        let script: String
        if let cmd = command {
            script = """
            tell application "iTerm"
                activate
                create window with default profile
                tell current session of current window
                    write text "cd '\(path.path())' && \(cmd)"
                end tell
            end tell
            """
        } else {
            script = """
            tell application "iTerm"
                activate
                create window with default profile
                tell current session of current window
                    write text "cd '\(path.path())'"
                end tell
            end tell
            """
        }

        try runAppleScript(script)
    }

    private func launchTerminalApp(at path: URL, command: String?) throws {
        let script: String
        if let cmd = command {
            script = """
            tell application "Terminal"
                activate
                do script "cd '\(path.path())' && \(cmd)"
            end tell
            """
        } else {
            script = """
            tell application "Terminal"
                activate
                do script "cd '\(path.path())'"
            end tell
            """
        }

        try runAppleScript(script)
    }

    private func launchKitty(at path: URL, command: String?) throws {
        var args = ["--single-instance", "--directory", path.path()]

        if let cmd = command {
            args.append(contentsOf: ["--", "zsh", "-c", cmd])
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/local/bin/kitty")
        process.arguments = args

        try process.run()
    }

    // MARK: - IDE Launchers

    private func openCursor(at path: URL, workspace: String?) throws {
        let targetPath: String
        if let ws = workspace {
            targetPath = path.appendingPathComponent(ws).path()
        } else if let ws = findCodeWorkspace(in: path) {
            targetPath = ws.path()
        } else {
            targetPath = path.path()
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/local/bin/cursor")
        process.arguments = [targetPath]

        try process.run()
    }

    private func openVSCode(at path: URL, workspace: String?) throws {
        let targetPath: String
        if let ws = workspace {
            targetPath = path.appendingPathComponent(ws).path()
        } else if let ws = findCodeWorkspace(in: path) {
            targetPath = ws.path()
        } else {
            targetPath = path.path()
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/local/bin/code")
        process.arguments = [targetPath]

        try process.run()
    }

    private func openZed(at path: URL) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/local/bin/zed")
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
}
