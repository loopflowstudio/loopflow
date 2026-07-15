// Manages the tmux session backing a workspace shell.

import Foundation
import Loopflow

@MainActor
final class TmuxSession {
    let sessionName: String
    let worktreePath: String
    private let registry: TmuxSessionRegistry

    init(
        sessionName: String,
        worktreePath: String,
        registry: TmuxSessionRegistry = .shared
    ) {
        self.sessionName = sessionName
        self.worktreePath = worktreePath
        self.registry = registry
    }

    func ensureBaseSession() async throws {
        if await sessionExists(sessionName) {
            registry.track(sessionName: sessionName)
            return
        }

        try await run("tmux", "new-session", "-d", "-s", sessionName, "-c", worktreePath)
        try await run("tmux", "set-option", "-t", sessionName, "status", "off")
        registry.track(sessionName: sessionName)
    }

    func sessionExists(_ name: String) async -> Bool {
        do {
            try await run("tmux", "has-session", "-t", name)
            return true
        } catch {
            return false
        }
    }

    /// Paste one literal command into the session's shell. `set-buffer` receives
    /// the command as a process argument, not shell input, so the structured
    /// refinement seed cannot become a second command before the paste.
    func sendCommand(_ command: String) async throws {
        let buffer = "lf-ui-\(UUID().uuidString.lowercased())"
        try await run("tmux", "set-buffer", "-b", buffer, command)
        try await run("tmux", "paste-buffer", "-d", "-b", buffer, "-t", sessionName)
        try await run("tmux", "send-keys", "-t", sessionName, "Enter")
    }

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
        let process = Process()
        let stdout = Pipe()
        let stderr = Pipe()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = args
        process.standardOutput = stdout
        process.standardError = stderr
        process.environment = GUIProcessEnvironment.enriched(ProcessInfo.processInfo.environment)

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

    var errorDescription: String? {
        switch self {
        case .commandFailed(let cmd, let status, let detail):
            if detail.isEmpty {
                return "tmux command failed (\(status)): \(cmd)"
            }
            return "tmux command failed (\(status)): \(cmd)\n\(detail)"
        }
    }
}
