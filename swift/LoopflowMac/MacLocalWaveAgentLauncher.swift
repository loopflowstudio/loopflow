// Launch Tasks and local sessions through the CLI.

#if os(macOS)
import Foundation
import Loopflow

struct LocalLfError: LocalizedError {
    let errorDescription: String?
}

private struct DevelopmentControlConfig: Decodable {
    let lfPath: String
}

enum LocalWaveAgentLauncher {
    /// Run a filed Task's default Flow. `lf task run` owns Project lookup,
    /// worktree placement and Flow selection; the app does not reproduce them.
    static func runTask(repoPath: String, issue: String) throws {
        try startLf(taskRunArguments(issue: issue), cwd: WaveOrigin.resolve(repoPath))
    }

    /// Queue the audited Task interrupt. The live run observes it, stops its
    /// provider turn and records the receipt in the shared store.
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

    /// Ensure Task Work and its checkout without running a Flow; returns
    /// the authoritative worktree.
    static func checkoutTask(repoPath: String, issue: String) throws -> WorkspaceIdentity {
        let stdout = try runCheckedOutput(taskCheckoutCommand(lfPath: try controlLfPath(), issue: issue), cwd: repoPath)
        return try taskCheckoutWorkspace(stdout)
    }

    static func taskCheckoutCommand(lfPath: String, issue: String) -> [String] {
        [lfPath, "task", "checkout", issue, "--json"]
    }

    static func taskCheckoutWorkspace(_ stdout: String) throws -> WorkspaceIdentity {
        do {
            return try JSONDecoder().decode(WorkspaceIdentity.self, from: Data(stdout.utf8))
        } catch {
            throw LocalLfError(
                errorDescription: "lf task checkout returned an invalid receipt: \(error.localizedDescription)"
            )
        }
    }

    static func taskRunArguments(issue: String) -> [String] {
        ["-b", "task", "run", issue]
    }

    static func taskInterruptCommand(lfPath: String, issue: String) -> [String] {
        [lfPath, "task", "interrupt", issue]
    }

    /// Return the CLI that owns the Home this Mac client controls.
    ///
    /// Use an explicitly configured helper when present; otherwise use the
    /// bundled CLI, which shares the app's protocol and selects its own Home.
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

    /// Run an `lf` query verb (`list`, `status`, `monitor`, …) and return its
    /// stdout. Backs `RegistryQuery` on macOS: the wave dashboard reads durable
    /// facts by shelling the daemonless Home `lf` over the local store, not
    /// by streaming a center. Throws on a spawn failure or a non-zero exit.
    static func queryLf(_ subargs: [String], cwd: String?, input: String? = nil) throws -> String {
        let lfPath = try controlLfPath()
        guard let result = run([lfPath] + subargs, cwd: cwd, input: input) else {
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

    /// Start an `lf` command that runs for as long as its work does, such as
    /// `lf -b task run`, and leave it running as this app's child. A refusal
    /// exits within moments and is thrown; later output stays in lf's own logs.
    static func startLf(_ subargs: [String], cwd: String?) throws {
        let process = queryProcess([try controlLfPath()] + subargs, cwd: cwd)
        let log = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-start-\(UUID().uuidString).log")
        FileManager.default.createFile(atPath: log.path, contents: nil)
        let output = try FileHandle(forWritingTo: log)
        defer { try? output.close() }
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = output
        process.standardError = output
        do { try process.run() } catch {
            throw LocalLfError(errorDescription: "Failed to spawn: lf \(subargs.joined(separator: " "))")
        }
        let deadline = Date().addingTimeInterval(startRefusalWindow)
        while process.isRunning, Date() < deadline { Thread.sleep(forTimeInterval: 0.05) }
        guard !process.isRunning, process.terminationStatus != 0 else { return }
        let detail = (try? String(contentsOf: log, encoding: .utf8))?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        try? FileManager.default.removeItem(at: log)
        throw LocalLfError(errorDescription: detail.isEmpty
            ? "lf \(subargs.joined(separator: " ")) failed (\(process.terminationStatus))"
            : detail)
    }

    /// How long a started command is watched for an immediate refusal.
    private static let startRefusalWindow: TimeInterval = 3

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
        cwd: String? = nil,
        input: String? = nil
    ) -> (status: Int32, stdout: String, stderr: String)? {
        let process = queryProcess(args, cwd: cwd)
        let stdout = Pipe()
        let stderr = Pipe()
        process.standardOutput = stdout
        process.standardError = stderr
        let stdin = input.map { _ in Pipe() }
        process.standardInput = stdin

        let outHandle = stdout.fileHandleForReading
        let errHandle = stderr.fileHandleForReading

        do {
            try process.run()
        } catch {
            return nil
        }

        // Drain both pipes while the child is still writing. A pipe holds 64KB;
        // waiting for exit first deadlocks the moment a command says more than
        // that, and `lf repo tokens --json` says about 120KB. `lf usage --days 0 --task ID --json`/`lf doctor`
        // are small, which is why this only ever bit the largest reader.
        let collector = OutputCollector()
        let group = DispatchGroup()
        let queue = DispatchQueue.global(qos: .userInitiated)
        queue.async(group: group) { collector.setStdout(outHandle.readDataToEndOfFile()) }
        queue.async(group: group) { collector.setStderr(errHandle.readDataToEndOfFile()) }

        if let input, let stdin {
            // lf can exit before reading; report the failed write instead of taking SIGPIPE.
            _ = fcntl(stdin.fileHandleForWriting.fileDescriptor, F_SETNOSIGPIPE, 1)
            queue.async(group: group) {
                defer { try? stdin.fileHandleForWriting.close() }
                do { try stdin.fileHandleForWriting.write(contentsOf: Data(input.utf8)) }
                catch { collector.setInputError(error.localizedDescription) }
            }
        }
        process.waitUntilExit()
        group.wait()
        if process.terminationStatus == 0, let error = collector.inputError {
            return (1, "", "Could not send draft to lf: \(error)")
        }

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
    private var inputFailure: String?

    var inputError: String? { lock.withLock { inputFailure } }
    func setInputError(_ message: String) { lock.withLock { inputFailure = message } }

    var stdout: Data { lock.withLock { out } }
    var stderr: Data { lock.withLock { err } }

    func setStdout(_ data: Data) { lock.withLock { out = data } }
    func setStderr(_ data: Data) { lock.withLock { err = data } }
}

#endif
