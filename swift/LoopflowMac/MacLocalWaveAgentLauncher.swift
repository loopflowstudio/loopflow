// Starts and controls the local Wave backing a WaveChat pane.
//
// Loopflow uses the same `lf start <name>` lifecycle as the CLI. lfd owns the
// detached listener; quitting the app never kills a wave.

#if os(macOS)
import Foundation
import Loopflow

struct LocalLfError: LocalizedError {
    let errorDescription: String?
}

struct TaskStartReceipt: Decodable, Sendable, Equatable {
    let issueIdentifier: String
    let project: String
    let wave: String
}

enum LocalWaveAgentLauncher {
    /// Stop the listener through the same `lf` lifecycle surface the CLI uses.
    /// The server performs resident, registry, and discovery-file cleanup.
    static func stopWave(repoPath: String, waveName: String) throws {
        let origin = WaveOrigin.resolve(repoPath)
        let lfPath = try controlLfPath()
        try runChecked(waveStopCommand(lfPath: lfPath, waveName: waveName), cwd: origin)
    }

    static func waveStopCommand(lfPath: String, waveName: String) -> [String] {
        [lfPath, "stop", waveName]
    }

    /// Start a filed Task through the same durable lifecycle command as the CLI.
    /// `lf task run` owns Project Work startup, worktree placement, and the
    /// Task process; the app does not reproduce any of those decisions.
    static func runTask(repoPath: String, issue: String) throws {
        let origin = WaveOrigin.resolve(repoPath)
        let lfPath = try controlLfPath()
        try runChecked(taskRunCommand(lfPath: lfPath, issue: issue), cwd: origin)
    }

    /// Create and start one Task with the normal PM and worker lifecycle.
    static func startTask(
        repoPath: String,
        title: String,
        project: String,
        directive: String
    ) throws -> TaskStartReceipt {
        let origin = WaveOrigin.resolve(repoPath)
        let lfPath = try controlLfPath()
        let stdout = try runCheckedOutput(
            taskStartCommand(
                lfPath: lfPath,
                title: title,
                project: project,
                directive: directive
            ),
            cwd: origin
        )
        return try taskStartReceipt(stdout)
    }

    /// Restart existing Task Work without creating another worktree or
    /// status record.
    static func resumeTask(repoPath: String, issue: String) throws {
        let origin = WaveOrigin.resolve(repoPath)
        let lfPath = try controlLfPath()
        try runChecked(taskResumeCommand(lfPath: lfPath, issue: issue), cwd: origin)
    }

    /// Queue the audited Task interrupt. The Task runner decides how the live
    /// provider turn is stopped and records the receipt in the shared store.
    static func interruptTask(repoPath: String, issue: String) throws {
        let origin = WaveOrigin.resolve(repoPath)
        let lfPath = try controlLfPath()
        try runChecked(taskInterruptCommand(lfPath: lfPath, issue: issue), cwd: origin)
    }

    /// Open the branch's PR for human review from `worktree`. This delegates to
    /// `lf pr open` — the single presentation boundary — instead of building a
    /// GitHub URL and opening it here, so any later review-surface preference is
    /// honored in one place. Only an explicit user review action calls this;
    /// background app work publishes with `lf pr publish`.
    static func reviewPullRequest(worktree: String) throws {
        let lfPath = try controlLfPath()
        try runChecked(pullRequestReviewCommand(lfPath: lfPath), cwd: worktree)
    }

    static func pullRequestReviewCommand(lfPath: String) -> [String] {
        [lfPath, "pr", "open"]
    }

    static func taskRunCommand(lfPath: String, issue: String) -> [String] {
        [lfPath, "task", "run", issue]
    }

    static func taskStartCommand(
        lfPath: String,
        title: String,
        project: String,
        directive: String
    ) -> [String] {
        [
            lfPath, "task", "start", project, title,
            "--directive", directive,
            "--json",
        ]
    }

    static func taskStartReceipt(_ stdout: String) throws -> TaskStartReceipt {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        do {
            return try decoder.decode(TaskStartReceipt.self, from: Data(stdout.utf8))
        } catch {
            throw LocalLfError(
                errorDescription: "lf task start returned an invalid receipt: \(error.localizedDescription)"
            )
        }
    }

    static func taskResumeCommand(lfPath: String, issue: String) -> [String] {
        [lfPath, "task", "resume", issue]
    }

    static func taskInterruptCommand(lfPath: String, issue: String) -> [String] {
        [lfPath, "task", "interrupt", issue]
    }

    /// Return the active machine control binary, or the bundled offline fallback.
    ///
    /// Loopflow's enriched GUI PATH leads with `~/.local/bin`, whose `lf` is the
    /// machine entry gate. It dispatches to the binary that owns the selected
    /// installed Home, including an installed development store with draft
    /// migrations a release-provenance helper cannot interpret.
    static func controlLfPath(
        searchPath: String = GUIProcessEnvironment.enrichedPath(
            from: ProcessInfo.processInfo.environment["PATH"]
        ),
        bundled: URL? = Bundle.main.url(forAuxiliaryExecutable: "lf")
    ) throws -> String {
        for directory in searchPath.split(separator: ":") {
            let candidate = URL(fileURLWithPath: String(directory), isDirectory: true)
                .appendingPathComponent("lf", isDirectory: false)
            if FileManager.default.isExecutableFile(atPath: candidate.path) {
                return candidate.path
            }
        }
        return try bundledLfPath(bundled: bundled)
    }

    /// Return the control binary shipped with this exact Mac client build.
    static func bundledLfPath(
        bundled: URL? = Bundle.main.url(forAuxiliaryExecutable: "lf")
    ) throws -> String {
        guard let bundled, FileManager.default.isExecutableFile(atPath: bundled.path) else {
            throw LocalLfError(
                errorDescription: "Loopflow.app is missing its executable bundled lf helper at Contents/MacOS/lf. "
                    + "No active machine lf was found; rebuild or reinstall Loopflow."
            )
        }
        return bundled.path
    }

    /// Run an `lf` query verb (`ls`, `status`, `runs`, …) and return its
    /// stdout. Backs `RegistryQuery` on macOS: the wave dashboard reads durable
    /// facts through the machine's active daemonless `lf`, not by streaming a
    /// center. Throws on a spawn failure or a non-zero exit.
    static func queryLf(_ subargs: [String], cwd: String?) throws -> String {
        let lfPath = try controlLfPath()
        guard let result = run([lfPath] + subargs, cwd: cwd) else {
            throw LocalLfError(
                errorDescription: "Failed to spawn: lf \(subargs.joined(separator: " "))"
            )
        }
        // `lf doctor --json` exits 1 when a check fails, but its stdout is the
        // report the telemetry dashboard must show. A red monitor cannot hide
        // itself behind the query runner's generic non-zero handling.
        if subargs == ["doctor", "--json"], !result.stdout.isEmpty {
            return result.stdout
        }
        guard result.status == 0 else {
            let detail = result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
            let message = detail.isEmpty
                ? "lf \(subargs.joined(separator: " ")) failed (\(result.status))"
                : detail
            throw LocalLfError(errorDescription: message)
        }
        return result.stdout
    }

    // MARK: - Process plumbing

    private static func runChecked(_ args: [String], cwd: String) throws {
        _ = try runCheckedOutput(args, cwd: cwd)
    }

    private static func runCheckedOutput(_ args: [String], cwd: String) throws -> String {
        let result = run(args, cwd: cwd)
        guard let result else {
            throw LocalLfError(
                errorDescription: "Failed to spawn: \(args.joined(separator: " "))"
            )
        }
        guard result.status == 0 else {
            let detail = result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
            let message = detail.isEmpty
                ? "Command failed (\(result.status)): \(args.joined(separator: " "))"
                : detail
            throw LocalLfError(errorDescription: message)
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

        let outHandle = stdout.fileHandleForReading
        let errHandle = stderr.fileHandleForReading

        do {
            try process.run()
        } catch {
            return nil
        }

        // Drain both pipes while the child is still writing. A pipe holds 64KB;
        // waiting for exit first deadlocks the moment a command says more than
        // that, and `lf tokens --json` says about 120KB. `lf runs`/`lf doctor`
        // are small, which is why this only ever bit the largest reader.
        let collector = OutputCollector()
        let group = DispatchGroup()
        let queue = DispatchQueue.global(qos: .userInitiated)
        queue.async(group: group) { collector.setStdout(outHandle.readDataToEndOfFile()) }
        queue.async(group: group) { collector.setStderr(errHandle.readDataToEndOfFile()) }

        process.waitUntilExit()
        group.wait()

        return (
            process.terminationStatus,
            String(data: collector.stdout, encoding: .utf8) ?? "",
            String(data: collector.stderr, encoding: .utf8) ?? ""
        )
    }
}

/// Two reader threads, one box. The pipes must be drained concurrently with the
/// child's execution, so their results cross a thread boundary.
private final class OutputCollector: @unchecked Sendable {
    private let lock = NSLock()
    private var out = Data()
    private var err = Data()

    var stdout: Data { lock.withLock { out } }
    var stderr: Data { lock.withLock { err } }

    func setStdout(_ data: Data) { lock.withLock { out = data } }
    func setStderr(_ data: Data) { lock.withLock { err = data } }
}

#endif
