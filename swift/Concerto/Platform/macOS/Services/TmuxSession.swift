<<<<<<< HEAD
<<<<<<< HEAD
// Manages the tmux session backing a multiplexer terminal pane.
// Concerto owns layout and focus; tmux just hosts one durable shell per pane.
=======
// Manages the tmux session backing the multiplexer terminal pane.
// Concerto owns the outer pane tree; tmux owns shell subdivision inside the terminal pane.
>>>>>>> 55cd605c (lf commit: implement)
=======
// Manages the tmux session backing a multiplexer terminal pane.
// Concerto owns layout and focus; tmux just hosts one durable shell per pane.
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)

import Foundation
import LoopflowCore

@MainActor
final class TmuxSession {
<<<<<<< HEAD
<<<<<<< HEAD
    let sessionName: String
    let worktreePath: String
    private let registry: TmuxSessionRegistry
<<<<<<< HEAD

    init(
        sessionName: String,
        worktreePath: String,
        registry: TmuxSessionRegistry = .shared
    ) {
        self.sessionName = sessionName
        self.worktreePath = worktreePath
        self.registry = registry
    }

    func ensureBaseSession(launchCommand: String? = nil) async throws {
        if await sessionExists(sessionName) {
            registry.track(sessionName: sessionName)
            return
        }

        try await run("tmux", "new-session", "-d", "-s", sessionName, "-c", worktreePath)
        try await run("tmux", "set-option", "-t", sessionName, "status", "off")
        if let launchCommand, !launchCommand.isEmpty {
            try await run("tmux", "send-keys", "-t", sessionName, "-l", launchCommand)
            try await run("tmux", "send-keys", "-t", sessionName, "Enter")
        }
        registry.track(sessionName: sessionName)
    }

    func attachCommand() -> [String] {
        ["tmux", "attach-session", "-t", sessionName]
=======
    let baseName: String
=======
    let sessionName: String
>>>>>>> 19106fdd (concerto: simplify multiplexer helpers)
    let worktreePath: String
=======
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)

    init(
        sessionName: String,
        worktreePath: String,
        registry: TmuxSessionRegistry = .shared
    ) {
        self.sessionName = sessionName
        self.worktreePath = worktreePath
        self.registry = registry
    }

    func ensureBaseSession(launchCommand: String? = nil) async throws {
        if await sessionExists(sessionName) {
            registry.track(sessionName: sessionName)
            return
        }

        try await run("tmux", "new-session", "-d", "-s", sessionName, "-c", worktreePath)
        try await run("tmux", "set-option", "-t", sessionName, "status", "off")
        if let launchCommand, !launchCommand.isEmpty {
            try await run("tmux", "send-keys", "-t", sessionName, "-l", launchCommand)
            try await run("tmux", "send-keys", "-t", sessionName, "Enter")
        }
        registry.track(sessionName: sessionName)
    }

    func attachCommand() -> [String] {
        ["tmux", "attach-session", "-t", sessionName]
    }

<<<<<<< HEAD
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
<<<<<<< HEAD
        try await run("tmux", "kill-window", "-t", baseName)
>>>>>>> 55cd605c (lf commit: implement)
=======
        try await run("tmux", "kill-window", "-t", sessionName)
>>>>>>> 19106fdd (concerto: simplify multiplexer helpers)
    }

=======
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
    func sessionExists(_ name: String) async -> Bool {
        do {
            try await run("tmux", "has-session", "-t", name)
            return true
        } catch {
            return false
        }
    }

<<<<<<< HEAD
<<<<<<< HEAD
<<<<<<< HEAD
    nonisolated static func killSessionIfExists(named sessionName: String) throws {
        do {
            _ = try runCommandSync(["tmux", "kill-session", "-t", sessionName])
        } catch let TmuxError.commandFailed(_, _, detail)
            where detail.localizedCaseInsensitiveContains("can't find session") {
            return
        }
    }

    private func run(_ args: String...) async throws {
        _ = try await runCommand(args)
    }

    private func runCommand(_ args: [String]) async throws -> String {
        try Self.runCommandSync(args)
    }

    private nonisolated static func runCommandSync(_ args: [String]) throws -> String {
=======
    private func currentWindowPaneCount() async throws -> Int {
        try await displayMessageCount("#{window_panes}")
=======
    func kill() async throws {
        try Self.killSessionIfExists(named: sessionName)
        registry.untrack(sessionName: sessionName)
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
    }

=======
>>>>>>> 14032ed8 (Remove checked-in build artifacts and trim multiplexer scaffolding)
    nonisolated static func killSessionIfExists(named sessionName: String) throws {
        do {
            _ = try runCommandSync(["tmux", "kill-session", "-t", sessionName])
        } catch let TmuxError.commandFailed(_, _, detail)
            where detail.localizedCaseInsensitiveContains("can't find session") {
            return
        }
    }

    private func run(_ args: String...) async throws {
        _ = try await runCommand(args)
    }

    private func runCommand(_ args: [String]) async throws -> String {
        try Self.runCommandSync(args)
    }

<<<<<<< HEAD
<<<<<<< HEAD
    private func runOutput(_ args: [String]) async throws -> String {
>>>>>>> 55cd605c (lf commit: implement)
=======
    private func runCommand(_ args: [String]) async throws -> String {
>>>>>>> 19106fdd (concerto: simplify multiplexer helpers)
=======
    private nonisolated static func runCommandSync(_ args: [String]) throws -> String {
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
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
<<<<<<< HEAD
<<<<<<< HEAD
=======
    case unexpectedOutput(String)
>>>>>>> 55cd605c (lf commit: implement)
=======
>>>>>>> 14032ed8 (Remove checked-in build artifacts and trim multiplexer scaffolding)

    var errorDescription: String? {
        switch self {
        case .commandFailed(let cmd, let status, let detail):
            if detail.isEmpty {
                return "tmux command failed (\(status)): \(cmd)"
            }
            return "tmux command failed (\(status)): \(cmd)\n\(detail)"
<<<<<<< HEAD
<<<<<<< HEAD
=======
        case .unexpectedOutput(let output):
            return "Unexpected tmux output: \(output)"
>>>>>>> 55cd605c (lf commit: implement)
=======
>>>>>>> 14032ed8 (Remove checked-in build artifacts and trim multiplexer scaffolding)
        }
    }
}
