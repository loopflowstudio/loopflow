// Service for checking and installing loopflow and its dependencies.

import Foundation
import os.log

private let logger = Logger(subsystem: "com.loopflow.concerto", category: "setup")

struct SetupService {
    struct DependencyStatus {
        var lfInstalled: Bool
        var lfPath: String?
    }

    private let logFile: URL = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".lf/logs/concerto-setup.log")

    private func log(_ message: String) {
        let timestamp = ISO8601DateFormatter().string(from: Date())
        let line = "[\(timestamp)] \(message)\n"

        // Log to system log
        logger.info("\(message)")

        // Also append to file for easy access
        try? FileManager.default.createDirectory(
            at: logFile.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        if let data = line.data(using: .utf8) {
            if FileManager.default.fileExists(atPath: logFile.path) {
                if let handle = try? FileHandle(forWritingTo: logFile) {
                    handle.seekToEndOfFile()
                    handle.write(data)
                    handle.closeFile()
                }
            } else {
                try? data.write(to: logFile)
            }
        }
    }

    func checkDependencies() -> DependencyStatus {
        log("checkDependencies: searching for lf")
        let lfPath = findExecutable("lf")
        log("checkDependencies: lf path = \(lfPath ?? "not found")")
        return DependencyStatus(
            lfInstalled: lfPath != nil,
            lfPath: lfPath
        )
    }

    /// Install loopflow and all its dependencies
    func install() async throws {
        log("install: starting")

        // Install uv if needed
        let existingUv = findExecutable("uv")
        log("install: existing uv = \(existingUv ?? "not found")")

        if existingUv == nil {
            log("install: installing uv...")
            try await installUv()
        }

        guard let uvPath = findExecutable("uv") else {
            log("install: ERROR - uv not found after install attempt")
            throw SetupError.uvInstallFailed
        }
        log("install: uv path = \(uvPath)")

        // Install loopflow via uv
        log("install: running uv tool install loopflow...")
        try await runCommand(uvPath, args: ["tool", "install", "loopflow"])
        log("install: uv tool install completed")

        // Install dependencies directly (lf init is interactive, can't run headlessly)
        try await installDependencies()
        log("install: completed successfully")
    }

    /// Install Claude Code dependencies
    private func installDependencies() async throws {
        // Install Node.js via Homebrew if needed
        if findExecutable("npm") == nil {
            log("install: installing Node.js...")
            if let brewPath = findExecutable("brew") {
                try await runCommand(brewPath, args: ["install", "node"])
            } else {
                log("install: WARNING - Homebrew not found, skipping Node.js")
            }
        }

        // Install Claude Code if needed
        if findExecutable("claude") == nil {
            if let npmPath = findExecutable("npm") {
                log("install: installing Claude Code...")
                try await runCommand(npmPath, args: ["install", "-g", "@anthropic-ai/claude-code"])
            } else {
                log("install: WARNING - npm not found, skipping Claude Code")
            }
        }

        // Note: worktrunk (wt) is installed by lfd install, not here.
        // Concerto talks to lfd, which uses wt as an implementation detail.
    }

    /// Ensure the loopflow daemon is running
    func ensureDaemonRunning() async throws {
        // Skip if lfd not installed
        guard let lfdPath = findExecutable("lfd") else {
            log("ensureDaemonRunning: lfd not found, skipping")
            return
        }
        log("ensureDaemonRunning: lfd found at \(lfdPath)")

        let plistPath = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/LaunchAgents/com.loopflow.lfd.plist")

        if !FileManager.default.fileExists(atPath: plistPath.path) {
            log("ensureDaemonRunning: plist not found, installing...")
            try await runCommand(lfdPath, args: ["install"])
        } else if !isDaemonRunning() {
            log("ensureDaemonRunning: plist exists but daemon not running, loading...")
            try await runCommand("/bin/launchctl", args: ["load", plistPath.path])
        } else {
            log("ensureDaemonRunning: daemon already running")
        }
    }

    private func isDaemonRunning() -> Bool {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/launchctl")
        process.arguments = ["list", "com.loopflow.lfd"]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice

        do {
            try process.run()
            process.waitUntilExit()
            return process.terminationStatus == 0
        } catch {
            return false
        }
    }

    private func installUv() async throws {
        try await runCommand("/bin/sh", args: [
            "-c",
            "curl -LsSf https://astral.sh/uv/install.sh | sh"
        ])
    }

    private func findExecutable(_ name: String) -> String? {
        for dir in binDirectories {
            let path = (dir as NSString).appendingPathComponent(name)
            if FileManager.default.isExecutableFile(atPath: path) {
                return path
            }
        }
        return nil
    }

    private var binDirectories: [String] {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        return [
            "\(home)/.local/bin",
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/usr/bin",
            "\(home)/.cargo/bin",
        ]
    }

    private func runCommand(_ executable: String, args: [String]) async throws {
        let cmdString = "\(executable) \(args.joined(separator: " "))"
        log("runCommand: \(cmdString)")

        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            let process = Process()
            let pipe = Pipe()

            process.executableURL = URL(fileURLWithPath: executable)
            process.arguments = args
            process.standardOutput = pipe
            process.standardError = pipe

            do {
                try process.run()
                process.waitUntilExit()

                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                let output = String(data: data, encoding: .utf8) ?? ""

                log("runCommand: exit code \(process.terminationStatus)")
                if !output.isEmpty {
                    log("runCommand: output = \(output.prefix(500))")
                }

                if process.terminationStatus == 0 {
                    continuation.resume()
                } else {
                    continuation.resume(throwing: SetupError.commandFailed(output.isEmpty ? "Exit code \(process.terminationStatus)" : output))
                }
            } catch {
                log("runCommand: exception = \(error)")
                continuation.resume(throwing: error)
            }
        }
    }

    enum SetupError: LocalizedError {
        case commandFailed(String)
        case uvInstallFailed

        var errorDescription: String? {
            switch self {
            case .commandFailed(let msg): return msg
            case .uvInstallFailed:
                return "Failed to install uv. Please install manually:\n\ncurl -LsSf https://astral.sh/uv/install.sh | sh"
            }
        }
    }
}

