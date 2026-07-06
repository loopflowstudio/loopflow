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
    /// A live server is the one hard block (one brain per wave). A tmux session
    /// that exists with no live server is a ghost — a crashed `lf wave` whose
    /// session lingered — and it is reclaimed (killed) so the Start button
    /// isn't dead forever. To avoid reclaiming a wave that's still coming up,
    /// the ghost verdict is taken only after a short grace probe.
    ///
    /// Wave state lives at the wave's ORIGIN repo (`WaveOrigin`), so a worktree
    /// `repoPath` resolves once up front and that one path feeds everything:
    /// the session name, the endpoint guard, the launch cwd, and the dev-tree
    /// lf candidate. Guard and discovery read the same file by construction.
    static func launchWave(repoPath: String, waveName: String) throws {
        let origin = WaveOrigin.resolve(repoPath)
        let sessionName = PortfolioRepoState.waveAgentSessionName(repoPath: origin, waveName: waveName)
        let exists = sessionExists(repoPath: origin, waveName: waveName)
        // A lingering session with no endpoint is either mid-boot or a ghost;
        // grace-probe (3 tries) so we don't kill a wave that's still
        // publishing. No session means one probe is enough.
        let endpoint = awaitLiveEndpoint(origin: origin, waveName: waveName, attempts: exists ? 3 : 1)

        switch launchAction(sessionName: sessionName, sessionExists: exists, endpoint: endpoint) {
        case .blocked(let reason):
            throw WaveLaunchError.alreadyRunning(reason)
        case .reclaim(let staleSession):
            reclaimStaleSession(sessionName: staleSession)
        case .launch:
            break
        }

        let lfPath = try resolveWaveCapableLf(originRepo: origin)
        let args = waveLaunchCommand(
            lfPath: lfPath,
            sessionName: sessionName,
            repoPath: origin,
            waveName: waveName
        )
        try runChecked(args, cwd: origin)
    }

    enum LaunchAction: Equatable {
        /// A live server owns this wave — refuse the second brain.
        case blocked(String)
        /// A ghost session with no live server — kill this name, then launch.
        case reclaim(String)
        /// Nothing in the way — launch.
        case launch
    }

    /// What to do given whether a tmux session exists and a PROBED live
    /// endpoint (`awaitLiveEndpoint`/`liveEndpoint`), never the raw pointer
    /// file. A live endpoint is the only hard block: a SIGKILL leaves the
    /// pointer behind, and a dead `lf wave` leaves the tmux session behind, so
    /// neither the file nor the session may block on mere existence. The
    /// server's own boot floor is the real one-brain guarantee — reclaiming a
    /// ghost here only clears the name for it.
    static func launchAction(sessionName: String, sessionExists: Bool, endpoint: String?) -> LaunchAction {
        if let endpoint {
            return .blocked("Wave already has a live server at \(endpoint).")
        }
        if sessionExists {
            return .reclaim(sessionName)
        }
        return .launch
    }

    /// Poll for a live server up to `attempts` times, 1s apart — the mid-boot
    /// window where `lf wave` is up but hasn't published `.wave-endpoint` yet.
    /// Returns the endpoint once one answers; nil means the session is a ghost.
    static func awaitLiveEndpoint(origin: String, waveName: String, attempts: Int) -> String? {
        for attempt in 0..<attempts {
            if let endpoint = liveEndpoint(
                recorded: WaveEndpoint.read(repoPath: origin, waveName: waveName),
                waveName: waveName
            ) {
                return endpoint
            }
            if attempt < attempts - 1 {
                Thread.sleep(forTimeInterval: 1)
            }
        }
        return nil
    }

    /// Kill a ghost wave-agent tmux session so a fresh `lf wave` can claim the
    /// deterministic name. Safe once no server answers: the name is one-per-wave.
    static func reclaimStaleSession(sessionName: String) {
        _ = runCommandSync(["tmux", "kill-session", "-t", sessionName], logFailure: false)
        LoggingService.lfd("reclaimed stale wave session '\(sessionName)'")
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
    /// `status` is channel liveness (`serving`) and `mind` is the resident's
    /// state — a wave whose mind failed still answers here and still blocks a
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
    /// wave's repo, running `lf wave <name>`.
    static func waveLaunchCommand(
        lfPath: String,
        sessionName: String,
        repoPath: String,
        waveName: String
    ) -> [String] {
        ["tmux", "new-session", "-d", "-s", sessionName, "-c", repoPath, lfPath, "wave", waveName]
    }

    /// Candidate lf binaries in trust order: the lf bundled inside Concerto.app
    /// (shipped next to the bundled lfd), each `lf` on the enriched PATH, then
    /// `<origin>/target/release/lf` — the dev-tree build, for a Concerto pointed
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

    /// First candidate that actually has the `wave` subcommand. Resolving `lf`
    /// from PATH can find a build that predates `lf wave`; it launches fine,
    /// exits instantly, and the UI sits on a dead 20s wait (observed live) —
    /// so every candidate is capability-probed before it's trusted.
    ///
    /// The probe is `lf help wave`, NOT `lf wave --help`: lf's arg reorderer
    /// treats an unknown `wave` as a step name, so `lf wave --help` prints the
    /// root help and exits 0 even on a build without the subcommand. `lf help
    /// wave` exits 0 only when the subcommand exists (verified against both a
    /// stale and a wave-capable build), and clap answers it without touching
    /// any wave state.
    static func resolveWaveCapableLf(
        originRepo: String,
        bundled: URL? = Bundle.main.url(forAuxiliaryExecutable: "lf"),
        pathEnv: String = GUIProcessEnvironment.enrichedPath(from: ProcessInfo.processInfo.environment["PATH"]),
        isExecutableFile: (String) -> Bool = { FileManager.default.isExecutableFile(atPath: $0) },
        probe: (String) -> Bool = hasWaveCommand
    ) throws -> String {
        let candidates = lfCandidates(
            originRepo: originRepo,
            bundled: bundled,
            pathEnv: pathEnv,
            isExecutableFile: isExecutableFile
        )
        guard !candidates.isEmpty else {
            throw WaveLaunchError.noUsableLf(
                "Can't find an lf binary — not bundled with Concerto and not on PATH."
            )
        }
        for candidate in candidates where probe(candidate) {
            return candidate
        }
        throw WaveLaunchError.noUsableLf(
            "No lf with the wave command. Rejected (each failed `lf help wave`, "
                + "so it predates `lf wave`): " + candidates.joined(separator: ", ")
        )
    }

    /// `lf help wave` exits 0 only when this build knows the subcommand.
    static func hasWaveCommand(lfPath: String) -> Bool {
        run([lfPath, "help", "wave"])?.status == 0
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
