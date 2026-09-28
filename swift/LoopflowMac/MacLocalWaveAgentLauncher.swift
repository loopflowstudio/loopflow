// Launch Tasks and local sessions through the CLI.

#if os(macOS)
import Foundation
import Loopflow

struct LocalLfError: LocalizedError {
    let errorDescription: String?
}

struct TaskCreateReceipt: Decodable, Sendable, Equatable {
    let issueIdentifier: String
    let project: String
    let wave: String
}

private struct DevelopmentControlConfig: Decodable {
    let lfPath: String
}

enum LocalWaveAgentLauncher {
    /// Start a filed Task through the same bounded worker command as the CLI.
    /// `lf task run` owns Project lookup, worktree placement, Flow selection,
    /// and the Task worker; the app does not reproduce those decisions.
    static func runTask(repoPath: String, issue: String) throws {
        let origin = WaveOrigin.resolve(repoPath)
        let lfPath = try controlLfPath()
        try runChecked(taskRunCommand(lfPath: lfPath, issue: issue), cwd: origin)
    }

    /// Create one Task, then start its normal bounded worker path.
    static func startTask(
        repoPath: String,
        title: String,
        wave: String,
        directive: String
    ) throws -> TaskCreateReceipt {
        let origin = WaveOrigin.resolve(repoPath)
        let lfPath = try controlLfPath()
        let stdout = try runCheckedOutput(
            taskCreateCommand(
                lfPath: lfPath,
                title: title,
                wave: wave,
                directive: directive
            ),
            cwd: origin
        )
        return try taskCreateReceipt(stdout)
    }

    /// Queue the audited Task interrupt. The Task worker decides how the live
    /// provider turn is stopped and records the receipt in the shared store.
    static func interruptTask(repoPath: String, issue: String) throws {
        let origin = WaveOrigin.resolve(repoPath)
        let lfPath = try controlLfPath()
        try runChecked(taskInterruptCommand(lfPath: lfPath, issue: issue), cwd: origin)
    }

    /// Open the branch's PR for review from `worktree`. This delegates to
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

    /// Ensure Task Work and its checkout without starting a worker; returns
    /// the authoritative worktree.
    static func checkoutTask(repoPath: String, issue: String) throws -> String {
        let stdout = try runCheckedOutput(taskCheckoutCommand(lfPath: try controlLfPath(), issue: issue), cwd: repoPath)
        return try taskCheckoutWorktree(stdout)
    }

    static func taskCheckoutCommand(lfPath: String, issue: String) -> [String] {
        [lfPath, "task", "checkout", issue, "--json"]
    }

    static func taskCheckoutWorktree(_ stdout: String) throws -> String {
        struct Prepared: Decodable { let worktree: String }
        do {
            return try JSONDecoder().decode(Prepared.self, from: Data(stdout.utf8)).worktree
        } catch {
            throw LocalLfError(
                errorDescription: "lf task checkout returned an invalid receipt: \(error.localizedDescription)"
            )
        }
    }

    static func taskRunCommand(lfPath: String, issue: String) -> [String] {
        [lfPath, "task", "run", issue]
    }

    static func taskCreateCommand(
        lfPath: String,
        title: String,
        wave: String,
        directive: String
    ) -> [String] {
        [
            lfPath, "task", "create", "--run", "--wave", wave, "--title", title,
            "--notes", directive,
            "--json",
        ]
    }

    static func taskCreateReceipt(_ stdout: String) throws -> TaskCreateReceipt {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        do {
            return try decoder.decode(TaskCreateReceipt.self, from: Data(stdout.utf8))
        } catch {
            throw LocalLfError(
                errorDescription: "lf task create --run returned an invalid receipt: \(error.localizedDescription)"
            )
        }
    }

    static func taskInterruptCommand(lfPath: String, issue: String) -> [String] {
        [lfPath, "task", "interrupt", issue]
    }

    /// Return the CLI that owns the Home this Mac client controls.
    ///
    /// Installed apps use their registered bundled helper. A development app
    /// carries an explicit pointer to the machine install gate so Finder and
    /// scripted launches see the same selected Home as terminal `lf`.
    static func controlLfPath(
        bundled: URL? = Bundle.main.url(forAuxiliaryExecutable: "lf"),
        developmentConfig: URL? = Bundle.main.url(
            forResource: "LoopflowDevControl",
            withExtension: "json"
        )
    ) throws -> String {
        if let developmentConfig {
            let config: DevelopmentControlConfig
            do {
                let data = try Data(contentsOf: developmentConfig)
                let decoder = JSONDecoder()
                decoder.keyDecodingStrategy = .convertFromSnakeCase
                config = try decoder.decode(DevelopmentControlConfig.self, from: data)
            } catch {
                throw LocalLfError(
                    errorDescription: "Loopflow Dev has an invalid machine control configuration: "
                        + error.localizedDescription
                )
            }
            guard FileManager.default.isExecutableFile(atPath: config.lfPath) else {
                throw LocalLfError(
                    errorDescription: "Loopflow Dev's machine lf is missing or not executable: "
                        + config.lfPath
                )
            }
            return config.lfPath
        }
        guard let bundled, FileManager.default.isExecutableFile(atPath: bundled.path) else {
            throw LocalLfError(
                errorDescription: "Loopflow.app is missing its executable bundled lf helper at Contents/MacOS/lf. "
                    + "Rebuild or reinstall Loopflow; PATH fallback is disabled."
            )
        }
        return bundled.path
    }

    /// Run an `lf` query verb (`ls`, `status`, `runs`, …) and return its
    /// stdout. Backs `RegistryQuery` on macOS: the wave dashboard reads durable
    /// facts by shelling the daemonless Home `lf` over the local store, not
    /// by streaming a center. Throws on a spawn failure or a non-zero exit.
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

    static func queryProcess(_ args: [String], cwd: String? = nil) -> Process {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = args
        process.environment = GUIProcessEnvironment.enriched(ProcessInfo.processInfo.environment)
        if let cwd {
            process.currentDirectoryURL = URL(fileURLWithPath: cwd, isDirectory: true)
        }
        return process
    }

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
        let process = queryProcess(args, cwd: cwd)
        let stdout = Pipe()
        let stderr = Pipe()
        process.standardOutput = stdout
        process.standardError = stderr

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
