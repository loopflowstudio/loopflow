// Service for checking and installing loopflow and its dependencies.

import Foundation

struct SetupService {
    struct DependencyStatus {
        var lfInstalled: Bool
        var lfPath: String?
    }

    func checkDependencies() -> DependencyStatus {
        let lfPath = findExecutable("lf")
        return DependencyStatus(
            lfInstalled: lfPath != nil,
            lfPath: lfPath
        )
    }

    /// Install loopflow and all its dependencies
    func install() async throws {
        // Install uv if needed
        if findExecutable("uv") == nil {
            try await installUv()
        }

        guard let uvPath = findExecutable("uv") else {
            throw SetupError.uvInstallFailed
        }

        // Install loopflow via uv
        try await runCommand(uvPath, args: ["tool", "install", "loopflow"])

        // Run lfops install to set up all dependencies
        guard let lfopsPath = findExecutable("lfops") else {
            throw SetupError.commandFailed("lfops not found after installing loopflow")
        }

        try await runCommand(lfopsPath, args: ["install"])
    }

    /// Ensure the loopflow daemon is running
    func ensureDaemonRunning() async throws {
        // Skip if lfd not installed
        guard let lfdPath = findExecutable("lfd") else { return }

        let plistPath = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/LaunchAgents/com.loopflow.lfd.plist")

        if !FileManager.default.fileExists(atPath: plistPath.path) {
            // Install daemon if plist doesn't exist
            try await runCommand(lfdPath, args: ["install"])
        } else if !isDaemonRunning() {
            // Load daemon if installed but not running
            try await runCommand("/bin/launchctl", args: ["load", plistPath.path])
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
