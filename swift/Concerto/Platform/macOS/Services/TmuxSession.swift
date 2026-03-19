// Manages the tmux session backing the multiplexer terminal pane.
// Concerto owns the outer pane tree; tmux owns shell subdivision inside the terminal pane.

import Foundation
import LoopflowCore

@MainActor
final class TmuxSession {
    let sessionName: String
    let worktreePath: String

    init(waveId: String, worktreePath: String) {
        self.sessionName = "lf-\(waveId)"
        self.worktreePath = worktreePath
    }

    func ensureBaseSession() async throws {
        guard !(await sessionExists(sessionName)) else { return }
        try await run("tmux", "new-session", "-d", "-s", sessionName, "-c", worktreePath)
        try await run("tmux", "set-option", "-t", sessionName, "status", "off")
    }

    func attachCommand() -> [String] {
        ["tmux", "attach-session", "-t", sessionName]
    }

    func split(_ axis: SplitAxis) async throws {
        try await ensureBaseSession()
        switch axis {
        case .horizontal:
            try await run("tmux", "split-window", "-h", "-c", worktreePath, "-t", sessionName)
        case .vertical:
            try await run("tmux", "split-window", "-v", "-c", worktreePath, "-t", sessionName)
        }
    }

    func createWindow() async throws {
        try await ensureBaseSession()
        try await run("tmux", "new-window", "-c", worktreePath, "-t", sessionName)
    }

    func selectNextWindow() async throws {
        try await ensureBaseSession()
        try await run("tmux", "select-window", "-n", "-t", sessionName)
    }

    func selectPreviousWindow() async throws {
        try await ensureBaseSession()
        try await run("tmux", "select-window", "-p", "-t", sessionName)
    }

    func closeFocusedThing() async throws {
        try await ensureBaseSession()

        let paneCount = try await currentWindowPaneCount()
        if paneCount > 1 {
            try await run("tmux", "kill-pane", "-t", sessionName)
            return
        }

        let windowCount = try await sessionWindowCount()
        guard windowCount > 1 else { return }
        try await run("tmux", "kill-window", "-t", sessionName)
    }

    func sessionExists(_ name: String) async -> Bool {
        do {
            try await run("tmux", "has-session", "-t", name)
            return true
        } catch {
            return false
        }
    }

    private func currentWindowPaneCount() async throws -> Int {
        try await displayMessageCount("#{window_panes}")
    }

    private func sessionWindowCount() async throws -> Int {
        try await displayMessageCount("#{session_windows}")
    }

    private func displayMessageCount(_ format: String) async throws -> Int {
        let output = try await runOutput(
            "tmux",
            "display-message",
            "-p",
            "-t",
            sessionName,
            format
        )
        guard let count = Int(output.trimmingCharacters(in: .whitespacesAndNewlines)) else {
            throw TmuxError.unexpectedOutput(output)
        }
        return count
    }

    private func run(_ args: String...) async throws {
        _ = try await runCommand(args)
    }

    private func runOutput(_ args: String...) async throws -> String {
        try await runCommand(args)
    }

    private func runCommand(_ args: [String]) async throws -> String {
        let process = Process()
        let stdout = Pipe()
        let stderr = Pipe()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = args
        process.standardOutput = stdout
        process.standardError = stderr
        process.environment = ProcessInfo.processInfo.environment

        try process.run()
        process.waitUntilExit()

        let stdoutData = stdout.fileHandleForReading.readDataToEndOfFile()
        let stderrData = stderr.fileHandleForReading.readDataToEndOfFile()
        let stdoutText = String(data: stdoutData, encoding: .utf8) ?? ""
        let stderrText = String(data: stderrData, encoding: .utf8) ?? ""

        guard process.terminationStatus == 0 else {
            let detail = [stdoutText, stderrText]
                .filter { !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
                .joined(separator: "\n")
            throw TmuxError.commandFailed(args.joined(separator: " "), status: process.terminationStatus, detail: detail)
        }

        return stdoutText
    }
}

enum TmuxError: LocalizedError {
    case commandFailed(String, status: Int32, detail: String)
    case unexpectedOutput(String)

    var errorDescription: String? {
        switch self {
        case .commandFailed(let cmd, let status, let detail):
            if detail.isEmpty {
                return "tmux command failed (\(status)): \(cmd)"
            }
            return "tmux command failed (\(status)): \(cmd)\n\(detail)"
        case .unexpectedOutput(let output):
            return "Unexpected tmux output: \(output)"
        }
    }
}
