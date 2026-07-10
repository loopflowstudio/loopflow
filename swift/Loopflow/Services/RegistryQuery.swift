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
// runner is injected: on macOS it resolves and execs a local `lf`. There is no
// HTTP fallback for reads; remote reads need to become proxied `lf` queries.

import Foundation

/// One `lf` query failed — the subprocess errored, or its JSON didn't decode.
public struct RegistryQueryError: LocalizedError, Sendable {
    public let message: String
    public init(_ message: String) { self.message = message }
    public var errorDescription: String? { message }
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

    /// Every wave the registry knows across the machine. Callers that need
    /// several repo slices should call this once and filter locally.
    public func allWaves() async throws -> [Wave] {
        let stdout = try await run(["ls", "--json"], nil)
        let snapshots = try Self.decode([WaveSnapshot].self, from: stdout)
        return snapshots.map { $0.toWave() }
    }

    /// Every wave the registry knows (running and stopped alike), scoped to one
    /// repo. This replaces the old `/ws` connected snapshot — a point-in-time
    /// read the caller re-queries on a cadence, not a stream.
    public func waves(repoPath: String) async throws -> [Wave] {
        let waves = try await allWaves()
        let target = repoPath.normalizedFilePath
        return waves.filter { $0.repo.normalizedFilePath == target }
    }

    /// One wave's runs and attention (its durable history from the ledger),
    /// plus the live loop state when a server is answering. Feeds `RunStore`
    /// and `AttentionStore` for the focused wave.
    public func status(wave: String, waveId: String, cwd: String?) async throws
        -> (runs: [Run], attention: [AttentionItem], loopState: String?) {
        let stdout = try await run(["status", wave, "--json"], cwd)
        let snapshot = try Self.decode(WaveStatusSnapshot.self, from: stdout)
        let repo = snapshot.wave.repo
        let runs = snapshot.runs.map { $0.toRun(waveId: waveId, repo: repo) }
            + snapshot.tasks.map { $0.toRun(waveId: waveId, repo: repo) }
        let attention = snapshot.attention.map { $0.toItem(waveId: waveId) }
        return (runs, attention, snapshot.loopState)
    }

    /// The recent-run window across every wave on the machine — the ledger the
    /// live `op` frames mirror. A lightweight timeline, not full `Run` objects.
    public func recentRuns() async throws -> [RunLedgerEntry] {
        let stdout = try await run(["runs", "--json"], nil)
        return try Self.decode([RunLedgerEntry].self, from: stdout)
    }

    /// Filed PM tasks for one wave. Active runs remain a separate registry
    /// query; callers subtract running task ids/titles to render backlog. App
    /// reads stay cache-only so a stale snapshot cannot put Linear on the UI
    /// refresh path.
    public func backlog(wave: String, cwd: String?) async throws -> [BacklogItem] {
        let stdout = try await run(["pm", "show", "--wave", wave, "--json", "--no-sync"], cwd)
        let snapshot = try Self.decode(PmShowSnapshot.self, from: stdout)
        return snapshot.items.filter { !$0.completed }
    }

    /// A wave's measured bets from the local PM snapshot. Cache-only reads keep
    /// rendering off the network; explicit and scheduled syncs refresh SQLite.
    public func plan(wave: String, objective: String, cwd: String?) async throws -> WavePlan {
        let stdout = try await run(["pm", "show", "--wave", wave, "--json", "--no-sync"], cwd)
        let snapshot = try Self.decode(PmShowSnapshot.self, from: stdout)
        return WavePlan(
            objective: objective,
            projects: snapshot.projects.map { project in
                WaveProject(
                    id: project.slug,
                    title: project.name,
                    definition: project.definition.isEmpty ? nil : project.definition,
                    krs: project.krs.map {
                        WaveKeyResult(text: $0.text, proof: $0.holds ? .holds : .open)
                    }
                )
            }
        )
    }

    /// Per-boundary spend over a window: what each skill, and each terminal run,
    /// actually spent. `lf usage --json` applies the cumulative-diff rule, so
    /// these rows are additive and sum to the totals `lf usage` prints.
    public func spend(days: Int = 30) async throws -> [TraceSpan] {
        let stdout = try await run(["usage", "--json", "--days", String(days)], nil)
        return try Self.decode([TraceSpan].self, from: stdout)
    }

    /// The codebase on disk, as a tree of directories weighted by tokens.
    /// Mirrors Rust `CodeNode`. Runs in `repoPath` — `lf tokens` measures the
    /// repo it is invoked in.
    public func codebase(repoPath: String) async throws -> CodeNode {
        let stdout = try await run(["tokens", "--json"], repoPath)
        return try Self.decode(CodeNode.self, from: stdout)
    }

    /// How big the codebase was on each day it changed. Mirrors Rust
    /// `CodeSnapshot`. Blob counts are cached by sha, so only the first walk of
    /// a window pays to tokenize.
    public func codebaseHistory(repoPath: String, days: Int = 30) async throws -> [CodeSnapshot] {
        let stdout = try await run(["tokens", "--json", "--days", String(days)], repoPath)
        return try Self.decode([CodeSnapshot].self, from: stdout)
    }

    /// The ledger's self-audit, including continuity and lineage tripwires.
    public func doctor() async throws -> DoctorReport {
        let stdout = try await run(["doctor", "--json"], nil)
        return try Self.decode(DoctorReport.self, from: stdout)
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

private struct PmShowSnapshot: Decodable {
    let wave: String
    let provider: String
    let initiative: String
    let project: String?
    let syncedAt: Int64
    let projects: [PmProjectSnapshot]
    let items: [BacklogItem]

    enum CodingKeys: String, CodingKey {
        case wave, provider, initiative, project, projects, items
        case syncedAt = "synced_at"
    }
}

private struct PmProjectSnapshot: Decodable {
    let id: String
    let slug: String
    let name: String
    let summary: String
    let definition: String
    let krs: [PmKrSnapshot]
    let initiativeIds: [String]

    enum CodingKeys: String, CodingKey {
        case id, slug, name, summary, definition, krs
        case initiativeIds = "initiative_ids"
    }
}

private struct PmKrSnapshot: Decodable {
    let text: String
    let holds: Bool
}

public struct BacklogItem: Decodable, Sendable, Identifiable, Hashable {
    public let id: String
    public let name: String
    public let description: String
    public let rank: UInt32
    public let completed: Bool
    public let project: String?
    public let assignee: String?
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
    let activeTasks: Int
    let live: Bool
    let endpoint: String?
    let createdAt: String?
    let parentWaveId: String?

    enum CodingKeys: String, CodingKey {
        case id, name, status, paused, goal, repo, iteration, workers, live, endpoint
        case activeRuns = "active_runs"
        case activeTasks = "active_tasks"
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
    let loopState: String?
    let runs: [RunSnapshot]
    let tasks: [TaskSnapshot]
    let attention: [AttentionSnapshot]

    enum CodingKeys: String, CodingKey {
        case wave, runs, tasks, attention
        case loopState = "loop_state"
    }
}

/// One durable Task Session under `lf status`.
struct TaskSnapshot: Decodable {
    let issueId: String
    let sessionId: String
    let project: String
    let status: String
    let reason: String
    let statusAt: String
    let worktree: String
    let branch: String
    let provider: String
    let processAlive: Bool
    let prURL: String?

    enum CodingKeys: String, CodingKey {
        case project, status, reason, worktree, branch, provider
        case issueId = "issue_id"
        case sessionId = "session_id"
        case statusAt = "status_at"
        case processAlive = "process_alive"
        case prURL = "pr_url"
    }

    func toRun(waveId: String, repo: String) -> Run {
        let runStatus: RunStatus = switch status {
        case "created", "starting": .pending
        case "running": .running
        case "waiting", "submitted", "blocked": .waiting
        case "merged", "abandoned": .completed
        case "failed": .failed
        default: .unspecified
        }
        let pr = prURL.flatMap(URL.init(string:)).map {
            PullRequest(url: $0, number: nil, state: nil, title: nil, branch: branch)
        }
        return Run(
            id: sessionId,
            waveId: waveId,
            flow: "task",
            task: issueId,
            repo: repo,
            status: runStatus,
            stepIndex: 0,
            worktree: worktree,
            branch: branch,
            error: status == "failed" ? reason : nil,
            pr: pr,
            startedAt: RegistrySnapshotDate.parse(statusAt),
            endedAt: ["merged", "abandoned"].contains(status)
                ? RegistrySnapshotDate.parse(statusAt) : nil,
            createdAt: RegistrySnapshotDate.parse(statusAt)
        )
    }
}

/// One run under `lf status`. Mirrors Rust `RunSnapshot`.
struct RunSnapshot: Decodable {
    let id: String
    let flow: String
    let task: String?
    let stepIndex: Int
    let status: String
    let branch: String
    let worktree: String
    let startedAt: String?
    let endedAt: String?
    let error: String?
    let prURL: String?
    let prState: String?
    let prTitle: String?

    enum CodingKeys: String, CodingKey {
        case id, flow, task, status, branch, worktree, error
        case stepIndex = "step_index"
        case startedAt = "started_at"
        case endedAt = "ended_at"
        case prURL = "pr_url"
        case prState = "pr_state"
        case prTitle = "pr_title"
    }

    func toRun(waveId: String, repo: String) -> Run {
        let pr: PullRequest? = prURL
            .flatMap { URL(string: $0) }
            .map {
                PullRequest(
                    url: $0,
                    number: nil,
                    state: prState.flatMap(PRState.init(rawValue:)),
                    title: prTitle,
                    branch: nil
                )
            }
        return Run(
            id: id,
            waveId: waveId,
            flow: flow,
            task: task,
            repo: repo,
            status: RunStatus(lfToken: status),
            stepIndex: stepIndex,
            worktree: worktree.isEmpty ? nil : worktree,
            branch: branch.isEmpty ? nil : branch,
            error: error,
            pr: pr,
            startedAt: RegistrySnapshotDate.parse(startedAt),
            endedAt: RegistrySnapshotDate.parse(endedAt),
            createdAt: RegistrySnapshotDate.parse(startedAt)
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
    public let runId: String
    public let processId: String
    public let parentProcessId: String?
    public let repo: String?
    public let wave: String?
    public let label: String
    public let status: String
    public let started: Int
    public let ended: Int?
    public let inputTokens: Int
    public let outputTokens: Int
    public let cacheReadTokens: Int
    public let costUsd: Double?
    public let durationSecs: Double?
    public let provider: String?
    public let model: String?

    enum CodingKeys: String, CodingKey {
        case id, repo, wave, label, status, started, ended, provider, model
        case runId = "run_id"
        case processId = "process_id"
        case parentProcessId = "parent_process_id"
        case inputTokens = "input_tokens"
        case outputTokens = "output_tokens"
        case cacheReadTokens = "cache_read_tokens"
        case costUsd = "cost_usd"
        case durationSecs = "duration_secs"
    }
}

/// One process in `lf trace --json`. Mirrors Rust `SpanDto` exactly.
public struct TraceSpan: Decodable, Sendable, Identifiable {
    /// A process contributes several boundaries. Their event sequence is the
    /// stable discriminator even when one skill completes twice in one second.
    public var id: String { "\(processId)-\(seq)" }

    public let runId: String
    public let processId: String
    public let parentProcessId: String?
    public let seq: Int
    public let node: String
    public let name: String?
    public let repo: String?
    public let wave: String?
    public let flow: String?
    public let skill: String?
    public let startedAt: Int
    public let endedAt: Int?
    public let status: String
    public let inputTokens: Int?
    public let outputTokens: Int?
    public let cacheReadTokens: Int?
    public let costUsd: Double?
    public let durationSecs: Double?
    public let provider: String?
    public let model: String?

    /// `provider:model` — the harness and the model it drove.
    public var agent: String {
        switch (provider, model) {
        case let (provider?, model?): return "\(provider):\(model)"
        case let (provider?, nil): return provider
        default: return "unattributed"
        }
    }

    public var totalTokens: Int { (inputTokens ?? 0) + (outputTokens ?? 0) }

    enum CodingKeys: String, CodingKey {
        case seq, node, name, status, provider, model, repo, wave, flow, skill
        case runId = "run_id"
        case processId = "process_id"
        case parentProcessId = "parent_process_id"
        case startedAt = "started_at"
        case endedAt = "ended_at"
        case inputTokens = "input_tokens"
        case outputTokens = "output_tokens"
        case cacheReadTokens = "cache_read_tokens"
        case costUsd = "cost_usd"
        case durationSecs = "duration_secs"
    }
}

public struct DoctorReport: Decodable, Sendable {
    public let rows: Int
    public let checks: [DoctorCheck]
}

public struct DoctorCheck: Decodable, Sendable, Identifiable {
    public var id: String { name }

    public let name: String
    public let status: String
    public let detail: String
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

/// A directory or file in the codebase, weighted by the tokens a model pays to
/// read it. Mirrors Rust `CodeNode` exactly.
public struct CodeNode: Decodable, Sendable, Identifiable {
    public var id: String { path.isEmpty ? name : path }

    public let path: String
    public let name: String
    public let lines: Int
    public let tokens: Int
    public let children: [CodeNode]
}

/// The codebase's size on one day. Mirrors Rust `CodeSnapshot` exactly.
public struct CodeSnapshot: Decodable, Sendable, Identifiable {
    public var id: String { commit }

    public let date: String
    public let commit: String
    public let lines: Int
    public let tokens: Int
    public let slices: [CodeSlice]
}

/// One file extension's weight in a snapshot. Mirrors Rust `CodeSlice`.
/// `ext` carries no dot; a file with no extension is `(none)` and the long tail
/// is folded into `other`.
public struct CodeSlice: Decodable, Sendable, Identifiable {
    public var id: String { ext }

    public let ext: String
    public let lines: Int
    public let tokens: Int
}
