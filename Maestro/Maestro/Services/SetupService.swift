// Service for checking and installing dependencies and daemon.

import Foundation

struct SetupService {
    struct DependencyStatus {
        var lfInstalled: Bool
        var wtInstalled: Bool
        var daemonInstalled: Bool
        var daemonRunning: Bool
        var lfPath: String?
        var wtPath: String?

        var allInstalled: Bool {
            lfInstalled && wtInstalled
        }
    }

    private let plistPath = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent("Library/LaunchAgents/com.loopflow.lfd.plist")

    func checkDependencies() -> DependencyStatus {
        let lfPath = findExecutable("lf")
        let wtPath = findExecutable("wt")
        let daemonInstalled = FileManager.default.fileExists(atPath: plistPath.path)
        let daemonRunning = isDaemonRunning()

        return DependencyStatus(
            lfInstalled: lfPath != nil,
            wtInstalled: wtPath != nil,
            daemonInstalled: daemonInstalled,
            daemonRunning: daemonRunning,
            lfPath: lfPath,
            wtPath: wtPath
        )
    }

    func installLoopflow() async throws {
        try await runCommand("/bin/zsh", args: ["-l", "-c", "pip install loopflow"])
    }

    func installWt() async throws {
        try await runCommand("/bin/zsh", args: ["-l", "-c", "lf ops install"])
    }

    func installDaemon() async throws {
        try await runCommand("/bin/zsh", args: ["-l", "-c", "lfd install"])
    }

    func uninstallDaemon() async throws {
        try await runCommand("/bin/zsh", args: ["-l", "-c", "lfd uninstall"])
    }

    func ensureDaemonRunning() async throws {
        let status = checkDependencies()

        // Skip if lf not installed
        guard status.lfInstalled else { return }

        if !status.daemonInstalled {
            try await installDaemon()
        } else if !status.daemonRunning {
            // Restart daemon if installed but not running
            try await runCommand("launchctl", args: ["load", plistPath.path])
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

    private func findExecutable(_ name: String) -> String? {
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
                    return path
                }
            }
        } catch {
            // Fall through
        }
        return nil
    }

    private func runCommand(_ executable: String, args: [String]) async throws {
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

                if process.terminationStatus == 0 {
                    continuation.resume()
                } else {
                    let data = pipe.fileHandleForReading.readDataToEndOfFile()
                    let output = String(data: data, encoding: .utf8) ?? "Unknown error"
                    continuation.resume(throwing: SetupError.commandFailed(output))
                }
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }

    enum SetupError: LocalizedError {
        case commandFailed(String)

        var errorDescription: String? {
            switch self {
            case .commandFailed(let msg): return msg
            }
        }
    }
}
