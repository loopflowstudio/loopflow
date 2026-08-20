// Starts and controls the local Wave backing a WaveChat pane.
//
// Loopflow uses the same `lf start <name>` lifecycle as the CLI. lfd owns the
// detached listener; quitting the app never kills a wave.

#if os(macOS)
import Foundation
import Loopflow

enum WaveLaunchError: LocalizedError, Equatable {
    case noUsableLf(String)
    case launchFailed(String)

    var errorDescription: String? {
        switch self {
        case .noUsableLf(let detail):
            return detail
        case .launchFailed(let detail):
            return detail
        }
    }
}

struct TaskStartReceipt: Decodable, Sendable, Equatable {
    let issueIdentifier: String
    let project: String
    let wave: String
}

enum LocalWaveAgentLauncher {
    private static let resolutionCache = ResolvedLfCache()

    /// Stop the listener through the same `lf` lifecycle surface the CLI uses.
    /// The server performs resident, registry, and discovery-file cleanup.
    static func stopWave(repoPath: String, waveName: String) throws {
        let origin = WaveOrigin.resolve(repoPath)
        let lfPath = try resolveWaveCapableLf(originRepo: origin)
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
        let lfPath = try resolveWaveCapableLf(originRepo: origin)
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
        let lfPath = try resolveWaveCapableLf(originRepo: origin)
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
        let lfPath = try resolveWaveCapableLf(originRepo: origin)
        try runChecked(taskResumeCommand(lfPath: lfPath, issue: issue), cwd: origin)
    }

    /// Queue the audited Task interrupt. The Task runner decides how the live
    /// provider turn is stopped and records the receipt in the shared store.
    static func interruptTask(repoPath: String, issue: String) throws {
        let origin = WaveOrigin.resolve(repoPath)
        let lfPath = try resolveWaveCapableLf(originRepo: origin)
        try runChecked(taskInterruptCommand(lfPath: lfPath, issue: issue), cwd: origin)
    }

    /// Open the branch's PR for human review from `worktree`. This delegates to
    /// `lf pr open` — the single presentation boundary — instead of building a
    /// GitHub URL and opening it here, so any later review-surface preference is
    /// honored in one place. Only an explicit user review action calls this;
    /// background app work publishes with `lf pr publish`.
    static func reviewPullRequest(worktree: String) throws {
        let origin = WaveOrigin.resolve(worktree)
        let lfPath = try resolveWaveCapableLf(originRepo: origin)
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
            throw WaveLaunchError.launchFailed(
                "lf task start returned an invalid receipt: \(error.localizedDescription)"
            )
        }
    }

    static func taskResumeCommand(lfPath: String, issue: String) -> [String] {
        [lfPath, "task", "resume", issue]
    }

    static func taskInterruptCommand(lfPath: String, issue: String) -> [String] {
        [lfPath, "task", "interrupt", issue]
    }

    /// Candidate lf binaries in trust order: the lf bundled inside Loopflow.app,
    /// each `lf` on the enriched PATH, then
    /// `<origin>/target/release/lf` — the dev-tree build, for a Loopflow pointed
    /// at a loopflow checkout where the freshest lf is the one just compiled.
    static func lfCandidates(
        originRepo: String,
        bundled: URL?,
        pathEnv: String,
        isExecutableFile: (String) -> Bool
    ) -> [String] {
        var candidates: [String] = []
        if let bundled {
            candidates.append(bundled.path)
        }
        for dir in pathEnv.split(separator: ":") where !dir.isEmpty {
            let candidate = "\(dir)/lf"
            if isExecutableFile(candidate), !candidates.contains(candidate) {
                candidates.append(candidate)
            }
        }
        let devBuild = "\(originRepo)/target/release/lf"
        if isExecutableFile(devBuild), !candidates.contains(devBuild) {
            candidates.append(devBuild)
        }
        return candidates
    }

    /// First candidate that has the Wave lifecycle and turn-intent commands. Resolving `lf`
    /// from PATH can find a build that predates one of them; treating an
    /// unknown lifecycle verb as a skill would launch the wrong work, so every
    /// candidate is capability-probed before it's trusted.
    ///
    /// The probes use `lf help <verb>`, not `<verb> --help`: lf's arg reorderer
    /// may treat an unknown lifecycle verb as a skill name. Clap's help command
    /// answers without touching wave state.
    static func resolveWaveCapableLf(
        originRepo: String,
        bundled: URL? = Bundle.main.url(forAuxiliaryExecutable: "lf"),
        pathEnv: String = GUIProcessEnvironment.enrichedPath(from: ProcessInfo.processInfo.environment["PATH"]),
        isExecutableFile: (String) -> Bool = { FileManager.default.isExecutableFile(atPath: $0) },
        probe: (String) -> Bool = hasWaveCommands,
        useCache: Bool = true
    ) throws -> String {
        let cacheKey = "\(originRepo)|\(bundled?.path ?? "")|\(pathEnv)"
        if useCache,
           let cached = resolutionCache.get(cacheKey),
           isExecutableFile(cached) {
            return cached
        }

        let candidates = lfCandidates(
            originRepo: originRepo,
            bundled: bundled,
            pathEnv: pathEnv,
            isExecutableFile: isExecutableFile
        )
        guard !candidates.isEmpty else {
            throw WaveLaunchError.noUsableLf(
                "Can't find an lf binary — not bundled with Loopflow and not on PATH."
            )
        }
        for candidate in candidates where probe(candidate) {
            if useCache {
                resolutionCache.set(candidate, for: cacheKey)
            }
            return candidate
        }
        throw WaveLaunchError.noUsableLf(
            "No lf with the Wave control commands. Rejected (each failed at least one of "
                + "`lf help start`, `lf help stop`, `lf help pause`, or `lf help resume`): "
                + candidates.joined(separator: ", ")
        )
    }

    /// Help probes parse without touching wave state.
    static func hasWaveCommands(lfPath: String) -> Bool {
        run([lfPath, "help", "start"])?.status == 0
            && run([lfPath, "help", "stop"])?.status == 0
            && run([lfPath, "help", "pause"])?.status == 0
            && run([lfPath, "help", "resume"])?.status == 0
    }

    /// Run an `lf` query verb (`ls`, `status`, `runs`, …) and return its
    /// stdout. Backs `RegistryQuery` on macOS: the wave dashboard reads durable
    /// facts by shelling the daemonless `lf` over the local store, not by streaming
    /// a center. Resolves the same wave-capable `lf` the launcher trusts (a build
    /// old enough to lack `lf start` also lacks these verbs), then execs it with
    /// the enriched GUI PATH. Throws on a spawn failure or a non-zero exit.
    static func queryLf(_ subargs: [String], cwd: String?) throws -> String {
        let origin = cwd.map(WaveOrigin.resolve) ?? FileManager.default.currentDirectoryPath
        let lfPath = try resolveWaveCapableLf(originRepo: origin)
        guard let result = run([lfPath] + subargs, cwd: cwd) else {
            throw WaveLaunchError.launchFailed("Failed to spawn: lf \(subargs.joined(separator: " "))")
        }
        // `lf doctor --json` exits 1 when a check fails, but its stdout is the
        // report the telemetry dashboard must show. A red monitor cannot hide
        // itself behind the query runner's generic non-zero handling.
        if subargs == ["doctor", "--json"], !result.stdout.isEmpty {
            return result.stdout
        }
        guard result.status == 0 else {
            let detail = result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
            throw WaveLaunchError.launchFailed(
                detail.isEmpty ? "lf \(subargs.joined(separator: " ")) failed (\(result.status))" : detail
            )
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
            throw WaveLaunchError.launchFailed("Failed to spawn: \(args.joined(separator: " "))")
        }
        guard result.status == 0 else {
            let detail = result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
            throw WaveLaunchError.launchFailed(
                detail.isEmpty ? "Command failed (\(result.status)): \(args.joined(separator: " "))" : detail
            )
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

private final class ResolvedLfCache: @unchecked Sendable {
    private let lock = NSLock()
    private var resolvedLfByKey: [String: String] = [:]

    func get(_ key: String) -> String? {
        withLock { resolvedLfByKey[key] }
    }

    func set(_ value: String, for key: String) {
        withLock {
            resolvedLfByKey[key] = value
        }
    }

    private func withLock<T>(_ body: () -> T) -> T {
        lock.lock()
        defer { lock.unlock() }
        return body()
    }
}
#endif
