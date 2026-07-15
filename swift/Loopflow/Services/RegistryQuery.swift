// RegistryQuery — discovery and history as `lf` queries over the machine
// registry store, not a streaming center.
//
// The wave model has no telemetry hub (see `scratch/eventing.md`): durable
// facts — which waves exist (running and stopped) and their work
// — are QUERIES against the shared SQLite ledger, served by the daemonless `lf`
// CLI. Live motion is a per-wave SSE stream (`WaveChatConnection`), never this.
//
// This runs `lf ls/status/roadmap/runs --json` as a subprocess and decodes the wire
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

    /// One wave's Project/Task work, plus the live loop state when its resident
    /// is answering.
    public func status(wave: String, cwd: String?) async throws
        -> WaveStatusResult {
        let stdout = try await run(["status", wave, "--json"], cwd)
        let snapshot = try Self.decode(WaveStatusSnapshot.self, from: stdout)
        return WaveStatusResult(
            workMap: WaveWorkMap(objective: snapshot.wave.goal, projects: snapshot.projects),
            loopState: snapshot.loopState,
            runs: snapshot.runs,
            attention: snapshot.attention
        )
    }

    /// Every durable plan row across the machine, joined to the same Task
    /// references and live evidence as `lf status`. One subprocess reads every
    /// Wave; an optional scope filters that shared snapshot at the source.
    public func roadmap(wave: String? = nil) async throws -> RoadmapSnapshot {
        var args = ["roadmap"]
        if let wave {
            args.append(contentsOf: ["--wave", wave])
        }
        args.append("--json")
        let stdout = try await run(args, nil)
        return try Self.decode(RoadmapSnapshot.self, from: stdout)
    }

    /// Files changed by one Task, classified across commits, index, worktree,
    /// and untracked state relative to the Task Session's recorded base.
    public func taskChanges(issue: String, cwd: String?) async throws -> TaskChangesSnapshot {
        let stdout = try await run(["task", "changes", issue, "--json"], cwd)
        return try Self.decode(TaskChangesSnapshot.self, from: stdout)
    }

    /// One Task's complete patch, or the patch for a selected changed file.
    public func taskDiff(
        issue: String,
        path: String?,
        cwd: String?
    ) async throws -> TaskDiffSnapshot {
        var args = ["task", "diff", issue]
        if let path { args.append(path) }
        args.append("--json")
        let stdout = try await run(args, cwd)
        return try Self.decode(TaskDiffSnapshot.self, from: stdout)
    }

    /// Current contents of one file, constrained to the Task worktree.
    public func taskFile(
        issue: String,
        path: String,
        cwd: String?
    ) async throws -> TaskFileSnapshot {
        let stdout = try await run(["task", "file", issue, path, "--json"], cwd)
        return try Self.decode(TaskFileSnapshot.self, from: stdout)
    }

    /// The recent-run window across every wave on the machine — the ledger the
    /// live `op` frames mirror. A process timeline, not a second work hierarchy.
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
/// `WaveHomeDto` (`engine/wave_home.rs`) — a Wave's Home: a user-owned execution
/// address (owner plus location). `address` is the canonical string to show
/// (`jack@local` or `ssh://jack@host[:port]`); `location` is the structured form
/// to navigate/open. The app never parses or reimplements SSH — it reads these.
public struct WaveHome: Decodable, Sendable, Hashable {
    public let address: String
    public let owner: String
    public let location: HomeLocation
}

/// `HomeLocationDto` — where a Home's execution context lives.
public enum HomeLocation: Decodable, Sendable, Hashable {
    case local
    case ssh(host: String, port: Int?)

    enum CodingKeys: String, CodingKey {
        case kind, host, port
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "local":
            self = .local
        case "ssh":
            self = .ssh(
                host: try container.decode(String.self, forKey: .host),
                port: try container.decodeIfPresent(Int.self, forKey: .port)
            )
        case let other:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: container,
                debugDescription: "unknown home location kind \(other)"
            )
        }
    }
}

/// `HomeState` (`engine/wave_home.rs`) — a Home's observed liveness.
enum HomeState: String, Decodable, Equatable {
    case unreachable, stopped, running, unknown
}

/// `HomeActionDto` — the single contextual action a surface should offer, so the
/// UI never branches on `HomeState` itself: Attach when running, Start when
/// reachable-but-stopped, or the actionable reason otherwise.
enum HomeAction: Decodable, Equatable {
    case attach(endpoint: String)
    case start(home: String)
    case reason(message: String)

    enum CodingKeys: String, CodingKey {
        case kind, endpoint, home, message
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "attach":
            self = .attach(endpoint: try container.decode(String.self, forKey: .endpoint))
        case "start":
            self = .start(home: try container.decode(String.self, forKey: .home))
        case "reason":
            self = .reason(message: try container.decode(String.self, forKey: .message))
        case let other:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: container,
                debugDescription: "unknown home action kind \(other)"
            )
        }
    }
}

/// `HomeRuntimeDto` — a Wave's Home probed for liveness: the address, the state
/// with its evidence, the attach endpoint when running, and the one action.
/// This is the shared contract the conductor renders; the app never probes SSH
/// itself — it calls `lf home probe|start --json`.
struct HomeRuntime: Decodable, Equatable {
    let home: WaveHome
    let state: HomeState
    let reason: String
    let endpoint: String?
    let action: HomeAction
}

/// `WaveSnapshot` (`lf/commands/waves.rs`) — every field present, Optionals
/// explicit (no serde defaults on the wire).
public struct WaveSnapshot: Decodable, Sendable, Hashable {
    public let id: String
    public let name: String
    public let status: WaveStatus
    public let paused: Bool
    public let goal: String
    public let repo: String
    public let activeTasks: Int
    public let activeProjects: Int
    public let live: Bool
    public let endpoint: String?
    public let createdAt: String?
    public let parentWaveId: String?
    public let home: WaveHome

    enum CodingKeys: String, CodingKey {
        case id, name, status, paused, goal, repo, live, endpoint, home
        case activeTasks = "active_tasks"
        case activeProjects = "active_projects"
        case createdAt = "created_at"
        case parentWaveId = "parent_wave_id"
    }

    /// Map the registry snapshot to the app's deliberately small Wave row.
    func toWave() -> Wave {
        Wave(
            id: id,
            name: name,
            repo: repo,
            status: status
        )
    }
}

public struct RoadmapSnapshot: Decodable, Sendable, Hashable {
    public let generatedAt: String
    public let waves: [WaveRoadmap]

    enum CodingKeys: String, CodingKey {
        case waves
        case generatedAt = "generated_at"
    }
}

public struct WaveRoadmap: Decodable, Sendable, Hashable {
    public let wave: WaveSnapshot
    public let projects: WorkEvidence<RoadmapProject>
}

/// `lf status <wave>` snapshot. Mirrors Rust `WaveDetailSnapshot`.
struct WaveStatusSnapshot: Decodable {
    let wave: WaveSnapshot
    let loopState: String?
    let projects: [WaveProjectWork]
    let runs: WorkEvidence<RunLedgerEntry>
    let attention: WorkEvidence<WaveAttentionItem>
    /// The focused Wave's Home probed for liveness and its one contextual action.
    let homeRuntime: HomeRuntime

    enum CodingKeys: String, CodingKey {
        case wave, projects, runs, attention
        case homeRuntime = "home_runtime"
        case loopState = "loop_state"
    }
}

/// One folded run from `lf runs --json`. Mirrors Rust `RunLedgerEntry`
/// (`lf/commands/runs.rs`): a ledger timeline entry, `started`/`ended` in unix
/// seconds, `wave` a name (not an id).
public struct RunLedgerEntry: Decodable, Sendable, Identifiable, Hashable {
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
