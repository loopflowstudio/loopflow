// Launches and probes the local `lf wave <name>` process backing a WaveChat pane.
//
// Concerto is a viewer, never a participant: launching a wave is the human's act
// through the same door as a terminal — a detached tmux session running
// `lf wave <name>` with the wave's repo as cwd. The session belongs to tmux, not
// to Concerto; quitting the app never kills a wave.

#if os(macOS)
import Foundation
import LoopflowCore

enum WaveLaunchError: LocalizedError, Equatable {
    case alreadyRunning(String)
    case lfNotFound
    case launchFailed(String)

    var errorDescription: String? {
        switch self {
        case .alreadyRunning(let reason):
            return reason
        case .lfNotFound:
            return "Can't find the lf binary — not bundled with Concerto and not on PATH."
        case .launchFailed(let detail):
            return detail
        }
    }
}

enum LocalWaveAgentLauncher {
    static func sessionExists(repoPath: String, waveName: String) -> Bool {
        runCommandSync(
            [
                "tmux",
                "has-session",
                "-t",
                PortfolioRepoState.waveAgentSessionName(repoPath: repoPath, waveName: waveName),
            ],
            logFailure: false
        ) != nil
    }

    /// Start `lf wave <name>` in a detached tmux session so it outlives Concerto.
    /// Refuses when the wave already has a tmux session or a live endpoint —
    /// the server enforces one brain per wave; we just avoid the doomed spawn.
    static func launchWave(repoPath: String, waveName: String) throws {
        let sessionName = PortfolioRepoState.waveAgentSessionName(repoPath: repoPath, waveName: waveName)
        if let reason = launchBlockReason(
            sessionName: sessionName,
            sessionExists: sessionExists(repoPath: repoPath, waveName: waveName),
            endpoint: WaveEndpoint.read(repoPath: repoPath, waveName: waveName)
        ) {
            throw WaveLaunchError.alreadyRunning(reason)
        }
        guard let lfPath = resolveLfBinary() else {
            throw WaveLaunchError.lfNotFound
        }
        let args = waveLaunchCommand(
            lfPath: lfPath,
            sessionName: sessionName,
            repoPath: repoPath,
            waveName: waveName
        )
        try runChecked(args, cwd: repoPath)
    }

    /// Why a launch must not happen, or nil when the way is clear.
    static func launchBlockReason(sessionName: String, sessionExists: Bool, endpoint: String?) -> String? {
        if let endpoint {
            return "Wave already has a live server at \(endpoint)."
        }
        if sessionExists {
            return "tmux session '\(sessionName)' already exists — the wave may still be starting."
        }
        return nil
    }

    /// The detached tmux invocation: session named after the wave, cwd at the
    /// wave's repo, running `lf wave <name>`.
    static func waveLaunchCommand(
        lfPath: String,
        sessionName: String,
        repoPath: String,
        waveName: String
    ) -> [String] {
        ["tmux", "new-session", "-d", "-s", sessionName, "-c", repoPath, lfPath, "wave", waveName]
    }

    /// Resolution order: the lf bundled inside Concerto.app (shipped next to the
    /// bundled lfd), then `lf` on the enriched PATH. Nil means nothing to run.
    static func resolveLfBinary(
        bundled: URL? = Bundle.main.url(forAuxiliaryExecutable: "lf"),
        pathEnv: String = GUIProcessEnvironment.enrichedPath(from: ProcessInfo.processInfo.environment["PATH"]),
        isExecutableFile: (String) -> Bool = { FileManager.default.isExecutableFile(atPath: $0) }
    ) -> String? {
        if let bundled {
            return bundled.path
        }
        for dir in pathEnv.split(separator: ":") where !dir.isEmpty {
            let candidate = "\(dir)/lf"
            if isExecutableFile(candidate) {
                return candidate
            }
        }
        return nil
    }

    // MARK: - Process plumbing

    private static func runChecked(_ args: [String], cwd: String) throws {
        let result = run(args, cwd: cwd)
        guard let result else {
            throw WaveLaunchError.launchFailed("Failed to spawn: \(args.joined(separator: " "))")
        }
        guard result.status == 0 else {
            let detail = result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
            throw WaveLaunchError.launchFailed(
                detail.isEmpty ? "Command failed (\(result.status)): \(args.joined(separator: " "))" : detail
            )
        }
    }

    private static func runCommandSync(
        _ args: [String],
        cwd: String? = nil,
        logFailure: Bool = true
    ) -> String? {
        guard let result = run(args, cwd: cwd) else { return nil }
        guard result.status == 0 else {
            guard logFailure else { return nil }
            LoggingService.lfd(
                "command failed: \(args.joined(separator: " ")) \(result.stderr.trimmingCharacters(in: .whitespacesAndNewlines))"
            )
            return nil
        }
        return result.stdout
    }

    private static func run(
        _ args: [String],
        cwd: String? = nil
    ) -> (status: Int32, stdout: String, stderr: String)? {
        let process = Process()
        let stdout = Pipe()
        let stderr = Pipe()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = args
        process.standardOutput = stdout
        process.standardError = stderr
        process.environment = GUIProcessEnvironment.enriched(ProcessInfo.processInfo.environment)
        if let cwd {
            process.currentDirectoryURL = URL(fileURLWithPath: cwd, isDirectory: true)
        }

        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            return nil
        }

        let stdoutText = String(data: stdout.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        let stderrText = String(data: stderr.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        return (process.terminationStatus, stdoutText, stderrText)
    }
}
#endif
