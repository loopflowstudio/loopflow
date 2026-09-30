// RegistryQuery — typed `lf` reads over the machine registry.
//
// Planning and history are one-shot queries. Active Runs use one foreground
// observation per window so native receipt discovery survives between samples.
//
// This runs `lf wave list`, `lf wave status`, and the roadmap, ps, and activity
// readers with `--json` as subprocesses and decodes the wire
// snapshots (mirrors of the Rust types in `lf/commands/waves.rs` and
// `lf/commands/runs.rs`) into the app models the stores hold. The subprocess
// runner is injected: on macOS it execs the `lf` shipped inside the app. There is no
// HTTP fallback for reads; remote reads need to become proxied `lf` queries.

import Foundation

/// One `lf` query failed — the subprocess errored, or its JSON didn't decode.
public struct RegistryQueryError: LocalizedError, Sendable {
    public let message: String
    public init(_ message: String) { self.message = message }
    public var errorDescription: String? { message }
}

/// Runs an `lf` argv (already including the subcommand, e.g. `["wave", "list","--json"]`)
/// and returns captured stdout. Throws on a non-zero exit or spawn failure.
/// `cwd` seeds ambient resolution for verbs that want it (`lf wave status` with no
/// wave); the machine-wide reads (`lf wave list`, `lf runs`) ignore it.
public typealias RegistryRunner = @Sendable (_ lfArgs: [String], _ cwd: String?) async throws -> String

public struct RegistryQuery: Sendable {
    private let run: RegistryRunner
    private let runWithInput: @Sendable ([String], String?, String) async throws -> String
    private let observe: @Sendable () async throws -> ActiveRunsObservation

    public init(
        runWithInput: @escaping @Sendable ([String], String?, String) async throws -> String = { _, _, _ in
            throw RegistryQueryError("Draft comparison is unavailable on this transport")
        },
        watchActiveRuns: @escaping @Sendable () async throws -> ActiveRunsObservation = {
            throw RegistryQueryError("Active Run observation is unavailable on this transport")
        },
        run: @escaping RegistryRunner
    ) {
        self.runWithInput = runWithInput
        self.run = run
        self.observe = watchActiveRuns
    }

    /// Current Waves across the machine, including stopped Waves. The shared
    /// reader excludes historical registrations; callers only slice by repo.
    public func allWaves() async throws -> [Wave] {
        let stdout = try await run(["wave", "list", "--all", "--current", "--json"], nil)
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
        -> WaveDetailSnapshot {
        let stdout = try await run(["wave", "status", wave, "--json"], cwd)
        return try Self.decode(WaveDetailSnapshot.self, from: stdout)
    }

    public func chapterHistory(wave: String, cwd: String?) async throws -> [ChapterHistoryEntry] {
        try Self.decode([ChapterHistoryEntry].self, from: await run(["wave", "history", "--wave", wave, "--json"], cwd))
    }

    public func chapter(wave: String, id: String, cwd: String?) async throws -> ChapterSnapshot {
        try Self.decode(ChapterSnapshot.self, from: await run(["wave", "status", wave, "--chapter", id, "--json"], cwd))
    }


    /// Every durable plan row across the machine, joined to the same Task
    /// references and live evidence as `lf wave status`. One subprocess reads every
    /// Wave; an optional scope filters that shared snapshot at the source.
    public func roadmap(wave: String? = nil) async throws -> RoadmapSnapshot {
        var args = ["roadmap"]
        if let wave {
            args.append(contentsOf: ["--wave", wave])
        } else {
            args.append("--all")
        }
        args.append("--json")
        let stdout = try await run(args, nil)
        return try Self.decode(RoadmapSnapshot.self, from: stdout)
    }

    /// Exact OS-live Loopflow process trees and unattributed provider processes.
    public func processActivity() async throws -> ActivitySnapshot {
        let stdout = try await run(["ps", "--json"], nil)
        return try Self.decode(ActivitySnapshot.self, from: stdout)
    }

    public func userName() async throws -> String? {
        let stdout = try await run(["user", "name", "--json"], nil)
        return try Self.decode(String?.self, from: stdout)
    }

    public func watchActiveRuns() async throws -> ActiveRunsObservation {
        try await observe()
    }

    /// Durable Work facts across creation, Runs, PR lifecycle, and Steers.
    /// Filters are composed by `lf` before its bounded presentation window.
    public func workActivity(
        since: String = "7d",
        limit: Int = 50,
        wave: String? = nil,
        project: String? = nil,
        task: String? = nil
    ) async throws -> WorkActivitySnapshot {
        var args = ["activity", "--since", since, "--limit", String(limit)]
        if let wave { args.append(contentsOf: ["--wave", wave]) }
        if let project { args.append(contentsOf: ["--project", project]) }
        if let task { args.append(contentsOf: ["--task", task]) }
        args.append("--json")
        let stdout = try await run(args, nil)
        return try Self.decode(WorkActivitySnapshot.self, from: stdout)
    }

    /// Files changed by one Task, classified across commits, index, worktree,
    /// and untracked state relative to the Task's recorded base.
    public func taskChanges(issue: String, base: String = "parent", cwd: String?) async throws -> TaskChangesSnapshot {
        let stdout = try await run(["task", "changes", issue, "--base", base, "--json"], cwd)
        return try Self.decode(TaskChangesSnapshot.self, from: stdout)
    }

    /// Every selectable Flow with the topology it would pin, via the shared loader.
    public func flowCatalog(cwd: String?) async throws -> [FlowCatalogEntry] {
        let stdout = try await run(["flow", "list", "--json"], cwd)
        return try Self.decode([FlowCatalogEntry].self, from: stdout)
    }

    /// Start (preparing when needed) the Task's managed Flow.
    public func runTaskFlow(issue: String, flow: String?, cwd: String?) async throws {
        var args = ["task", "run", issue]
        if let flow { args += ["--flow", flow] }
        _ = try await run(args, cwd)
    }

    /// Checkpoint, stop, and replace the pinned Flow. Rust validates `flow`
    /// before any side effect.
    public func restartTaskFlow(issue: String, flow: String, cwd: String?) async throws {
        _ = try await run(["task", "restart", issue, "--flow", flow], cwd)
    }

    /// One planning Task's complete comment thread. Read-only; works before
    /// the Task is prepared or started.
    public func taskComments(id: String, wave: String, cwd: String) async throws -> TaskComments {
        let stdout = try await run(["task", "comment", id, "--wave", wave, "--json"], cwd)
        return try Self.decode(TaskComments.self, from: stdout)
    }

    /// Recent Runs attributed to one Task through the shared `lf runs` reader:
    /// the last seven days, newest first, capped. Read-only; an unstarted Task
    /// is neither prepared nor started by asking.
    public func taskRuns(task: String, cwd: String?) async throws -> [RunSnapshot] {
        let stdout = try await run(["runs", "--task", task, "--json"], cwd)
        return try Self.decode([RunSnapshot].self, from: stdout)
    }

    public func updateTaskDirective(id: String, wave: String, text: String, cwd: String) async throws {
        _ = try await run([
            "task", "edit", id, "--wave", wave, "--notes=\(text)",
        ], cwd)
    }

    /// One Task's complete patch, or the patch for a selected changed file.
    public func taskDiff(
        issue: String,
        path: String?,
        base: String = "parent",
        draft: String? = nil,
        cwd: String?
    ) async throws -> TaskDiffSnapshot {
        var args = ["task", "diff", issue]
        if let path { args.append(path) }
        args.append(contentsOf: ["--base", base, "--json"])
        let stdout: String
        if let draft {
            args.append("--draft")
            stdout = try await runWithInput(args, cwd, draft)
        } else {
            stdout = try await run(args, cwd)
        }
        return try Self.decode(TaskDiffSnapshot.self, from: stdout)
    }

    /// One page of immediate directory entries in the Task checkout.
    public func taskFiles(issue: String, directory: String, cursor: String? = nil,
                          showIgnored: Bool = false, cwd: String?) async throws -> TaskDirectory {
        var args = ["task", "files", issue, directory.isEmpty ? "." : directory, "--json"]
        if let cursor { args += ["--cursor", cursor] }
        if showIgnored { args.append("--show-ignored") }
        return try Self.decode(TaskDirectory.self, from: try await run(args, cwd))
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

    public func saveTaskFile(issue: String, path: String, revision: String, content: String,
                             cwd: String?) async throws -> TaskFileSave {
        let stdout = try await runWithInput(["task", "save", issue, path, "--revision", revision, "--json"], cwd, content)
        return try Self.decode(TaskFileSave.self, from: stdout)
    }

    /// Sessions in this repository.
    public func sessions(cwd: String? = nil) async throws -> [SessionRecord] {
        let stdout = try await run(["session", "list", "--json"], cwd)
        return try Self.decode([SessionRecord].self, from: stdout)
    }

    /// Open one Session and return its terminal command.
    public func openSession(
        id: String,
        replacing: Bool = false,
        cwd: String? = nil
    ) async throws -> SessionRecord {
        var args = ["session", "open", id, "--json"]
        if replacing { args.append("--replace") }
        let stdout = try await run(args, cwd)
        return try Self.decode(SessionRecord.self, from: stdout)
    }

    /// Give one Session a human-assigned name and return the authoritative
    /// record. A Run ID reaches the Ask or Flow boundary that owns it.
    public func renameSession(
        id: String,
        name: String,
        cwd: String? = nil
    ) async throws -> SessionRecord {
        let stdout = try await run(["session", "rename", "--json", "--", id, name], cwd)
        return try Self.decode(SessionRecord.self, from: stdout)
    }

    /// Complete an interactive conversation, Flow review, or blocked Ask.
    public func completeSession(
        id: String,
        cwd: String? = nil
    ) async throws {
        _ = try await run(["session", "complete", id], cwd)
    }

    /// A wave's measured bets from the local PM snapshot. Cache-only reads keep
    /// rendering off the network; explicit and scheduled syncs refresh SQLite.
    public func plan(
        wave: String,
        objective: String,
        cwd: String?,
        sync: Bool = false
    ) async throws -> WavePlan {
        if sync { _ = try await run(["wave", "sync", wave], cwd) }
        let stdout = try await run(["wave", "status", wave, "--json"], cwd)
        let snapshot = try Self.decode(WaveDetailSnapshot.self, from: stdout)
        return WavePlan(objective: objective, chapter: snapshot.chapter)
    }

    /// Direct provider-authored usage for recent Home-local Runs, optionally
    /// drilled through the shared Wave → Project → Task attribution.
    public func usage(
        days: Int = 30,
        wave: String? = nil,
        project: String? = nil,
        task: String? = nil
    ) async throws -> [RunSnapshot] {
        var arguments = ["usage", "--days", String(days), "--json"]
        if let wave { arguments += ["--wave", wave] }
        if let project { arguments += ["--project", project] }
        if let task { arguments += ["--task", task] }
        let stdout = try await run(arguments, nil)
        return try Self.decode([RunSnapshot].self, from: stdout)
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

// MARK: - Wire snapshots (mirror the Rust `--json` types)

/// Durable execution authority and its mutable observed route.
public struct Home: Decodable, Sendable, Hashable {
    public let id: String
    public let route: String
    public let createdAt: String
    public let observedAt: String

    enum CodingKeys: String, CodingKey {
        case id, route
        case createdAt = "created_at"
        case observedAt = "observed_at"
    }
}

/// `WaveSnapshot` (`lf/commands/waves.rs`) — every field present, Optionals
/// explicit (no serde defaults on the wire).
public struct WaveSnapshot: Decodable, Sendable, Hashable, Identifiable {
    public let id: String
    public let name: String
    public let status: WorkStatus
    public let goal: String
    public let repo: String
    public let activeTasks: Int
    public let createdAt: String?
    public let parentWaveId: String?
    public let retiredAt: String?
    public let supersededByWaveId: String?
    public let retirementReason: String?
    public let home: Home

    enum CodingKeys: String, CodingKey {
        case id, name, status, goal, repo, home
        case activeTasks = "active_tasks"
        case createdAt = "created_at"
        case parentWaveId = "parent_wave_id"
        case retiredAt = "retired_at"
        case supersededByWaveId = "superseded_by_wave_id"
        case retirementReason = "retirement_reason"
    }

    /// Map the registry snapshot to the app's Wave row, carrying the shared
    /// liveness, active-work, and ancestry facts the surface renders.
    public func toWave() -> Wave {
        Wave(
            id: id,
            name: name,
            repo: repo,
            status: status,
            activeTasks: activeTasks,
            parentWaveId: parentWaveId,
            retiredAt: retiredAt,
            supersededByWaveId: supersededByWaveId,
            retirementReason: retirementReason
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
    public let metricPortfolio: MetricPortfolio
    public let chapter: ChapterSummary?
    public let tasks: WorkEvidence<RoadmapTask>
    public let unavailableTasks: [UnavailableTaskEvidence]

    enum CodingKeys: String, CodingKey {
        case wave, chapter, tasks
        case metricPortfolio = "metric_portfolio"
        case unavailableTasks = "unavailable_tasks"
    }
}

/// Durable Project Work that cannot join the current PM plan, including
/// non-terminal Tasks stranded under a terminal historical Project.
/// Non-terminal durable Task Work whose historical Project is absent from the
/// current PM plan.
public struct UnavailableTaskEvidence: Decodable, Sendable, Hashable {
    public let workId: String
    public let taskId: String
    public let taskIdentifier: String
    public let status: WorkStatus
    public let owner: WorkNextMoveOwner
    public let reason: String
    public let recovery: String

    enum CodingKeys: String, CodingKey {
        case status, owner, reason, recovery
        case workId = "work_id"
        case taskId = "task_id"
        case taskIdentifier = "task_identifier"
    }
}

/// `lf wave status <wave>` snapshot. Mirrors Rust `WaveDetailSnapshot` without
/// reshaping or dropping fields, so every Wave surface starts from one reading.
public struct WaveDetailSnapshot: Decodable, Sendable {
    public let wave: WaveSnapshot
    public let chapter: ChapterSummary?
    public let tasks: WorkEvidence<WaveTaskWork>
    public let metricPortfolio: MetricPortfolio
    public let unavailableTasks: [UnavailableTaskEvidence]
    public let runs: WorkEvidence<RunSnapshot>

    public var workMap: WaveWorkMap {
        WaveWorkMap(objective: wave.goal, chapter: chapter, tasks: tasks)
    }

    enum CodingKeys: String, CodingKey {
        case wave, chapter, tasks, runs
        case metricPortfolio = "metric_portfolio"
        case unavailableTasks = "unavailable_tasks"
    }
}

/// One Home-local harness bundle from `lf runs --json`.
public struct RunSnapshot: Decodable, Sendable, Identifiable, Hashable {
    public let id: String
    public let parentRunId: String?
    public let taskPrId: String?
    public let repo: String?
    public let worktree: String?
    public let subjects: [RunSubjectAttribution]
    public let skill: String?
    public let outcome: String?
    public let started: Int
    public let firstProviderAttemptAt: Int?
    public let ended: Int?
    public let usage: RunUsageSnapshot
    public let evidenceGaps: Int
    public let harness: String
    public let model: String?
    public let surface: String

    enum CodingKeys: String, CodingKey {
        case id, repo, worktree, subjects, skill, outcome, started, ended, usage, harness, model,
            surface
        case taskPrId = "task_pr_id"
        case parentRunId = "parent_run_id"
        case firstProviderAttemptAt = "first_provider_attempt_at"
        case evidenceGaps = "evidence_gaps"
    }
}

public struct RunSubjectAttribution: Codable, Sendable, Hashable {
    public let selector: String
    public let source: String
}

public struct RunUsageSnapshot: Decodable, Sendable, Hashable {
    public let streams: Int
    public let finalStreams: Int
    public let gaps: Int
    public let inputTokens: Int?
    public let outputTokens: Int?
    public let totalInputTokens: Int?
    public let peakInputTokens: Int?
    public let contextWindowTokens: Int?
    public let reasoningTokens: Int?
    public let cacheReadTokens: Int?
    public let cacheWriteTokens: Int?
    public let costUsd: Double?

    enum CodingKeys: String, CodingKey {
        case streams, gaps
        case finalStreams = "final_streams"
        case inputTokens = "input_tokens"
        case outputTokens = "output_tokens"
        case totalInputTokens = "total_input_tokens"
        case peakInputTokens = "peak_input_tokens"
        case contextWindowTokens = "context_window_tokens"
        case reasoningTokens = "reasoning_tokens"
        case cacheReadTokens = "cache_read_tokens"
        case cacheWriteTokens = "cache_write_tokens"
        case costUsd = "cost_usd"
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
