// Launches and probes the local `lf loop <name>` process backing a WaveChat pane.
//
// Loopflow is a viewer, never a participant: launching a wave is the human's act
// through the same door as a terminal — a detached tmux session running
// `lf loop <name>` with the wave's repo as cwd. The session belongs to tmux, not
// to Loopflow; quitting the app never kills a wave.

#if os(macOS)
import Foundation
import Loopflow

enum WaveLaunchError: LocalizedError, Equatable {
    case alreadyRunning(String)
    case noUsableLf(String)
    case launchFailed(String)

    var errorDescription: String? {
        switch self {
        case .alreadyRunning(let reason):
            return reason
        case .noUsableLf(let detail):
            return detail
        case .launchFailed(let detail):
            return detail
        }
    }
}

enum LocalWaveAgentLauncher {
    private static let resolutionCache = ResolvedLfCache()

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

    /// Start `lf loop <name>` in a detached tmux session so it outlives Loopflow.
    /// Refuses when the wave already has a tmux session or a live endpoint —
    /// the server enforces one brain per wave; we just avoid the doomed spawn.
    ///
    /// Wave state lives at the wave's ORIGIN repo (`WaveOrigin`), so a worktree
    /// `repoPath` resolves once up front and that one path feeds everything:
    /// the session name, the endpoint guard, the launch cwd, and the dev-tree
    /// lf candidate. Guard and discovery read the same file by construction.
    static func launchWave(repoPath: String, waveName: String) throws {
        let origin = WaveOrigin.resolve(repoPath)
        let sessionName = PortfolioRepoState.waveAgentSessionName(repoPath: origin, waveName: waveName)
        if let reason = launchBlockReason(
            sessionName: sessionName,
            sessionExists: sessionExists(repoPath: origin, waveName: waveName),
            endpoint: liveEndpoint(
                recorded: WaveEndpoint.read(repoPath: origin, waveName: waveName),
                waveName: waveName
            )
        ) {
            throw WaveLaunchError.alreadyRunning(reason)
        }
        let lfPath = try resolveLoopCapableLf(originRepo: origin)
        let args = waveLaunchCommand(
            lfPath: lfPath,
            sessionName: sessionName,
            repoPath: origin,
            waveName: waveName
        )
        try runChecked(args, cwd: origin)
    }

    /// Why a launch must not happen, or nil when the way is clear. `endpoint`
    /// must be a PROBED address (`liveEndpoint`), never the raw pointer file:
    /// a SIGKILL or power loss leaves the file behind, and blocking on its
    /// mere existence would refuse the Start button forever.
    static func launchBlockReason(sessionName: String, sessionExists: Bool, endpoint: String?) -> String? {
        if let endpoint {
            return "Wave already has a live server at \(endpoint)."
        }
        if sessionExists {
            return "tmux session '\(sessionName)' already exists — the wave may still be starting."
        }
        return nil
    }

    /// The recorded endpoint, but only when a live wave server for `waveName`
    /// answers there. Mirrors Rust `server::live_endpoint`: a missing pointer,
    /// a dead address, or an answer for a different wave is stale — nil, clear
    /// to launch (the new server's own boot floor overwrites the file). Runs
    /// only on the launch click path, never on the 1s status poll.
    static func liveEndpoint(
        recorded: String?,
        waveName: String,
        probe: (String) -> String? = healthWaveName
    ) -> String? {
        guard let recorded else { return nil }
        return probe(recorded) == waveName ? recorded : nil
    }

    /// The `wave` a server at `endpoint` reports on `GET /health`, or nil when
    /// nothing answers within 2s (mirrors Rust `ENDPOINT_PROBE_TIMEOUT`).
    /// Probe contract: the guard keys on the `wave` field only. `/health`'s
    /// `status` is channel liveness (`serving`) and `loop_state` is the resident's
    /// state — a wave whose loop failed still answers here and still blocks a
    /// second launch, which is correct: the channel is live.
    static func healthWaveName(endpoint: String) -> String? {
        guard let url = URL(string: "http://\(endpoint)/health") else { return nil }
        var request = URLRequest(url: url)
        request.timeoutInterval = 2
        let semaphore = DispatchSemaphore(value: 0)
        nonisolated(unsafe) var name: String?
        URLSession.shared.dataTask(with: request) { data, response, _ in
            defer { semaphore.signal() }
            guard let http = response as? HTTPURLResponse, http.statusCode == 200,
                  let data,
                  let body = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
            else { return }
            name = body["wave"] as? String
        }.resume()
        semaphore.wait()
        return name
    }

    /// The detached tmux invocation: session named after the wave, cwd at the
    /// wave's repo, running `lf loop <name>`.
    static func waveLaunchCommand(
        lfPath: String,
        sessionName: String,
        repoPath: String,
        waveName: String
    ) -> [String] {
        ["tmux", "new-session", "-d", "-s", sessionName, "-c", repoPath, lfPath, "loop", waveName]
    }

    /// Candidate lf binaries in trust order: the lf bundled inside Loopflow.app
    /// (shipped next to the bundled lfd), each `lf` on the enriched PATH, then
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

    /// First candidate that actually has the `loop` subcommand. Resolving `lf`
    /// from PATH can find a build that predates `lf loop`; it launches fine,
    /// exits instantly, and the UI sits on a dead 20s wait (observed live) —
    /// so every candidate is capability-probed before it's trusted.
    ///
    /// The probe is `lf help loop`, NOT `lf loop --help`: lf's arg reorderer
    /// treats an unknown `loop` as a skill name, so `lf loop --help` prints the
    /// root help and exits 0 even on a build without the subcommand. `lf help
    /// loop` exits 0 only when the subcommand exists, and clap answers it without touching
    /// any wave state.
    static func resolveLoopCapableLf(
        originRepo: String,
        bundled: URL? = Bundle.main.url(forAuxiliaryExecutable: "lf"),
        pathEnv: String = GUIProcessEnvironment.enrichedPath(from: ProcessInfo.processInfo.environment["PATH"]),
        isExecutableFile: (String) -> Bool = { FileManager.default.isExecutableFile(atPath: $0) },
        probe: (String) -> Bool = hasLoopCommand,
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
            "No lf with the loop command. Rejected (each failed `lf help loop`, "
                + "so it predates `lf loop`): " + candidates.joined(separator: ", ")
        )
    }

    /// `lf help loop` exits 0 only when this build knows the subcommand.
    static func hasLoopCommand(lfPath: String) -> Bool {
        run([lfPath, "help", "loop"])?.status == 0
    }

    static func tmuxSessionNames() -> Set<String> {
        guard let result = run(["tmux", "list-sessions", "-F", "#S"]) else {
            return []
        }
        guard result.status == 0 else {
            return []
        }
        return Set(result.stdout
            .components(separatedBy: .newlines)
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty })
    }

    /// Run an `lf` query verb (`ls`, `status`, `runs`, …) and return its
    /// stdout. Backs `RegistryQuery` on macOS: the wave dashboard reads durable
    /// facts by shelling the daemonless `lf` over `lfdb`, not by streaming a
    /// center. Resolves the same wave-capable `lf` the launcher trusts (a build
    /// old enough to lack `lf loop` also lacks these verbs), then execs it with
    /// the enriched GUI PATH. Throws on a spawn failure or a non-zero exit.
    static func queryLf(_ subargs: [String], cwd: String?) throws -> String {
        let origin = cwd.map(WaveOrigin.resolve) ?? FileManager.default.currentDirectoryPath
        let lfPath = try resolveLoopCapableLf(originRepo: origin)
        guard let result = run([lfPath] + subargs, cwd: cwd) else {
            throw WaveLaunchError.launchFailed("Failed to spawn: lf \(subargs.joined(separator: " "))")
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
