// RegistryQuery — discovery and history as `lf` queries over the machine
// registry store, not a streaming center.
//
// The wave model has no telemetry hub (see `scratch/eventing.md`): durable
// facts — which waves exist (running and stopped) and their work
// — are QUERIES against the shared SQLite ledger, served by the daemonless `lf`
// CLI. The Podium re-queries the bounded process snapshot for live output;
// Wave conversation motion remains a per-wave SSE stream (`WaveChatConnection`).
//
// This runs `lf ls/status/roadmap/ps/activity --json` as a subprocess and decodes the wire
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
        let stdout = try await run(["ls", "--all", "--json"], nil)
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
        let stdout = try await run(["status", wave, "--json"], cwd)
        return try Self.decode(WaveDetailSnapshot.self, from: stdout)
    }

    /// Probe one Wave's Home for liveness and the single contextual action.
    /// The app never does SSH — `lf home probe` classifies the Home (local reads
    /// are instant; remote routes run one `lf status` on the target Home)
    /// and returns the shared `HomeRuntimeDto`. Probe on demand per
    /// focused Wave, never once per row.
    public func homeProbe(wave: String, cwd: String?) async throws -> HomeRuntime {
        let stdout = try await run(["home", "probe", wave, "--json"], cwd)
        return try Self.decode(HomeRuntime.self, from: stdout)
    }

    /// Idempotently start a Wave on its placed Home and return its status row.
    public func start(wave: String, cwd: String?) async throws -> [WaveSnapshot] {
        let stdout = try await run(["start", wave, "--json"], cwd)
        let snapshots = try Self.decode([WaveSnapshot].self, from: stdout)
        guard snapshots.count == 1,
              let snapshot = snapshots.first,
              snapshot.name == wave,
              snapshot.live,
              snapshot.endpoint != nil
        else {
            throw RegistryQueryError("lf start \(wave) returned no live Wave receipt")
        }
        return snapshots
    }

    /// Pause or resume new Wave turns without changing listener residency.
    public func setWavePaused(
        wave: String,
        paused: Bool,
        cwd: String?
    ) async throws -> WaveIntentReceipt {
        let verb = paused ? "pause" : "resume"
        let stdout = try await run([verb, wave, "--json"], cwd)
        return try Self.decode(WaveIntentReceipt.self, from: stdout)
    }

    /// Every durable plan row across the machine, joined to the same Task
    /// references and live evidence as `lf status`. One subprocess reads every
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

    /// Live process trees, normalized output rates, and completed provider usage.
    public func processActivity() async throws -> ActivitySnapshot {
        let stdout = try await run(["ps", "--json"], nil)
        return try Self.decode(ActivitySnapshot.self, from: stdout)
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

    /// Bounded history for one conversation epoch. `lf` uses the live backing
    /// when a listener exists and otherwise folds readable local epochs from
    /// the journal; this query never starts a Wave listener.
    public func chatHistory(
        wave: String,
        limit: Int = 12,
        epoch: String? = nil,
        cwd: String?
    ) async throws -> ChatHistorySnapshot {
        var args = [
            "chat", "--history", "--json", "--limit", String(limit), "--wave", wave,
        ]
        if let epoch {
            args.append(contentsOf: ["--epoch", epoch])
        }
        let stdout = try await run(args, cwd)
        return try Self.decode(ChatHistorySnapshot.self, from: stdout)
    }

    /// Files changed by one Task, classified across commits, index, worktree,
    /// and untracked state relative to the Task's recorded base.
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

    /// Resolve one durable Activity Run fact to its latest captured turn.
    public func traceAddress(invocationId: String) async throws -> TraceAddress {
        let stdout = try await run(["trace", invocationId, "--json"], nil)
        let trace = try Self.decode(TraceIndexSnapshot.self, from: stdout)
        guard let turn = trace.turns
            .filter({ $0.invocationId == invocationId })
            .max(by: { $0.ordinal < $1.ordinal })
        else {
            throw RegistryQueryError("Run \(invocationId) has no captured trace turn")
        }
        return TraceAddress(
            runId: trace.traceId,
            invocationId: invocationId,
            turnId: turn.id
        )
    }

    /// User-targeted Ask sessions from the same durable queue as bare `lf`.
    public func userAskAttention(cwd: String? = nil) async throws -> [AskAttentionRecord] {
        // Repo-scoped: the sessions surface shows the sessions for the repository
        // the app is opened to, never a cross-repo aggregate. To see another
        // repo's queue you open Loopflow to that repo (the direct-navigation
        // route), not `--all`.
        let stdout = try await run(["ask", "list", "--user", "--json"], cwd)
        return try Self.decode([AskAttentionRecord].self, from: stdout)
    }

    /// Claim or recover one Ask session and return its exact attach descriptor.
    /// Presentation remains the app's responsibility until `confirmAskPresented`.
    public func prepareAskOpen(
        askId: String,
        cwd: String? = nil
    ) async throws -> InvocationSurfaceRecord {
        let stdout = try await run(
            ["ask", "open", askId, "--prepare", "--json"],
            cwd
        )
        return try Self.decode(InvocationSurfaceRecord.self, from: stdout)
    }

    /// Confirm presentation only for the exact Invocation returned by prepare.
    public func confirmAskPresented(
        askId: String,
        invocationId: String,
        cwd: String? = nil
    ) async throws -> AgentInvocationRecord {
        let stdout = try await run(
            ["ask", "presented", askId, invocationId, "--json"],
            cwd
        )
        return try Self.decode(AgentInvocationRecord.self, from: stdout)
    }

    /// Every recorded non-ended Invocation, with its canonical liveness observation.
    public func activeInvocations() async throws -> [InvocationSurfaceRecord] {
        let stdout = try await run(["invocation", "list", "--active", "--json"], nil)
        return try Self.decode([InvocationSurfaceRecord].self, from: stdout)
    }

    /// Read the generic attach descriptor without changing Invocation liveness.
    public func attachInvocation(invocationId: String) async throws -> InvocationSurfaceRecord {
        let stdout = try await run(["invocation", "attach", invocationId, "--json"], nil)
        return try Self.decode(InvocationSurfaceRecord.self, from: stdout)
    }

    /// A wave's measured bets from the local PM snapshot. Cache-only reads keep
    /// rendering off the network; explicit and scheduled syncs refresh SQLite.
    public func plan(
        wave: String,
        objective: String,
        cwd: String?,
        sync: Bool = false
    ) async throws -> WavePlan {
        let freshness = sync ? "--sync" : "--no-sync"
        let stdout = try await run(["pm", "show", "--wave", wave, "--json", freshness], cwd)
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

    /// The same provider-billed output snapshot rendered by `lf usage`,
    /// `lf ps`, `lf top`, and the Podium.
    public func usage() async throws -> UsageSnapshot {
        let stdout = try await run(["usage", "--json"], nil)
        return try Self.decode(UsageSnapshot.self, from: stdout)
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

    /// One atomic Context Lab population. Rust owns every trace join, token
    /// attribution, revision identity, and representative choice; the app sends
    /// only the filter query and renders the returned snapshot.
    public func contextLab(_ query: InvocationSetQuery) async throws -> ContextLabSnapshot {
        var args = [
            "context", "--json",
            "--started-after", String(query.startedAfter),
            "--started-before", String(query.startedBefore),
        ]
        Self.append(query.repoPaths, flag: "--repo", to: &args)
        Self.append(query.waves, flag: "--wave", to: &args)
        Self.append(query.projects, flag: "--project", to: &args)
        Self.append(query.tasks, flag: "--task", to: &args)
        Self.append(query.flows, flag: "--flow", to: &args)
        Self.append(query.skills, flag: "--skill", to: &args)
        Self.append(query.providers, flag: "--provider", to: &args)
        Self.append(query.models, flag: "--model", to: &args)
        Self.append(query.surfaces, flag: "--surface", to: &args)
        Self.append(query.outcomes.map(\.rawValue), flag: "--outcome", to: &args)
        Self.append(query.captureStates.map(\.rawValue), flag: "--capture-state", to: &args)
        if query.steeredOnly { args.append("--steered-only") }
        if query.currentRevisionOnly { args.append("--current-revision-only") }
        let stdout = try await run(args, nil)
        return try Self.decode(ContextLabSnapshot.self, from: stdout)
    }

    /// Exact local artifacts for one immutable trace address. Unlike Context
    /// Lab's aggregate query, this intentionally opens prompt and conversation
    /// bodies and must only be called after an explicit Open trace action.
    public func traceContent(_ address: TraceAddress) async throws -> TraceContentSnapshot {
        let stdout = try await run([
            "trace", address.runId, "--json", "--content",
            "--invocation", address.invocationId, "--turn", address.turnId,
        ], nil)
        return try Self.decode(TraceContentSnapshot.self, from: stdout)
    }

    private static func append(_ values: [String], flag: String, to args: inout [String]) {
        for value in values {
            args.append(contentsOf: [flag, value])
        }
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
    let items: [PmItemSnapshot]

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
    let flows: ProjectFlowPlanSnapshot
    let krs: [PmKrSnapshot]
    let initiativeIds: [String]
    let teamIds: [String]

    enum CodingKeys: String, CodingKey {
        case id, slug, name, summary, definition, flows, krs
        case initiativeIds = "initiative_ids"
        case teamIds = "team_ids"
    }
}

private struct PmItemSnapshot: Decodable {
    let id: String
    let identifier: String
    let url: String?
    let name: String
    let description: String
    let rank: Int
    let completed: Bool
    let projectId: String
    let project: String
    let teamId: String
    let assignee: String?

    enum CodingKeys: String, CodingKey {
        case id, identifier, url, name, description, rank, completed, project, assignee
        case projectId = "project_id"
        case teamId = "team_id"
    }
}

private struct PmKrSnapshot: Decodable {
    let text: String
    let holds: Bool
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

/// `HomeState` (`engine/wave_home.rs`) — a Home's observed liveness.
public enum HomeState: String, Decodable, Sendable, Equatable {
    case unreachable, stopped, running, unknown
}

/// `HomeActionDto` — the single contextual action a surface should offer, so the
/// UI never branches on `HomeState` itself: Attach when running, Start when
/// reachable-but-stopped, or the actionable reason otherwise.
public enum HomeAction: Decodable, Sendable, Equatable {
    case attach(endpoint: String)
    case start(homeId: String)
    case reason(message: String)

    enum CodingKeys: String, CodingKey {
        case kind, endpoint, homeId = "home_id", message
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "attach":
            self = .attach(endpoint: try container.decode(String.self, forKey: .endpoint))
        case "start":
            self = .start(homeId: try container.decode(String.self, forKey: .homeId))
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

/// `HomeRuntimeDto` — a Wave's Home probed for liveness: authority and route, the state
/// with its evidence, the attach endpoint when running, and the one action.
/// This is the shared contract the conductor renders; the app never probes SSH
/// itself — it calls `lf home probe --json` and `lf start --json`.
public struct HomeRuntime: Decodable, Sendable, Equatable {
    public let home: Home
    public let state: HomeState
    public let reason: String
    public let endpoint: String?
    public let action: HomeAction
}

/// `WaveSnapshot` (`lf/commands/waves.rs`) — every field present, Optionals
/// explicit (no serde defaults on the wire).
public struct WaveSnapshot: Decodable, Sendable, Hashable {
    public let id: String
    public let name: String
    public let status: WorkStatus
    public let current: CurrentWorkObservation
    public let goal: String
    public let repo: String
    public let activeTasks: Int
    public let activeProjects: Int
    public let live: Bool
    public let paused: Bool
    public let enabled: Bool
    public let endpoint: String?
    public let createdAt: String?
    public let parentWaveId: String?
    public let retiredAt: String?
    public let supersededByWaveId: String?
    public let retirementReason: String?
    public let home: Home

    enum CodingKeys: String, CodingKey {
        case id, name, status, current, goal, repo, live, paused, enabled, endpoint, home
        case activeTasks = "active_tasks"
        case activeProjects = "active_projects"
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
            current: current,
            live: live,
            paused: paused,
            enabled: enabled,
            activeTasks: activeTasks,
            activeProjects: activeProjects,
            parentWaveId: parentWaveId,
            retiredAt: retiredAt,
            supersededByWaveId: supersededByWaveId,
            retirementReason: retirementReason
        )
    }
}

/// Receipt returned by `lf pause|resume <wave> --json`.
public struct WaveIntentReceipt: Decodable, Sendable, Equatable {
    public let wave: String
    public let paused: Bool
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
    public let projects: WorkEvidence<RoadmapProject>
    public let unavailableProjects: [UnavailableProjectEvidence]

    enum CodingKeys: String, CodingKey {
        case wave, projects
        case metricPortfolio = "metric_portfolio"
        case unavailableProjects = "unavailable_projects"
    }
}

/// Durable Project Work that cannot join the current PM plan, including
/// non-terminal Tasks stranded under a terminal historical Project.
public struct UnavailableProjectEvidence: Decodable, Sendable, Hashable {
    public let workId: String
    public let projectId: String
    public let projectSlug: String
    public let status: WorkStatus
    public let current: CurrentWorkObservation
    public let owner: WorkNextMoveOwner
    public let reason: String
    public let recovery: String
    public let tasks: [UnavailableTaskEvidence]

    enum CodingKeys: String, CodingKey {
        case status, current, owner, reason, recovery, tasks
        case workId = "work_id"
        case projectId = "project_id"
        case projectSlug = "project_slug"
    }
}

/// Non-terminal durable Task Work whose historical Project is absent from the
/// current PM plan.
public struct UnavailableTaskEvidence: Decodable, Sendable, Hashable {
    public let workId: String
    public let taskId: String
    public let taskIdentifier: String
    public let status: WorkStatus
    public let current: CurrentWorkObservation
    public let owner: WorkNextMoveOwner
    public let reason: String
    public let recovery: String

    enum CodingKeys: String, CodingKey {
        case status, current, owner, reason, recovery
        case workId = "work_id"
        case taskId = "task_id"
        case taskIdentifier = "task_identifier"
    }
}

/// `lf status <wave>` snapshot. Mirrors Rust `WaveDetailSnapshot` without
/// reshaping or dropping fields, so every Wave surface starts from one reading.
public struct WaveDetailSnapshot: Decodable, Sendable {
    public let wave: WaveSnapshot
    public let loopState: String?
    public let projects: [WaveProjectWork]
    public let metricPortfolio: MetricPortfolio
    public let unavailableProjects: [UnavailableProjectEvidence]
    public let runs: WorkEvidence<SkillRunEntry>
    public let attention: WorkEvidence<WaveAttentionItem>
    /// The focused Wave's Home probed for liveness and its one contextual action.
    public let homeRuntime: HomeRuntime

    public var workMap: WaveWorkMap {
        WaveWorkMap(objective: wave.goal, projects: projects)
    }

    enum CodingKeys: String, CodingKey {
        case wave, projects, runs, attention
        case metricPortfolio = "metric_portfolio"
        case homeRuntime = "home_runtime"
        case loopState = "loop_state"
        case unavailableProjects = "unavailable_projects"
    }
}

/// One agent-backed skill invocation from `lf runs --json`. Mirrors Rust
/// `SkillRunEntry`; `lf status` filters the same dataset to one Wave.
public struct SkillRunEntry: Decodable, Sendable, Identifiable, Hashable {
    public let id: String
    public let traceId: String
    public let execId: String
    public let parentExecId: String?
    public let repo: String
    public let worktree: String
    public let wave: String?
    /// Roadmap Project slug that owns this run; nil when unattributed.
    public let project: String?
    /// Roadmap Task's Linear issue identifier (e.g. W2-122) that owns this run;
    /// nil when unattributed. Joins a roadmap row to its runs and trace.
    public let task: String?
    public let flow: String?
    public let skill: String
    public let status: String
    public let started: Int
    public let ended: Int?
    public let turns: Int
    public let systemTokens: Int
    public let taskTokens: Int
    public let suppliedContextTokens: Int
    public let inputTokens: Int?
    public let outputTokens: Int?
    public let reasoningTokens: Int?
    public let cacheReadTokens: Int?
    public let cacheWriteTokens: Int?
    public let costUsd: Double?
    public let durationSecs: Double?
    public let provider: String
    public let model: String?
    public let surface: String
    public let captureStatus: String

    enum CodingKeys: String, CodingKey {
        case id, repo, worktree, wave, project, task, flow, skill, status, started, ended, turns,
            provider, model, surface
        case traceId = "trace_id"
        case execId = "exec_id"
        case parentExecId = "parent_exec_id"
        case systemTokens = "system_tokens"
        case taskTokens = "task_tokens"
        case suppliedContextTokens = "supplied_context_tokens"
        case inputTokens = "input_tokens"
        case outputTokens = "output_tokens"
        case reasoningTokens = "reasoning_tokens"
        case cacheReadTokens = "cache_read_tokens"
        case cacheWriteTokens = "cache_write_tokens"
        case costUsd = "cost_usd"
        case durationSecs = "duration_secs"
        case captureStatus = "capture_status"
    }
}

/// Narrow decoding view over `lf trace <run> --json`. Rust owns the full trace
/// graph; The Podium needs only the immutable ids for an explicit content
/// request.
public struct TraceIndexSnapshot: Decodable, Sendable {
    public let traceId: String
    public let turns: [TraceTurnIndex]

    enum CodingKeys: String, CodingKey {
        case turns
        case traceId = "trace_id"
    }
}

public struct TraceTurnIndex: Decodable, Sendable {
    public let id: String
    public let invocationId: String
    public let ordinal: Int

    enum CodingKeys: String, CodingKey {
        case id, ordinal
        case invocationId = "invocation_id"
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
