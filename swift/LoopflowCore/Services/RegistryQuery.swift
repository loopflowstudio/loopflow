// RegistryQuery — discovery and history as `lf` queries over the machine
// registry (`lfdb`), not a streaming center.
//
// The wave model has no telemetry hub (see `scratch/eventing.md`): durable
// facts — which waves exist (running and stopped), a wave's runs, its attention
// — are QUERIES against the shared SQLite ledger, served by the daemonless `lf`
// CLI. Live motion is a per-wave SSE stream (`WaveChatConnection`), never this.
//
// This runs `lf ls/status/runs --json` as a subprocess and decodes the wire
// snapshots (mirrors of the Rust types in `lf/commands/waves.rs` and
// `lf/commands/runs.rs`) into the app models the stores hold. The subprocess
// runner is injected: on macOS it resolves and execs a local `lf`; the
// `RepoTarget.remote` path is a later seam (an `lf` over SSH), so a caller with
// no runner falls back to the surviving REST reads.

import Foundation

/// One `lf` query failed — the subprocess errored, or its JSON didn't decode.
public struct RegistryQueryError: Error, Sendable {
    public let message: String
    public init(_ message: String) { self.message = message }
}

/// Runs an `lf` argv (already including the subcommand, e.g. `["ls","--json"]`)
/// and returns captured stdout. Throws on a non-zero exit or spawn failure.
/// `cwd` seeds ambient resolution for verbs that want it (`lf status` with no
/// wave); the machine-wide reads (`lf ls`, `lf runs`) ignore it.
public typealias RegistryRunner = @Sendable (_ lfArgs: [String], _ cwd: String?) async throws -> String

public struct RegistryQuery: Sendable {
    private let run: RegistryRunner

    public init(run: @escaping RegistryRunner) {
        self.run = run
    }

    /// Every wave the registry knows (running and stopped alike), scoped to one
    /// repo. This replaces the old `/ws` connected snapshot — a point-in-time
    /// read the caller re-queries on a cadence, not a stream.
    public func waves(repoPath: String) async throws -> [Wave] {
        let stdout = try await run(["ls", "--json"], nil)
        let snapshots = try Self.decode([WaveSnapshot].self, from: stdout)
        let target = repoPath.normalizedFilePath
        return snapshots
            .filter { $0.repo.normalizedFilePath == target }
            .map { $0.toWave() }
    }

    /// One wave's runs and attention (its durable history from the ledger),
    /// plus the live mind state when a server is answering. Feeds `RunStore`
    /// and `AttentionStore` for the focused wave.
    public func status(wave: String, waveId: String, cwd: String?) async throws
        -> (runs: [Run], attention: [AttentionItem], mind: String?) {
        let stdout = try await run(["status", wave, "--json"], cwd)
        let snapshot = try Self.decode(WaveStatusSnapshot.self, from: stdout)
        let repo = snapshot.wave.repo
        let runs = snapshot.runs.map { $0.toRun(waveId: waveId, repo: repo) }
        let attention = snapshot.attention.map { $0.toItem(waveId: waveId) }
        return (runs, attention, snapshot.mind)
    }

    /// The recent-run window across every wave on the machine — the ledger the
    /// live `op` frames mirror. A lightweight timeline, not full `Run` objects.
    public func recentRuns() async throws -> [RunLedgerEntry] {
        let stdout = try await run(["runs", "--json"], nil)
        return try Self.decode([RunLedgerEntry].self, from: stdout)
    }

    private static func decode<T: Decodable>(_ type: T.Type, from stdout: String) throws -> T {
        // `lf` prints one JSON line; trim any surrounding whitespace/newline.
        let trimmed = stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let data = trimmed.data(using: .utf8) else {
            throw RegistryQueryError("lf query returned non-UTF8 output")
        }
        do {
            return try JSONDecoder().decode(T.self, from: data)
        } catch {
            throw RegistryQueryError("lf query JSON did not decode: \(error)")
        }
    }
}

// MARK: - Wire snapshots (mirror the Rust `--json` types)

/// One `lf ls` row / the `wave` field of `lf status`. Mirrors Rust
/// `WaveSnapshot` (`lf/commands/waves.rs`) — every field present, Optionals
/// explicit (no serde defaults on the wire).
struct WaveSnapshot: Decodable {
    let id: String
    let name: String
    let status: String
    let paused: Bool
    let goal: String
    let repo: String
    let iteration: Int
    let workers: Int
    let activeRuns: Int
    let live: Bool
    let endpoint: String?
    let createdAt: String?
    let parentWaveId: String?

    enum CodingKeys: String, CodingKey {
        case id, name, status, paused, goal, repo, iteration, workers, live, endpoint
        case activeRuns = "active_runs"
        case createdAt = "created_at"
        case parentWaveId = "parent_wave_id"
    }

    /// Map the registry snapshot to the app `Wave`. `lf ls` carries the durable
    /// identity + rolled-up status; the rich per-run and on-disk detail
    /// (direction, area, active run object, diff) is loaded separately when a
    /// wave is opened, so those stay empty here.
    func toWave() -> Wave {
        Wave(
            id: id,
            name: name,
            repo: repo,
            goal: goal,
            status: WaveStatus(rawValue: status) ?? .idle,
            iteration: iteration,
            createdAt: RegistrySnapshotDate.parse(createdAt),
            parentWaveId: parentWaveId
        )
    }
}

/// `lf status <wave>` snapshot. Mirrors Rust `WaveStatusSnapshot`.
struct WaveStatusSnapshot: Decodable {
    let wave: WaveSnapshot
    let mind: String?
    let runs: [RunSnapshot]
    let attention: [AttentionSnapshot]
}

/// One run under `lf status`. Mirrors Rust `RunSnapshot`.
struct RunSnapshot: Decodable {
    let id: String
    let flow: String
    let task: String?
    let status: String
    let branch: String
    let worktree: String
    let startedAt: String?
    let endedAt: String?
    let error: String?
    let prURL: String?

    enum CodingKeys: String, CodingKey {
        case id, flow, task, status, branch, worktree, error
        case startedAt = "started_at"
        case endedAt = "ended_at"
        case prURL = "pr_url"
    }

    func toRun(waveId: String, repo: String) -> Run {
        let pr: PullRequest? = prURL
            .flatMap { URL(string: $0) }
            .map { PullRequest(url: $0, number: nil, state: nil, title: nil, branch: nil) }
        return Run(
            id: id,
            waveId: waveId,
            flow: flow,
            task: task,
            area: ".",
            repo: repo,
            status: RunStatus(rawValue: status) ?? .pending,
            worktree: worktree.isEmpty ? nil : worktree,
            branch: branch.isEmpty ? nil : branch,
            error: error,
            pr: pr,
            startedAt: RegistrySnapshotDate.parse(startedAt),
            endedAt: RegistrySnapshotDate.parse(endedAt),
            createdAt: RegistrySnapshotDate.parse(startedAt) ?? Date()
        )
    }
}

/// One attention item under `lf status`. Mirrors Rust `AttentionSnapshot`.
struct AttentionSnapshot: Decodable {
    let id: String
    let kind: String
    let status: String
    let title: String
    let summary: String
    let runId: String?
    let surfacedAt: String

    enum CodingKeys: String, CodingKey {
        case id, kind, status, title, summary
        case runId = "run_id"
        case surfacedAt = "surfaced_at"
    }

    func toItem(waveId: String) -> AttentionItem {
        let attentionKind = AttentionKind(rawValue: kind) ?? .interactive
        return AttentionItem(
            id: id,
            waveId: waveId,
            runId: runId,
            kind: attentionKind,
            status: AttentionStatus(rawValue: status) ?? .surfaced,
            title: title,
            summary: summary,
            context: AttentionItem.context(kind: attentionKind, json: [:]),
            surfacedAt: RegistrySnapshotDate.parse(surfacedAt) ?? Date()
        )
    }
}

/// One folded run from `lf runs --json`. Mirrors Rust `RunLedgerEntry`
/// (`lf/commands/runs.rs`): a ledger timeline entry, `started`/`ended` in unix
/// seconds, `wave` a name (not an id).
public struct RunLedgerEntry: Decodable, Sendable, Identifiable {
    public let id: String
    public let repo: String?
    public let wave: String?
    public let label: String
    public let status: String
    public let started: Int
    public let ended: Int?
    public let inputTokens: Int
    public let outputTokens: Int

    enum CodingKeys: String, CodingKey {
        case id, repo, wave, label, status, started, ended
        case inputTokens = "input_tokens"
        case outputTokens = "output_tokens"
    }
}

private enum RegistrySnapshotDate {
    /// Parse an RFC3339 timestamp (with or without fractional seconds), the
    /// grain the `lf` snapshots emit.
    static func parse(_ value: String?) -> Date? {
        guard let value, !value.isEmpty else { return nil }
        let fractional = ISO8601DateFormatter()
        fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return fractional.date(from: value) ?? ISO8601DateFormatter().date(from: value)
    }
}
