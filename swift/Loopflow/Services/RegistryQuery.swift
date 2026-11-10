// RegistryQuery — typed `lf` reads over the machine registry.
//
// History and explicit lookups are one-shot queries. What a window shows is kept
// current by the same reads `--subscribe` streams, one served subscription per part.
//
// On macOS the Work subscriptions, `ServedRead` reads and `ServedWrite` writes run
// through the local server; other commands run the `lf`
// shipped inside the app.
// Every read decodes shared wire types into app models. There is no HTTP
// fallback; remote reads use the same CLI with explicit Machine selectors.

import Foundation

public enum SessionOpenIntent: String, Codable, Sendable, CaseIterable {
    case resume, start, reveal

    var arguments: [String] {
        switch self {
        case .resume: []
        case .start: ["--start-if-free"]
        case .reveal: ["--reveal-existing"]
        }
    }

    /// Every intent flag a prepared command may carry.
    public static let flags = Set(allCases.flatMap(\.arguments))
}

/// One `lf` query failed — the subprocess errored, or its JSON didn't decode.
/// `stdout` is what a failed command printed before it exited, such as the
/// receipt of a Task filed before its run failed.
public struct RegistryQueryError: LocalizedError, Sendable {
    public let message: String
    public let stdout: String
    public init(_ message: String, stdout: String = "") {
        self.message = message
        self.stdout = stdout
    }
    public var errorDescription: String? { message }
}

/// Runs an `lf` argv (already including the subcommand, e.g. `["wave", "list","--json"]`)
/// and returns captured stdout. Throws on a non-zero exit or spawn failure; a
/// non-zero exit throws `RegistryQueryError` carrying its stdout.
/// `cwd` seeds ambient resolution for verbs that want it (`lf wave status` with no
/// wave); machine-wide reads such as `lf wave list` ignore it.
public typealias RegistryRunner = @Sendable (_ lfArgs: [String], _ cwd: String?) async throws -> String

/// A read Loopflow serves inside the app, encoded as its Rust `Read`: what
/// `argv` prints in `cwd`, given `input` on stdin. A read whose command takes
/// `--subscribe` is also served as that subscription.
public enum ServedRead: Equatable, Sendable, Encodable {
    case processHistory(id: String)
    case creationGet(requestID: String)
    case processList
    case desktopViewList(directory: String)
    case sessionList(directory: String, headless: Bool, after: String?, limit: Int?)
    case taskGet(directory: String, issue: String)
    case taskDetail(directory: String, issue: String)
    case sessionCard(id: String)
    case planBoard(directory: String, wave: String?, task: String?, all: Bool)
    case taskChanges(issue: String, base: String, head: String?)
    case taskDiff(issue: String, path: String?, base: String, head: String?, draft: String?)
    case taskFile(issue: String, path: String)
    case waveList
    case machineSelf
    case desktopPortfolio
    case configSettings(directory: String, wave: String?, skill: String?, graph: String?, step: String?)
    case desktopLayout(directory: String, work: String)
    case repoPeople(directory: String)
    case graphList(directory: String)
    case desktopViewGet(directory: String, expression: String?)
    case taskRuns(directory: String, task: String, after: String?)
    case processGet(id: String)
    case sessionUsage(days: Int, wave: String?, project: String?, task: String?)
    case taskContext(task: String)
    case wavePlan(directory: String, wave: String)
    case taskList(directory: String, wave: String, after: String?)
    case machineList
    case sessionHistory(since: String, limit: Int, wave: String?, project: String?, task: String?)
    case planList(directory: String, wave: String?, task: String?, all: Bool)
    case repoPlan(directory: String)
    case workResolve(directory: String, url: String)
    case repoPath(id: String)
    case waveGraph(directory: String, wave: String?)
    case repoTokens(directory: String, days: Int?)
    case sessionRetention
    case machineDoctor
    /// How failed CI was detected, repaired and landed on this Machine since
    /// `since` (`7d`, `24h` or an RFC 3339 instant).
    case repoCi(since: String, wave: String?, repo: String?)
    /// Every login's state on this Machine; `cached` skips the providers.
    case accountList(target: String?, cached: Bool)
    /// How the repository selects an account for each provider.
    case accountNext(directory: String, repo: String?, default: Bool)
    /// The commands, skills, graphs and Waves under `path`, as `lf list` shows them.
    case skillList(directory: String, path: [String])
    /// The branch's check report as the provider prints it, with its state.
    case prChecks(directory: String, required: Bool)
    /// Requested reviews and review submissions, each with its commit.
    case prReviews(directory: String, pr: String?)
    /// The Task now and whether it reached `until` (`submitted` or
    /// `terminal`); subscribed, it ends after the reading that reached it.
    case taskWait(issue: String, until: String)

    /// `lf plan list --all`: every repository's Waves and Tasks.
    public static let allPlans = ServedRead.planList(directory: "/", wave: nil, task: nil, all: true)

    /// The equivalent `lf` command, `--json` included.
    public var argv: [String] {
        switch self {
        case .processHistory(let id): return ["session", "history", "show", id]
        case .creationGet(let requestID): return ["server", "receipt", requestID, "--json"]
        case .processList:
            return ["process", "list", "--json"]
        case .desktopViewList:
            return ["desktop", "view", "list", "--json"]
        case .sessionList(_, let headless, let after, let limit):
            if let limit {
                return ["session", "list", "--json", "--page"] + (headless ? ["--interactive", "all"] : [])
                    + ["--limit", String(limit)] + (after.map { ["--after", $0] } ?? [])
            }
            return ["session", "list"] + (headless ? ["--interactive", "all"] : []) + ["--json", "--limit", "0"]
        case .taskDetail(_, let issue):
            return ["task", "detail", issue, "--json"]
        case .sessionCard(let id):
            return ["session", "card", id, "--json"]
        case .planBoard(_, let wave, let task, let all):
            return ["plan", "board"] + argument(nil, wave) + argument("--task", task)
                + (all ? ["--all"] : []) + ["--json"]
        case .taskGet(_, let issue):
            return ["task", "show", issue, "--json"]
        case .taskChanges(let issue, let base, let head):
            return ["task", "changes", issue, "--base", base, "--json"] + (head.map { ["--head", $0] } ?? [])
        case .taskDiff(let issue, let path, let base, let head, let draft):
            return ["task", "diff", issue] + (path.map { [$0] } ?? []) + ["--base", base, "--json"]
                + (head.map { ["--head", $0] } ?? []) + (draft == nil ? [] : ["--draft"])
        case .taskFile(let issue, let path):
            return ["task", "file", issue, path, "--json"]
        case .waveList:
            return ["wave", "list", "--all", "--json"]
        case .machineSelf:
            return ["machine", "self", "--json"]
        case .desktopPortfolio:
            return ["desktop", "portfolio", "--json"]
        case .configSettings(_, let wave, let skill, let graph, let step):
            return ["config", "settings", "--json"] + (wave.map { ["--wave", $0] } ?? [])
                + (skill.map { ["--skill", $0] } ?? []) + (graph.map { ["--graph", $0] } ?? [])
                + (step.map { ["--step", $0] } ?? [])
        case .desktopLayout(_, let work):
            return ["desktop", "layout", work]
        case .repoPeople:
            return ["repo", "people", "--json"]
        case .graphList:
            return ["graph", "list", "--json"]
        case .desktopViewGet(_, let expression):
            return ["desktop", "view", "show"] + (expression.map { [$0] } ?? []) + ["--json"]
        case .taskRuns(_, let task, let after):
            return ["task", "runs", task] + (after.map { ["--after", $0] } ?? []) + ["--json"]
        case .processGet(let id):
            return ["process", "show", id, "--json"]
        case .sessionUsage(let days, let wave, let project, let task):
            return ["session", "usage", "--days", String(days)] + Self.work(wave: wave, project: project, task: task) + ["--json"]
        case .taskContext(let task):
            return ["task", "context", task, "--json"]
        case .wavePlan(_, let wave):
            return ["wave", "plan", wave, "--json"]
        case .taskList(_, let wave, let after):
            return ["task", "list", "--closed", "--wave", wave, "--json"] + (after.map { ["--after", $0] } ?? [])
        case .machineList:
            return ["machine", "list", "--json"]
        case .sessionHistory(let since, let limit, let wave, let project, let task):
            return ["session", "history", "--since", since, "--limit", String(limit)]
                + Self.work(wave: wave, project: project, task: task) + ["--json"]
        case .planList(_, let wave, let task, let all):
            return ["plan", "list"] + (wave.map { [$0] } ?? []) + (task.map { ["--task", $0] } ?? [])
                + (all ? ["--all"] : []) + ["--json"]
        case .repoPlan:
            return ["repo", "plan", "--json"]
        case .workResolve(_, let url):
            return ["open", url, "--link", "--json"]
        case .repoPath(let id):
            return ["--repo-id", id, "repo", "path"]
        case .waveGraph(_, let wave):
            return ["wave", "graph", "show", wave ?? "--no-wave", "--json"]
        case .repoTokens(_, let days):
            return ["repo", "tokens", "--json"] + (days.map { ["--days", String($0)] } ?? [])
        case .sessionRetention:
            return ["session", "retention", "--json"]
        case .machineDoctor:
            return ["machine", "doctor", "--json"]
        case .repoCi(let since, let wave, let repo):
            return ["repo", "ci", "--since", since] + argument("--wave", wave) + argument("--repo", repo) + ["--json"]
        case .accountList(let target, let cached):
            return ["account"] + argument(nil, target) + (cached ? ["--cached"] : [])
                + ["--json"]
        case .accountNext(_, let repo, let `default`):
            return ["account", "next"] + argument("--repo", repo) + (`default` ? ["--default"] : []) + ["--json"]
        case .skillList(_, let path):
            return ["list"] + path + ["--json"]
        case .prChecks(_, let required):
            return ["pr", "checks"] + (required ? ["--required"] : []) + ["--json"]
        case .prReviews(_, let pr):
            return ["pr", "reviews"] + argument(nil, pr) + ["--json"]
        case .taskWait(let issue, let until):
            return ["task", "wait", issue, "--until", until, "--timeout", "0s", "--json"]
        }
    }

    private static func work(wave: String?, project: String?, task: String?) -> [String] {
        (wave.map { ["--wave", $0] } ?? []) + (project.map { ["--project", $0] } ?? [])
            + (task.map { ["--task", $0] } ?? [])
    }

    /// The directory `argv` reads from; Rust's `--cwd`.
    public var cwd: String? {
        tagged.fields.first { $0.key == "directory" }?.value as? String
    }

    public var input: String? {
        if case .taskDiff(_, _, _, _, let draft) = self { return draft }
        return nil
    }

    private var tagged: TaggedFields {
        switch self {
        case .processHistory(let id): TaggedFields("process.history", ["id": id])
        case .creationGet(let requestID): TaggedFields("creation.get", ["request_id": requestID])
        case .processList: TaggedFields("process.list")
        case .desktopViewList(let directory): TaggedFields("desktop.view.list", ["directory": directory])
        case .sessionList(let directory, let headless, let after, let limit):
            TaggedFields("session.list", ["directory": directory, "headless": headless, "after": after, "limit": limit])
        case .taskDetail(let directory, let issue):
            TaggedFields("task.detail", ["directory": directory, "issue": issue])
        case .sessionCard(let id):
            TaggedFields("session.card", ["id": id])
        case .planBoard(let directory, let wave, let task, let all):
            TaggedFields("plan.board", ["directory": directory, "wave": wave, "task": task, "all": all])
        case .taskGet(let directory, let issue):
            TaggedFields("task.get", ["directory": directory, "issue": issue])
        case .taskChanges(let issue, let base, let head):
            TaggedFields("task.changes", ["issue": issue, "base": base, "head": head])
        case .taskDiff(let issue, let path, let base, let head, let draft):
            TaggedFields("task.diff", ["issue": issue, "path": path, "base": base, "head": head, "draft": draft])
        case .taskFile(let issue, let path):
            TaggedFields("task.file", ["issue": issue, "path": path])
        case .waveList: TaggedFields("wave.list")
        case .machineSelf: TaggedFields("machine.self")
        case .desktopPortfolio: TaggedFields("desktop.portfolio")
        case .configSettings(let directory, let wave, let skill, let graph, let step):
            TaggedFields("config.settings", ["directory": directory, "wave": wave, "skill": skill, "graph": graph, "step": step])
        case .desktopLayout(let directory, let work):
            TaggedFields("desktop.layout", ["directory": directory, "work": work])
        case .repoPeople(let directory): TaggedFields("repo.people", ["directory": directory])
        case .graphList(let directory): TaggedFields("graph.list", ["directory": directory])
        case .desktopViewGet(let directory, let expression):
            TaggedFields("desktop.view.get", ["directory": directory, "expression": expression])
        case .taskRuns(let directory, let task, let after):
            TaggedFields("task.runs", ["directory": directory, "task": task, "after": after])
        case .processGet(let id): TaggedFields("process.get", ["id": id])
        case .sessionUsage(let days, let wave, let project, let task):
            TaggedFields("session.usage", ["days": days, "wave": wave, "project": project, "task": task])
        case .taskContext(let task): TaggedFields("task.context", ["task": task])
        case .wavePlan(let directory, let wave):
            TaggedFields("wave.plan", ["directory": directory, "wave": wave])
        case .taskList(let directory, let wave, let after):
            TaggedFields("task.list", ["directory": directory, "wave": wave, "after": after, "closed": true])
        case .machineList: TaggedFields("machine.list")
        case .sessionHistory(let since, let limit, let wave, let project, let task):
            TaggedFields("session.history", ["since": since, "limit": limit, "wave": wave, "project": project, "task": task])
        case .planList(let directory, let wave, let task, let all):
            TaggedFields("plan.list", ["directory": directory, "wave": wave, "task": task, "all": all])
        case .repoPlan(let directory): TaggedFields("repo.plan", ["directory": directory])
        case .workResolve(let directory, let url):
            TaggedFields("work.resolve", ["directory": directory, "url": url])
        case .repoPath(let id): TaggedFields("repo.path", ["id": id])
        case .waveGraph(let directory, let wave):
            TaggedFields("wave.graph", ["directory": directory, "wave": wave])
        case .repoTokens(let directory, let days):
            TaggedFields("repo.tokens", ["directory": directory, "days": days])
        case .sessionRetention: TaggedFields("session.retention")
        case .machineDoctor: TaggedFields("machine.doctor")
        case .repoCi(let since, let wave, let repo):
            TaggedFields("repo.ci", ["since": since, "wave": wave, "repo": repo])
        case .accountList(let target, let cached):
            TaggedFields("account.list", ["target": target, "cached": cached])
        case .accountNext(let directory, let repo, let `default`):
            TaggedFields("account.next", ["directory": directory, "repo": repo, "default": `default`])
        case .skillList(let directory, let path): TaggedFields("skill.list", ["directory": directory, "path": path])
        case .prChecks(let directory, let required):
            TaggedFields("pr.checks", ["directory": directory, "required": required])
        case .prReviews(let directory, let pr): TaggedFields("pr.reviews", ["directory": directory, "pr": pr])
        case .taskWait(let issue, let until): TaggedFields("task.wait", ["issue": issue, "until": until])
        }
    }

    public func encode(to encoder: any Encoder) throws {
        try tagged.encode(to: encoder, tag: "kind")
    }
}

/// A write Loopflow makes inside the app, encoded as its Rust `Write`: what
/// `argv` does in `cwd`, given `input` on stdin. Served requests retain their
/// own trace without recording an OS Process.
public enum ServedWrite: Equatable, Sendable, Encodable {
    case taskCreate(directory: String, requestID: String, title: String?, request: String?, wave: String?, graph: String?, session: String?, run: Bool)
    case taskComplete(directory: String, issue: String)
    case taskReopen(directory: String, issue: String)
    case taskAbandon(directory: String, issue: String)
    case taskUpdate(directory: String, issue: String, notes: String)
    case taskClaim(directory: String, issue: String)
    case taskUnassign(directory: String, issue: String)
    /// Start the Task's run detached; `approve` runs every step headless to
    /// `end` and approves Auto-Ship, as `-b` does.
    case taskRun(directory: String, issue: String, graph: String?, from: String?, agent: String?, approve: Bool)
    case taskShip(directory: String, issue: String)
    case taskHold(directory: String, issue: String)
    case taskSteer(directory: String, issue: String, message: String)
    case taskReport(directory: String, issue: String, message: String)
    case taskInterrupt(directory: String, issue: String)
    case taskMove(directory: String, issue: String, wave: String?, machine: String?)
    case taskSave(directory: String, issue: String, path: String, revision: String, content: String)
    /// A steer one Session, every Session of the Wave or, with neither, every
    /// Session of the repository reads at its next boundary.
    case sessionSteer(directory: String, session: String?, wave: String?, message: String)
    case sessionRename(directory: String, id: String, name: String)
    /// With `preview`, the Task a bind would assign, assigning nothing.
    case sessionBind(directory: String, id: String, task: String, preview: Bool)
    /// The repository's conversation when it names no Session, Task or Wave.
    case sessionResume(directory: String, id: String?, task: String?, wave: String?, agent: String?, intent: SessionOpenIntent)
    case sessionNew(directory: String, requestID: String, id: String, agent: String?)
    case waveRename(directory: String, wave: String, name: String)
    case waveRemove(directory: String, wave: String, preview: Bool)
    case waveMachine(directory: String, wave: String, machine: String)
    /// Move the open Sessions the selector names to an agent.
    case sessionMove(directory: String, blocked: Bool, on: String?, agent: String)
    case sessionRecover(directory: String, ids: [String])
    case waveCreate(directory: String, wave: String, objective: String?)
    case waveSync(directory: String, wave: String)
    case wavePlanBind(directory: String, wave: String, project: String)
    case wavePlanConnect(directory: String, wave: String?, all: Bool, teamKey: String?, teamName: String?)
    /// Without `apply`, the plan: nothing moves.
    case wavePlanReteam(directory: String, apply: Bool)
    case graphCustomize(directory: String, name: String)
    /// Started detached: the Process carrying the release.
    case repoRelease(directory: String, version: String?, target: String?)
    case checkoutCommit(directory: String, message: String?, push: Bool, noAdd: Bool, paths: [String], agent: String?)
    /// The plan with `preview`, a finished `continueSync` or `abort`, or the
    /// Process carrying a sync started detached.
    case checkoutSync(directory: String, onto: String?, preview: Bool, manual: Bool, continueSync: Bool, abort: Bool,
                      adopt: Bool)
    case checkoutCreate(directory: String, name: String, preview: Bool, persistent: Bool)
    /// Run from `/`: the Machine's logins, not a repository's.
    case accountDisconnect(provider: String, email: String?)
    /// Served, no browser profile is chosen interactively.
    case accountConnect(provider: String, email: String?, chromeProfile: String?, import: Bool, apiKey: Bool)
    case accountSet(provider: String, email: String?, loginEmail: String?, plan: String?,
                    clearPlan: Bool, paidThrough: String?, clearPaidThrough: Bool, clearCooldown: Bool,
                    chromeProfiles: [String], clearChromeProfiles: Bool)
    /// `file` is a path on the Machine; the token never travels in the request.
    case accountRedeemReset(provider: String, email: String, idempotencyKey: String?, creditId: String?)
    case accountUse(login: String)
    case accountPause(login: String)
    case accountUnpause(login: String)
    /// Started detached and headless; the result is the Process carrying the run.
    case graphRun(directory: String, name: String, message: String?, agent: String?)
    /// Started detached and headless; the result is the Process carrying the run.
    case skillRun(directory: String, name: String, message: String?, agent: String?)
    case prPublish(directory: String, title: String?, body: String?, agent: String?)
    /// Served, no review surface opens.
    case prOpen(directory: String, title: String?, body: String?, agent: String?)
    case prSubmit(directory: String, strict: Bool, worktree: String?, message: String?, title: String?, body: String?,
                  agent: String?)
    case prArm(directory: String, strict: Bool, local: Bool, worktree: String?, message: String?, title: String?,
               body: String?, agent: String?)
    /// Started detached; the result is the Process carrying the landing.
    case prLand(directory: String, waitAndFix: Bool, strict: Bool, local: Bool, worktree: String?, message: String?,
                title: String?, body: String?, agent: String?)
    case prAbandon(directory: String, branch: String?, force: Bool)
    case prReconcile(directory: String)
    case prRequestReview(directory: String, pr: String?, reviewers: [String])
    /// Run from `/`, as the app's `lf` ran it.
    case desktopPortfolioSet(change: [String])
    case desktopDashboardQuery(directory: String, id: String, expression: String, revision: Int64?)
    case desktopDashboardPin(directory: String, id: String, revision: Int64)
    case desktopDashboardUnpin(directory: String, id: String, revision: Int64)
    case desktopDashboardRename(directory: String, id: String, name: String, revision: Int64)
    case desktopDashboardHide(directory: String, id: String, member: String)
    case desktopDashboardShow(directory: String, id: String, member: String)
    case desktopDashboardSlots(directory: String, id: String, revision: Int64, slots: String)
    case desktopDashboardMove(directory: String, board: String, member: String, destination: String)
    case desktopLayoutSave(directory: String, work: String, layout: String)

    /// The equivalent `lf` command, without its `--cwd`.
    public var argv: [String] {
        switch self {
        case .prPublish, .prOpen, .prSubmit, .prArm, .prLand, .prAbandon, .prReconcile, .prRequestReview: return prArgv
        case .accountDisconnect, .accountConnect, .accountSet, .accountRedeemReset, .accountUse, .accountPause, .accountUnpause:
            return accountArgv
        case .graphRun(_, let name, let message, let agent):
            return ["--batch"] + argument("--agent", agent) + ["graph", name] + argument(nil, message)
        case .skillRun(_, let name, let message, let agent):
            return ["--batch"] + argument("--agent", agent) + ["skill", name] + argument(nil, message)
        case .taskCreate(_, let requestID, let title, let request, let wave, let graph, let session, let run):
            var argv = ["task", "create", "--request-id", requestID]
            argv += argument("--title", title)
            argv += argument("--wave", wave)
            argv += argument("--graph", graph)
            argv += argument("--session", session)
            if run { argv.append("--run") }
            argv.append("--json")
            if let request { argv += ["--", request] }
            return argv
        case .taskComplete(_, let issue): return ["task", "complete", issue, "--json"]
        case .taskReopen(_, let issue): return ["task", "reopen", issue, "--json"]
        case .taskAbandon(_, let issue): return ["task", "abandon", issue, "--json"]
        case .taskUpdate(_, let issue, let notes): return ["task", "update", issue, "--notes=\(notes)", "--json"]
        case .taskClaim(_, let issue): return ["task", "claim", issue, "--json"]
        case .taskUnassign(_, let issue): return ["task", "unassign", issue, "--json"]
        case .taskRun(_, let issue, let graph, let from, let agent, let approve):
            return (approve ? ["--batch"] : []) + argument("--agent", agent) + ["task", "run", issue]
                + [graph].compactMap { $0 } + argument("--from", from) + ["--json"]
        case .taskShip(_, let issue): return ["--task", issue, "ship", "--json"]
        case .taskHold(_, let issue): return ["task", "hold", issue, "--json"]
        case .taskSteer(_, let issue, let message): return ["task", "steer", issue, message, "--json"]
        case .taskReport(_, let issue, let message): return ["task", "report", issue, message, "--json"]
        case .taskInterrupt(_, let issue): return ["task", "interrupt", issue, "--json"]
        case .taskMove(_, let issue, let wave, let machine):
            if let machine { return ["task", "move", issue, "--machine", machine, "--json"] }
            return ["task", "move", issue] + (wave.map { ["--wave", $0] } ?? ["--no-wave"]) + ["--json"]
        case .taskSave(_, let issue, let path, let revision, _):
            return ["task", "save", issue, path, "--revision", revision, "--json"]
        case .sessionSteer(_, let session, let wave, let message):
            return argument("--wave", wave) + ["session", "steer"] + [session].compactMap { $0 } + [message, "--json"]
        case .sessionRename(_, let id, let name): return ["session", "rename", "--json", "--", id, name]
        case .sessionBind(_, let id, let task, let preview):
            return ["session", "bind", "--json", "--task", task] + (preview ? ["--dry-run"] : []) + ["--", id]
        case .sessionResume(let directory, let id, let task, let wave, let agent, let intent):
            return Self.prepared(agent, ["session", "resume"] + intent.arguments + [id].compactMap { $0 }
                + argument("--task", task) + argument("--wave", wave),
                repo: id == nil && task == nil && wave == nil ? directory : nil)
        case .sessionNew(_, let requestID, let id, let agent):
            return Self.prepared(agent, ["session", "new", id, "--request-id", requestID], repo: nil)
        case .waveRename(_, let wave, let name): return ["wave", "rename", wave, name, "--json"]
        case .waveRemove(_, let wave, let preview): return ["wave", "remove", wave, "--json"] + (preview ? ["--preview"] : [])
        case .waveMachine(_, let wave, let machine): return ["--wave", wave, "set", "machine", machine, "--json"]
        case .sessionMove(_, let blocked, let on, let agent):
            return ["--agent", agent, "session", "move", "--all"] + (blocked ? ["--blocked"] : [])
                + argument("--on", on) + ["--json"]
        case .sessionRecover(_, let ids): return ["session", "recover", "--json", "--"] + ids
        case .waveCreate(_, let wave, let objective): return ["wave", "create", wave] + argument("--objective", objective) + ["--json"]
        case .waveSync(_, let wave): return ["wave", "sync", wave, "--json"]
        case .wavePlanBind(_, let wave, let project): return ["wave", "plan", "bind", wave, project, "--json"]
        case .wavePlanConnect(_, let wave, let all, let teamKey, let teamName):
            return ["wave", "plan", "connect"] + [wave].compactMap { $0 } + (all ? ["--all"] : [])
                + argument("--team-key", teamKey) + argument("--team-name", teamName) + ["--json"]
        case .wavePlanReteam(_, let apply): return ["wave", "plan", "reteam"] + (apply ? ["--apply"] : []) + ["--json"]
        case .graphCustomize(_, let name): return ["graph", "customize", name, "--json"]
        case .repoRelease(_, let version, let target):
            return ["repo", "release", "run"] + [version].compactMap { $0 } + argument("--target", target) + ["--json"]
        case .checkoutCommit(_, let message, let push, let noAdd, let paths, let agent):
            var argv = argument("--agent", agent) + ["commit"] + argument("--message", message)
            argv += Self.flags([("--push", push), ("--no-add", noAdd)])
            return argv + paths + ["--json"]
        case .checkoutSync(_, let onto, let preview, let manual, let continueSync, let abort, let adopt):
            var argv = ["sync"] + argument(nil, onto)
            argv += Self.flags([("--plan", preview), ("--manual", manual), ("--continue", continueSync)])
            argv += Self.flags([("--abort", abort), ("--adopt", adopt)])
            return argv + ["--json"]
        case .checkoutCreate(_, let name, let preview, let persistent):
            return ["wt", "create", name] + Self.flags([("--plan", preview), ("--persistent", persistent)]) + ["--json"]
        case .desktopPortfolioSet(let change): return ["desktop", "portfolio"] + change + ["--json"]
        case .desktopDashboardQuery(_, let id, let expression, let revision):
            return ["desktop", "dashboard", "query"] + (revision.map { ["--revision=\($0)"] } ?? []) + ["--", id, expression]
        case .desktopDashboardPin(_, let id, let revision):
            return ["desktop", "dashboard", "pin", "--revision=\(revision)", "--", id]
        case .desktopDashboardUnpin(_, let id, let revision):
            return ["desktop", "dashboard", "unpin", "--revision=\(revision)", "--", id]
        case .desktopDashboardRename(_, let id, let name, let revision):
            return ["desktop", "dashboard", "rename", "--revision=\(revision)", "--", "query:" + id, name]
        case .desktopDashboardHide(_, let id, let member):
            return ["desktop", "dashboard", "hide", "query:\(id)/\(member)"]
        case .desktopDashboardShow(_, let id, let member):
            return ["desktop", "dashboard", "show", "query:\(id)/\(member)"]
        case .desktopDashboardSlots(_, let id, let revision, _):
            return ["desktop", "dashboard", "slots", "query:" + id, "--revision=\(revision)"]
        case .desktopDashboardMove(_, let board, let member, let destination):
            return ["desktop", "dashboard", "move", "\(board)/\(member)", "--destination", destination]
        case .desktopLayoutSave(_, let work, _): return ["desktop", "layout", work, "--save"]
        }
    }

    /// `lf desktop portfolio <change>`'s words as Rust's `PortfolioCommand`, tagged by
    /// `action`: `folder [path]`, `manual`, `add|import <paths…>`, `remove <path>`.
    private static func portfolioChange(_ words: [String]) -> JSONValue {
        let action = words.first ?? ""
        let rest = Array(words.dropFirst())
        var fields: [String: JSONValue] = ["action": .string(action)]
        switch action {
        case "folder": fields["path"] = rest.first.map(JSONValue.string) ?? .null
        case "add", "import": fields["paths"] = .array(rest.map(JSONValue.string))
        case "remove": fields["path"] = rest.first.map(JSONValue.string) ?? .null
        default: break
        }
        return .object(fields)
    }

    /// `lf [--repo <repo>] [--agent <agent>] <command> --prepare --json`.
    private static func prepared(_ agent: String?, _ command: [String], repo: String?) -> [String] {
        argument("--repo", repo) + argument("--agent", agent) + command + ["--prepare", "--json"]
    }

    /// The `pr` namespace's spellings.
    private var prArgv: [String] {
        switch self {
        case .prPublish(_, let title, let body, let agent):
            return Self.prArgv("publish", agent: agent, flags: [], worktree: nil, message: nil, title: title, body: body)
        case .prOpen(_, let title, let body, let agent):
            return Self.prArgv("open", agent: agent, flags: [], worktree: nil, message: nil, title: title, body: body)
        case .prSubmit(_, let strict, let worktree, let message, let title, let body, let agent):
            return Self.prArgv("submit", agent: agent, flags: [("--strict", strict)], worktree: worktree, message: message,
                        title: title, body: body)
        case .prArm(_, let strict, let local, let worktree, let message, let title, let body, let agent):
            return Self.prArgv("arm", agent: agent, flags: [("--strict", strict), ("--local", local)], worktree: worktree,
                        message: message, title: title, body: body)
        case .prLand(_, let waitAndFix, let strict, let local, let worktree, let message, let title, let body, let agent):
            return Self.prArgv("land", agent: agent, flags: [("--wait-and-fix", waitAndFix), ("--strict", strict), ("--local", local)],
                        worktree: worktree, message: message, title: title, body: body)
        case .prAbandon(_, let branch, let force):
            var argv = ["pr", "abandon"] + argument(nil, branch)
            if force { argv.append("--force") }
            return argv + ["--json"]
        case .prReconcile: return ["pr", "reconcile", "--json"]
        case .prRequestReview(_, let pr, let reviewers):
            var argv = ["pr", "request-review"] + argument(nil, pr)
            for reviewer in reviewers { argv += ["-r", reviewer] }
            return argv + ["--json"]
        default: return []
        }
    }

    private static func accountTarget(_ provider: String, _ email: String?) -> String {
        email.map { "\(provider)=\($0)" } ?? provider
    }

    /// The `account` namespace's spellings.
    private var accountArgv: [String] {
        switch self {
        case .accountDisconnect(let provider, let email):
            return ["account", "disconnect", Self.accountTarget(provider, email)]
        case .accountConnect(let provider, let email, let chromeProfile, let `import`, let apiKey):
            let argv = ["account", "connect", Self.accountTarget(provider, email)] + argument("--chrome-profile", chromeProfile)
            return argv + Self.flags([("--import", `import`), ("--api-key", apiKey)])
        case .accountSet(let provider, let email, let loginEmail, let plan, let clearPlan, let paidThrough,
                         let clearPaidThrough, let clearCooldown, let chromeProfiles, let clearChromeProfiles):
            var argv = ["account", "set", Self.accountTarget(provider, email)]
            argv += argument("--login-email", loginEmail) + argument("--plan", plan)
            argv += argument("--paid-through", paidThrough)
            argv += Self.flags([("--clear-plan", clearPlan), ("--clear-paid-through", clearPaidThrough),
                                ("--clear-cooldown", clearCooldown), ("--clear-chrome-profiles", clearChromeProfiles)])
            for profile in chromeProfiles { argv += ["--chrome-profile", profile] }
            return argv
        case .accountRedeemReset(let provider, let email, let idempotencyKey, let creditId):
            return ["account", "redeem-reset", Self.accountTarget(provider, email)] + argument("--idempotency-key", idempotencyKey)
                + argument("--credit-id", creditId) + ["--json"]
        case .accountUse(let login): return ["account", "use", login]
        case .accountPause(let login): return ["account", "pause", login]
        case .accountUnpause(let login): return ["account", "unpause", login]
        default: return []
        }
    }

    /// The flags that are on, in order.
    private static func flags(_ flags: [(String, Bool)]) -> [String] {
        flags.filter(\.1).map(\.0)
    }

    /// `lf [--agent] pr <verb> <flags> [--worktree] [--message] [--title] [--body] --json`.
    private static func prArgv(_ verb: String, agent: String?, flags: [(String, Bool)], worktree: String?,
                               message: String?, title: String?, body: String?) -> [String] {
        var argv = argument("--agent", agent) + ["pr", verb] + Self.flags(flags)
        argv += argument("--worktree", worktree) + argument("--message", message)
        argv += argument("--title", title) + argument("--body", body)
        return argv + ["--json"]
    }

    /// The directory `argv` runs in; Rust's `--cwd`.
    public var cwd: String {
        tagged.fields.first { $0.key == "directory" }?.value as? String ?? "/"
    }

    public var input: String? {
        switch self {
        case .taskSave(_, _, _, _, let content): content
        case .desktopDashboardSlots(_, _, _, let slots): slots
        case .desktopLayoutSave(_, _, let layout): layout
        default: nil
        }
    }

    private var tagged: TaggedFields {
        switch self {
        case .taskCreate(let directory, let requestID, let title, let request, let wave, let graph, let session, let run):
            TaggedFields("task.create", ["directory": directory, "request_id": requestID, "title": title,
                "request": request, "wave": wave, "graph": graph, "session": session, "run": run])
        case .taskComplete(let directory, let issue):
            TaggedFields("task.complete", ["directory": directory, "issue": issue])
        case .taskReopen(let directory, let issue):
            TaggedFields("task.reopen", ["directory": directory, "issue": issue])
        case .taskAbandon(let directory, let issue):
            TaggedFields("task.abandon", ["directory": directory, "issue": issue])
        case .taskUpdate(let directory, let issue, let notes):
            TaggedFields("task.update", ["directory": directory, "issue": issue, "notes": notes])
        case .taskClaim(let directory, let issue):
            TaggedFields("task.claim", ["directory": directory, "issue": issue])
        case .taskUnassign(let directory, let issue):
            TaggedFields("task.unassign", ["directory": directory, "issue": issue])
        case .taskRun(let directory, let issue, let graph, let from, let agent, let approve):
            TaggedFields("task.run", ["directory": directory, "issue": issue, "graph": graph, "from": from, "agent": agent,
                                      "approve": approve])
        case .taskShip(let directory, let issue):
            TaggedFields("task.ship", ["directory": directory, "issue": issue])
        case .taskHold(let directory, let issue):
            TaggedFields("task.hold", ["directory": directory, "issue": issue])
        case .taskSteer(let directory, let issue, let message):
            TaggedFields("task.steer", ["directory": directory, "issue": issue, "message": message])
        case .taskReport(let directory, let issue, let message):
            TaggedFields("task.report", ["directory": directory, "issue": issue, "message": message])
        case .taskInterrupt(let directory, let issue):
            TaggedFields("task.interrupt", ["directory": directory, "issue": issue])
        case .taskMove(let directory, let issue, let wave, let machine):
            TaggedFields("task.move", ["directory": directory, "issue": issue, "wave": wave, "machine": machine])
        case .taskSave(let directory, let issue, let path, let revision, let content):
            TaggedFields("task.save", ["directory": directory, "issue": issue, "path": path,
                                       "revision": revision, "content": content])
        case .sessionSteer(let directory, let session, let wave, let message):
            TaggedFields("session.steer", ["directory": directory, "session": session, "wave": wave, "message": message])
        case .sessionRename(let directory, let id, let name):
            TaggedFields("session.rename", ["directory": directory, "id": id, "name": name])
        case .sessionBind(let directory, let id, let task, let preview):
            TaggedFields("session.bind", ["directory": directory, "id": id, "task": task, "preview": preview])
        case .sessionResume(let directory, let id, let task, let wave, let agent, let intent):
            TaggedFields("session.resume", ["directory": directory, "id": id, "task": task, "wave": wave,
                                            "agent": agent, "intent": intent.rawValue])
        case .sessionNew(let directory, let requestID, let id, let agent):
            TaggedFields("session.new", ["directory": directory, "request_id": requestID, "id": id, "agent": agent])
        case .waveRename(let directory, let wave, let name):
            TaggedFields("wave.rename", ["directory": directory, "wave": wave, "name": name])
        case .waveRemove(let directory, let wave, let preview):
            TaggedFields("wave.remove", ["directory": directory, "wave": wave, "preview": preview])
        case .waveMachine(let directory, let wave, let machine):
            TaggedFields("wave.machine", ["directory": directory, "wave": wave, "machine": machine])
        case .sessionMove(let directory, let blocked, let on, let agent):
            TaggedFields("session.move", ["directory": directory, "blocked": blocked, "on": on, "agent": agent])
        case .sessionRecover(let directory, let ids):
            TaggedFields("session.recover", ["directory": directory, "ids": ids])
        case .waveCreate(let directory, let wave, let objective):
            TaggedFields("wave.create", ["directory": directory, "wave": wave, "objective": objective])
        case .waveSync(let directory, let wave):
            TaggedFields("wave.sync", ["directory": directory, "wave": wave])
        case .wavePlanBind(let directory, let wave, let project):
            TaggedFields("wave.plan.bind", ["directory": directory, "wave": wave, "project": project])
        case .wavePlanConnect(let directory, let wave, let all, let teamKey, let teamName):
            TaggedFields("wave.plan.connect", ["directory": directory, "wave": wave, "all": all,
                                                   "team_key": teamKey, "team_name": teamName])
        case .wavePlanReteam(let directory, let apply):
            TaggedFields("wave.plan.reteam", ["directory": directory, "apply": apply])
        case .graphCustomize(let directory, let name):
            TaggedFields("graph.customize", ["directory": directory, "name": name])
        case .repoRelease(let directory, let version, let target):
            TaggedFields("repo.release", ["directory": directory, "version": version, "target": target])
        case .checkoutCommit(let directory, let message, let push, let noAdd, let paths, let agent):
            TaggedFields("checkout.commit", ["directory": directory, "message": message, "push": push, "no_add": noAdd,
                                             "paths": paths, "agent": agent])
        case .checkoutSync(let directory, let onto, let preview, let manual, let continueSync, let abort, let adopt):
            TaggedFields("checkout.sync", ["directory": directory, "onto": onto, "preview": preview, "manual": manual,
                                           "continue": continueSync, "abort": abort, "adopt": adopt])
        case .checkoutCreate(let directory, let name, let preview, let persistent):
            TaggedFields("checkout.create", ["directory": directory, "name": name, "preview": preview,
                                             "persistent": persistent])
        case .accountDisconnect(let provider, let email):
            TaggedFields("account.disconnect", ["provider": provider, "email": email])
        case .accountConnect(let provider, let email, let chromeProfile, let `import`, let apiKey):
            TaggedFields("account.connect", ["provider": provider, "email": email, "chrome_profile": chromeProfile,
                                             "import": `import`, "api_key": apiKey])
        case .accountSet(let provider, let email, let loginEmail, let plan, let clearPlan, let paidThrough,
                         let clearPaidThrough, let clearCooldown, let chromeProfiles, let clearChromeProfiles):
            TaggedFields("account.set", ["provider": provider, "email": email, "login_email": loginEmail,
                                         "plan": plan, "clear_plan": clearPlan,
                                         "paid_through": paidThrough, "clear_paid_through": clearPaidThrough,
                                         "clear_cooldown": clearCooldown, "chrome_profiles": chromeProfiles,
                                         "clear_chrome_profiles": clearChromeProfiles])
        case .accountRedeemReset(let provider, let email, let idempotencyKey, let creditId):
            TaggedFields("account.redeem_reset", ["provider": provider, "email": email,
                                                  "idempotency_key": idempotencyKey, "credit_id": creditId])
        case .accountUse(let login): TaggedFields("account.use", ["login": login])
        case .accountPause(let login): TaggedFields("account.pause", ["login": login])
        case .accountUnpause(let login): TaggedFields("account.unpause", ["login": login])
        case .graphRun(let directory, let name, let message, let agent):
            TaggedFields("graph.run", ["directory": directory, "name": name, "message": message, "agent": agent])
        case .skillRun(let directory, let name, let message, let agent):
            TaggedFields("skill.run", ["directory": directory, "name": name, "message": message, "agent": agent])
        case .prPublish(let directory, let title, let body, let agent):
            TaggedFields("pr.publish", ["directory": directory, "title": title, "body": body, "agent": agent])
        case .prOpen(let directory, let title, let body, let agent):
            TaggedFields("pr.open", ["directory": directory, "title": title, "body": body, "agent": agent])
        case .prSubmit(let directory, let strict, let worktree, let message, let title, let body, let agent):
            TaggedFields("pr.submit", ["directory": directory, "strict": strict, "worktree": worktree,
                                       "message": message, "title": title, "body": body, "agent": agent])
        case .prArm(let directory, let strict, let local, let worktree, let message, let title, let body, let agent):
            TaggedFields("pr.arm", ["directory": directory, "strict": strict, "local": local, "worktree": worktree,
                                    "message": message, "title": title, "body": body, "agent": agent])
        case .prLand(let directory, let waitAndFix, let strict, let local, let worktree, let message, let title,
                     let body, let agent):
            TaggedFields("pr.land", ["directory": directory, "wait_and_fix": waitAndFix, "strict": strict,
                                     "local": local, "worktree": worktree, "message": message, "title": title,
                                     "body": body, "agent": agent])
        case .prAbandon(let directory, let branch, let force):
            TaggedFields("pr.abandon", ["directory": directory, "branch": branch, "force": force])
        case .prReconcile(let directory): TaggedFields("pr.reconcile", ["directory": directory])
        case .prRequestReview(let directory, let pr, let reviewers):
            TaggedFields("pr.request_review", ["directory": directory, "pr": pr, "reviewers": reviewers])
        case .desktopPortfolioSet(let change):
            TaggedFields("desktop.portfolio.set", ["change": Self.portfolioChange(change)])
        case .desktopDashboardQuery(let directory, let id, let expression, let revision):
            TaggedFields("desktop.dashboard.query", ["directory": directory, "id": id, "expression": expression,
                                             "revision": revision])
        case .desktopDashboardPin(let directory, let id, let revision):
            TaggedFields("desktop.dashboard.pin", ["directory": directory, "id": id, "revision": revision])
        case .desktopDashboardUnpin(let directory, let id, let revision):
            TaggedFields("desktop.dashboard.unpin", ["directory": directory, "id": id, "revision": revision])
        case .desktopDashboardRename(let directory, let id, let name, let revision):
            TaggedFields("desktop.dashboard.rename", ["directory": directory, "id": id, "name": name, "revision": revision])
        case .desktopDashboardHide(let directory, let id, let member):
            TaggedFields("desktop.dashboard.hide", ["directory": directory, "id": id, "member": member])
        case .desktopDashboardShow(let directory, let id, let member):
            TaggedFields("desktop.dashboard.show", ["directory": directory, "id": id, "member": member])
        case .desktopDashboardSlots(let directory, let id, let revision, let slots):
            TaggedFields("desktop.dashboard.slots", ["directory": directory, "id": id, "revision": revision, "slots": slots])
        case .desktopDashboardMove(let directory, let board, let member, let destination):
            TaggedFields("desktop.dashboard.move", ["directory": directory, "board": board, "member": member,
                                       "destination": destination])
        case .desktopLayoutSave(let directory, let work, let layout):
            TaggedFields("desktop.layout.save", ["directory": directory, "work": work, "layout": layout])
        }
    }

    public func encode(to encoder: any Encoder) throws {
        try tagged.encode(to: encoder, tag: "kind")
    }
}

/// One variant of a Rust enum serde tags internally: its name beside its
/// fields, `null` for an absent one.
struct TaggedFields {
    let name: String
    let fields: KeyValuePairs<String, (any Encodable)?>

    init(_ name: String, _ fields: KeyValuePairs<String, (any Encodable)?> = [:]) {
        self.name = name
        self.fields = fields
    }

    func encode(to encoder: any Encoder, tag: String) throws {
        var container = encoder.container(keyedBy: Key.self)
        try container.encode(name, forKey: Key(tag))
        for (key, value) in fields {
            if let value {
                try container.encode(value, forKey: Key(key))
            } else {
                try container.encodeNil(forKey: Key(key))
            }
        }
    }

    private struct Key: CodingKey {
        let stringValue: String
        var intValue: Int? { nil }
        init(_ string: String) { stringValue = string }
        init?(stringValue: String) { self.stringValue = stringValue }
        init?(intValue: Int) { nil }
    }
}

public struct RegistryQuery: Sendable {
    public private(set) var remoteMachine: String?
    private var run: RegistryRunner
    private var runWithInput: @Sendable ([String], String?, String) async throws -> String
    /// What a `ServedRead`'s `argv` prints: served in the app when the
    /// transport can, else run.
    private var serve: @Sendable (ServedRead) async throws -> String
    /// What a `ServedWrite`'s `argv` prints: made in the app when the
    /// transport can, else run.
    private var write: @Sendable (ServedWrite) async throws -> String
    private let observeFiles: (@Sendable ([String], String?) async throws -> TaskFilesObservation)?
    private var observeRead: (@Sendable ([String], String?) async throws -> LineSubscription)?
    private let observeWork: (@Sendable () async throws -> WorkObservation)?

    public init(
        runWithInput: @escaping @Sendable ([String], String?, String) async throws -> String = { _, _, _ in
            throw RegistryQueryError("Draft comparison is unavailable on this transport")
        },
        serve: (@Sendable (ServedRead) async throws -> String)? = nil,
        write: (@Sendable (ServedWrite) async throws -> String)? = nil,
        watchFiles: (@Sendable ([String], String?) async throws -> TaskFilesObservation)? = nil,
        subscribeRead: (@Sendable ([String], String?) async throws -> LineSubscription)? = nil,
        subscribeWork: (@Sendable () async throws -> WorkObservation)? = nil,
        run: @escaping RegistryRunner
    ) {
        self.runWithInput = runWithInput
        self.serve = serve ?? { query in
            if let input = query.input { return try await runWithInput(query.argv, query.cwd, input) }
            let output = try await run(query.argv, query.cwd)
            // This CLI spelling renders text; the shared read carries a JSON string.
            if case .processHistory = query {
                return String(decoding: try JSONEncoder().encode(output), as: UTF8.self)
            }
            return output
        }
        self.write = write ?? { change in
            if let input = change.input { return try await runWithInput(change.argv, change.cwd, input) }
            return try await run(change.argv, change.cwd)
        }
        self.run = run
        self.observeFiles = watchFiles
        self.observeWork = subscribeWork
        self.observeRead = subscribeRead
    }

    public func viewPlan(expression: String?, repoPath: String) async throws -> WorkViewPlan {
        try Self.decode(WorkViewPlan.self, from: await serve(.desktopViewGet(directory: repoPath, expression: expression)))
    }

    public func editDashboardQuery(id: String, expression: String, revision: Int64?, repoPath: String) async throws -> Dashboard {
        try Self.decode(Dashboard.self, from: await write(
            .desktopDashboardQuery(directory: repoPath, id: id, expression: expression, revision: revision)))
    }

    public func pinDashboard(id: String, pinned: Bool, revision: Int64, repoPath: String) async throws -> Dashboard {
        try Self.decode(Dashboard.self, from: await write(
            pinned ? .desktopDashboardPin(directory: repoPath, id: id, revision: revision)
                : .desktopDashboardUnpin(directory: repoPath, id: id, revision: revision)))
    }

    public func renameDashboard(id: String, name: String, revision: Int64, repoPath: String) async throws -> Dashboard {
        try Self.decode(Dashboard.self, from: await write(
            .desktopDashboardRename(directory: repoPath, id: id, name: name, revision: revision)))
    }

    /// Hide or show a Dashboard tile in its shared record; hiding keeps its place and mode.
    public func setDashboardTile(id: String, member: WorkWorkspaceKey, hidden: Bool, repoPath: String) async throws -> Dashboard {
        try Self.decode(Dashboard.self, from: await write(
            hidden ? .desktopDashboardHide(directory: repoPath, id: id, member: member.selector)
                : .desktopDashboardShow(directory: repoPath, id: id, member: member.selector)))
    }

    /// Nil restores the default slot layout.
    public func saveDashboardSlots(id: String, slots: DashboardSlotLayout?, revision: Int64, repoPath: String) async throws -> Dashboard {
        let input = String(decoding: try JSONEncoder().encode(slots), as: UTF8.self)
        return try Self.decode(Dashboard.self, from: await write(
            .desktopDashboardSlots(directory: repoPath, id: id, revision: revision, slots: input)))
    }

    /// Move `member`'s tile into `target`'s place in a Dashboard's order, as the
    /// repository order's drop does; Rust applies the move and decides the mode.
    public func reorderDashboard(id: String, member: WorkWorkspaceKey, to target: WorkWorkspaceKey, repoPath: String) async throws -> Dashboard {
        try Self.decode(Dashboard.self, from: await write(
            .desktopDashboardMove(directory: repoPath, board: "query:" + id, member: member.selector, destination: target.selector)))
    }

    /// Move a Working Tasks member into `target`'s place in the repository's one Task order.
    public func reorderTask(_ task: String, to target: String, repoPath: String) async throws -> [String] {
        try Self.decode([String].self, from: await write(
            .desktopDashboardMove(directory: repoPath, board: "working-tasks", member: "task:" + task, destination: "task:" + target)))
    }

    public func paneLayout(work: String, repoPath: String) async throws -> WorkPaneLayout? {
        try Self.decode(WorkPaneLayout?.self, from: await serve(.desktopLayout(directory: repoPath, work: work)))
    }

    public func savePaneLayout(_ layout: WorkPaneLayout, repoPath: String) async throws -> WorkPaneLayout {
        let input = String(decoding: try JSONEncoder().encode(layout), as: UTF8.self)
        return try Self.decode(WorkPaneLayout.self, from: await write(
            .desktopLayoutSave(directory: repoPath, work: layout.work.selector, layout: input)))
    }

    public func portfolio(_ arguments: [String] = []) async throws -> Portfolio {
        if arguments.isEmpty { return try Self.decode(Portfolio.self, from: await serve(.desktopPortfolio)) }
        return try Self.decode(Portfolio.self, from: await write(.desktopPortfolioSet(change: arguments)))
    }

    /// Keep execution selectors off local filesystem paths. The existing CLI owns
    /// connection identity, repository lookup and every operation on that Machine.
    public func onMachine(_ machine: String, repository: String) -> RegistryQuery {
        let prefix = ["--machine", machine, "--repo-id", repository]
        var query = RegistryQuery(runWithInput: { [runWithInput] args, _, input in
            try await runWithInput(prefix + args, nil, input)
        }, watchFiles: { [observeFiles] args, _ in
            guard let observeFiles else { throw RegistryQueryError("Live file observation is unavailable on this transport") }
            return try await observeFiles(prefix + args, nil)
        }, subscribeRead: { [observeRead] args, _ in
            guard let observeRead else { throw RegistryQueryError("Live Task observation is unavailable on this transport") }
            return try await observeRead(prefix + args, nil)
        }) { [run] args, _ in
            try await run(prefix + args, nil)
        }
        query.remoteMachine = machine
        return query
    }

    public func watchTaskFiles(issue: String, checkout: String, request: String) async throws -> TaskFilesObservation {
        guard let observeFiles else { throw RegistryQueryError("Live file observation is unavailable on this transport") }
        return try await observeFiles(["task", "watch-files", issue, "--checkout", checkout, "--request", request], nil)
    }

    /// Reject delayed reads after a recorded owner changes. Neither observation
    /// prepares a checkout or falls back to a path on the displaying Machine.
    public func pinTaskLocation(to location: TaskLocationObservation) -> RegistryQuery {
        let original = self
        let validate: @Sendable () async throws -> Void = {
            let current = try await original.taskLocation(task: location.taskID,
                repository: location.repositoryID, machine: location.machineID)
            guard current.location == location.location else {
                throw RegistryQueryError("The Task checkout changed; reopen the Task. Drafts were retained.")
            }
        }
        var query = self
        query.run = { [run] args, cwd in
            try await validate()
            let output = try await run(args, cwd)
            try await validate()
            return output
        }
        query.runWithInput = { [runWithInput] args, cwd, input in
            try await validate()
            let output = try await runWithInput(args, cwd, input)
            try await validate()
            return output
        }
        query.serve = { [serve] read in
            try await validate()
            let output = try await serve(read)
            try await validate()
            return output
        }
        query.write = { [write] change in
            try await validate()
            let output = try await write(change)
            try await validate()
            return output
        }
        if let observeRead {
            query.observeRead = { args, cwd in
                try await validate()
                let subscription = try await observeRead(args, cwd)
                let lines = AsyncThrowingStream<Data, any Error> { continuation in
                    let reader = Task {
                        do {
                            for try await line in subscription.lines {
                                try await validate()
                                try Task.checkCancellation()
                                continuation.yield(line)
                            }
                            continuation.finish()
                        } catch { continuation.finish(throwing: error) }
                        await subscription.cancel()
                    }
                    continuation.onTermination = { _ in reader.cancel() }
                }
                return LineSubscription(lines: lines, send: subscription.send, cancel: subscription.cancel)
            }
        }
        return query
    }

    /// A request-bound observation, not admission or checkout preparation.
    public func taskLocation(task: String, repository: String, machine: String) async throws -> TaskLocationObservation {
        // The request token is opaque and returns byte-exact; identities compare canonically.
        let request = UUID().uuidString
        let scope = remoteMachine == nil ? ["--repo-id", repository] : []
        let readings = try Self.decode([TaskLocationObservation].self, from: await run(
            scope + ["task", "location", task, "--request", request, "--json"], nil))
        guard readings.count == 1, let reading = readings.first,
              reading.request == request,
              reading.repositoryID == StructuredIdentity.prefixed(repository, "repo_"),
              reading.taskID == StructuredIdentity.prefixed(task, "task_"),
              reading.machineID == StructuredIdentity.prefixed(machine, "home_") else {
            throw RegistryQueryError("Stale or mismatched execution-location observation; nothing was opened.")
        }
        return reading
    }

    /// A copy that also reports each successful read's wire text, so a caller
    /// can retain exactly what it decoded.
    public func recording(_ record: @escaping @Sendable (_ stdout: String) -> Void) -> RegistryQuery {
        var query = self
        query.run = { [run] args, cwd in
            let stdout = try await run(args, cwd)
            record(stdout)
            return stdout
        }
        query.serve = { [serve] read in
            let stdout = try await serve(read)
            record(stdout)
            return stdout
        }
        query.write = { [write] change in
            let stdout = try await write(change)
            record(stdout)
            return stdout
        }
        return query
    }

    /// Explicit workspace opening binds a plain repository to its local plan.
    /// Registration runs the complete CLI command.
    public func repositoryIdentity(cwd: String) async throws -> RepositoryIdentity {
        try Self.decode(RepositoryIdentity.self, from: await run(["config", "repo", "--json"], cwd))
    }

    public func repositoryPath(id: String) async throws -> String {
        try await serve(.repoPath(id: id)).trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// Current Waves across the machine. The shared
    /// reader excludes historical registrations; callers only slice by repo.
    public func allWaves() async throws -> [Wave] {
        let stdout = try await serve(.waveList)
        let snapshots = try Self.decode([WaveSnapshot].self, from: stdout)
        return snapshots.map { $0.toWave() }
    }

    /// One page of a Wave's closed Tasks, most recently closed first. The
    /// Wave's status carries only its open Tasks.
    public func closedTasks(wave: String, after: String? = nil, cwd: String) async throws -> ClosedTaskPage {
        try Self.decode(ClosedTaskPage.self, from: await serve(.taskList(directory: cwd, wave: wave, after: after)))
    }

    /// One Wave's Project/Task work and recorded execution state.
    public func status(wave: String, cwd: String?) async throws
        -> WaveDetailSnapshot {
        // Without a repository the Wave resolves from `/`, where the app runs.
        let stdout = try await serve(.wavePlan(directory: cwd ?? "/", wave: wave))
        return try Self.decode(WaveDetailSnapshot.self, from: stdout)
    }

    /// Refresh one Wave's planning from its provider (Linear, or the
    /// repository's Git planning ref), retrying pending deliveries.
    public func syncPlanning(wave: String, cwd: String) async throws {
        _ = try await write(.waveSync(directory: cwd, wave: wave))
    }

    /// `lf wave create`: registers the Wave, commits its files on the main checkout and
    /// ensures its first Project. Adding an existing Wave changes nothing, so this
    /// is also the explicit opening/retry operation; periodic reads stay observational.
    /// The receipt's `summary` names anything left to finish.
    @discardableResult
    public func addWave(name: String, objective: String? = nil, cwd: String) async throws -> WaveAdded {
        let objective = objective.flatMap { $0.isEmpty ? nil : $0 }
        return try Self.decode(WaveAdded.self, from: await write(.waveCreate(directory: cwd, wave: name, objective: objective)))
    }

    /// `lf wave rename`: same Wave id, Tasks and chapters; its files move on main.
    /// The receipt's `summary` names anything left to finish.
    public func renameWave(wave: String, newName: String, cwd: String) async throws -> WaveRenamed {
        try Self.decode(WaveRenamed.self, from: await write(.waveRename(directory: cwd, wave: wave, name: newName)))
    }

    /// `lf wave remove`: with `preview`, the consequences without changing anything.
    public func removeWave(name: String, preview: Bool, cwd: String) async throws -> WaveRemoval {
        try Self.decode(WaveRemoval.self, from: await write(.waveRemove(directory: cwd, wave: name, preview: preview)))
    }

    public func roadmap(wave: String? = nil) async throws -> RoadmapSnapshot {
        try Self.decode(RoadmapSnapshot.self, from: await serve(
            .planList(directory: "/", wave: wave, task: nil, all: wave == nil)))
    }

    /// Resolve a public Work address through Rust's repository and Work owners.
    /// Reads only: no checkout, Session or window is prepared.
    public func workLink(_ url: URL) async throws -> WorkLink {
        let links = try Self.decode([WorkLink].self, from: await serve(.workResolve(directory: "/", url: url.absoluteString)))
        guard let link = links.first, links.count == 1 else {
            throw RegistryQueryError("The Work link did not resolve to one destination.")
        }
        return link
    }

    /// Without a repository the Task resolves across every repository.
    public func taskDestination(issue: String, repo: String?) async throws -> RoadmapSnapshot {
        try Self.decode(RoadmapSnapshot.self, from: await serve(
            .planList(directory: repo ?? "/", wave: nil, task: issue, all: repo == nil)))
    }

    public func peerPlanningStatus(cwd: String) async throws -> [PeerPlanningStatus] {
        struct Status: Decodable { let destinations: [PeerPlanningStatus] }
        return try Self.decode(Status.self, from: await serve(.repoPlan(directory: cwd))).destinations
    }

    public func localMachineId() async throws -> String {
        struct MachineIdentity: Decodable { let id: String }
        return try Self.decode(MachineIdentity.self, from: await serve(.machineSelf)).id
    }

    /// Whether this transport keeps a workspace current by itself. Without one,
    /// a caller reads once and shows that reading until it asks again.
    public var streamsWork: Bool { observeWork != nil }

    public func subscribeWork() async throws -> WorkObservation {
        guard let observeWork else {
            throw RegistryQueryError("Work observation is unavailable on this transport")
        }
        return try await observeWork()
    }

    /// Durable Work facts across creation, Session history, PR lifecycle, and Steers.
    /// Filters are composed by `lf` before its bounded presentation window.
    public func workActivity(
        since: String = "7d",
        limit: Int = 50,
        wave: String? = nil,
        project: String? = nil,
        task: String? = nil
    ) async throws -> WorkActivitySnapshot {
        try Self.decode(WorkActivitySnapshot.self, from: await serve(
            .sessionHistory(since: since, limit: limit, wave: wave, project: project, task: task)))
    }

    // Task content reads and saves address the Task by issue and run outside its
    // checkout, so a Desktop poll is never counted as the Task's execution.

    /// Files changed by one Task, classified across commits, index, worktree,
    /// and untracked state relative to the Task's recorded base.
    public func taskChanges(issue: String, base: String = "parent", head: String? = nil) async throws -> TaskChangesSnapshot {
        let stdout = try await serve(.taskChanges(issue: issue, base: base, head: head))
        return try Self.decode(TaskChangesSnapshot.self, from: stdout)
    }

    public func processHistory(id: String) async throws -> String {
        try Self.decode(String.self, from: await serve(.processHistory(id: id)))
    }

    /// One store-driven reading keeps position, readiness and run history together.
    public func subscribeTask(task: String) async throws -> LineSubscription {
        guard let observeRead else { throw RegistryQueryError("Live Task observation is unavailable on this transport") }
        let read = ServedRead.taskGet(directory: "/", issue: task)
        return try await observeRead(read.argv + ["--subscribe"], read.cwd)
    }

    /// Every selectable Graph with the topology it would capture, via the shared loader.
    public func graphCatalog(cwd: String?) async throws -> [GraphCatalogEntry] {
        // Without a repository the catalog resolves from `/`, where the app runs.
        let stdout = try await serve(.graphList(directory: cwd ?? "/"))
        return try Self.decode([GraphCatalogEntry].self, from: stdout)
    }

    /// The repository's Task owners with their GitHub users; lf caches each email's lookup.
    public func repoPeople(cwd: String?) async throws -> [RepoPerson] {
        let stdout = try await serve(.repoPeople(directory: cwd ?? "/"))
        return try Self.decode([RepoPerson].self, from: stdout)
    }

    public func customizeGraph(_ name: String, cwd: String?) async throws -> String {
        // Without a repository it customizes from `/`, where the app runs.
        struct Customized: Decodable { let path: String }
        return try Self.decode(Customized.self, from: await write(.graphCustomize(directory: cwd ?? "/", name: name))).path
    }

    /// Run the Task's graph until its next interactive step; returns once it
    /// runs. `graph` takes up another graph; `from` starts at that step instead,
    /// and an interactive `from` only puts the Task there.
    public func runTask(issue: String, graph: String?, from: String? = nil, cwd: String?) async throws {
        _ = try await write(.taskRun(directory: cwd ?? "/", issue: issue, graph: graph, from: from, agent: nil, approve: false))
    }

    /// New Task: Rust files the request in `wave` (nil files it in the
    /// repository), starts it as `lf task run` does and admits its conversation
    /// with the request as the first turn. Returns once the Graph registers.
    public func createTask(request: String, wave: String?, graph: String?, cwd: String?, requestID: String = UUID().uuidString) async throws -> TaskCreateReceipt {
        try Self.decode(TaskCreateReceipt.self, from: await write(.taskCreate(directory: cwd ?? "/", requestID: requestID,
            title: nil, request: request, wave: wave, graph: graph, session: nil, run: true)))
    }

    /// The graph a new Task in `wave`, or the repository when nil, would take up.
    public func waveGraph(wave: String?, cwd: String?) async throws -> WaveGraph {
        try Self.decode(WaveGraph.self, from: await serve(.waveGraph(directory: cwd ?? "/", wave: wave)))
    }

    /// Ship the Task: run the rest of its graph to the end headless, standing
    /// in at each interactive step. Returns once it is shipping; it runs on until the Task lands.
    public func shipTask(issue: String, cwd: String?) async throws {
        _ = try await write(.taskShip(directory: cwd ?? "/", issue: issue))
    }

    /// Withdraw a Task's Auto-Ship approval: its ship process stops and any
    /// auto-merge Loopflow armed is disabled.
    public func stopShipping(issue: String, cwd: String?) async throws {
        _ = try await write(.taskHold(directory: cwd ?? "/", issue: issue))
    }

    public func moveTask(issue: String, wave: String?, cwd: String?) async throws {
        _ = try await write(.taskMove(directory: cwd ?? "/", issue: issue, wave: wave, machine: nil))
    }

    /// Your registered Machines. This Machine appears only when a default names it.
    public func machines() async throws -> [Machine] {
        try Self.decode([Machine].self, from: await serve(.machineList))
    }

    /// Move a Task to `machine` through `lf task move --machine`, or set your
    /// Machine for a Wave through `lf set machine`.
    public func useMachine(_ machine: String, for target: WorkMachineTarget, cwd: String?) async throws {
        switch target {
        case .task(let issue): _ = try await write(.taskMove(directory: cwd ?? "/", issue: issue, wave: nil, machine: machine))
        case .wave(let id): _ = try await write(.waveMachine(directory: cwd ?? "/", wave: id, machine: machine))
        }
    }

    /// `lf task claim` or `lf task unassign`.
    public func assignTask(issue: String, claimed: Bool, cwd: String?) async throws {
        _ = try await write(claimed ? .taskClaim(directory: cwd ?? "/", issue: issue)
                               : .taskUnassign(directory: cwd ?? "/", issue: issue))
    }

    public func completeTask(issue: String, cwd: String?) async throws {
        _ = try await write(.taskComplete(directory: cwd ?? "/", issue: issue))
    }

    public func reopenTask(issue: String, cwd: String?) async throws {
        _ = try await write(.taskReopen(directory: cwd ?? "/", issue: issue))
    }

    public func abandonTask(issue: String, cwd: String?) async throws {
        _ = try await write(.taskAbandon(directory: cwd ?? "/", issue: issue))
    }

    /// Complete Task-attributed Session input/provider history. Read-only; querying
    /// an unstarted Task neither prepares nor starts it.
    public func taskHistory(task: String) async throws -> [SessionHistory] {
        try await usage(days: 0, task: task)
    }

    /// What each recorded step's submitted input was made of, oldest first,
    /// with Task totals. Read-only, from retained local captures.
    public func taskContext(task: String) async throws -> ContextReport {
        let stdout = try await serve(.taskContext(task: task))
        return try Self.decode(ContextReport.self, from: stdout)
    }

    public func updateTaskDirective(id: String, text: String, cwd: String) async throws {
        _ = try await write(.taskUpdate(directory: cwd, issue: id, notes: text))
    }

    /// One Task's complete patch, or the patch for a selected changed file.
    public func taskDiff(
        issue: String,
        path: String?,
        base: String = "parent",
        head: String? = nil,
        draft: String? = nil
    ) async throws -> TaskDiffSnapshot {
        let stdout = try await serve(.taskDiff(issue: issue, path: path, base: base, head: head, draft: draft))
        return try Self.decode(TaskDiffSnapshot.self, from: stdout)
    }

    /// Current contents of one file, constrained to the Task worktree.
    public func taskFile(
        issue: String,
        path: String
    ) async throws -> TaskFileSnapshot {
        let stdout = try await serve(.taskFile(issue: issue, path: path))
        return try Self.decode(TaskFileSnapshot.self, from: stdout)
    }

    public func saveTaskFile(issue: String, path: String, revision: String,
                             content: String) async throws -> TaskFileSave {
        let stdout = try await write(.taskSave(directory: "/", issue: issue, path: path,
                                               revision: revision, content: content))
        return try Self.decode(TaskFileSave.self, from: stdout)
    }

    /// One bounded page, ordered by stable Session identity.
    public func sessionPage(includingHeadless: Bool = false, after: String? = nil,
                            cwd: String? = nil) async throws -> SessionPage {
        try Self.decode(SessionPage.self, from: await serve(
            .sessionList(directory: cwd ?? "/", headless: includingHeadless, after: after, limit: 100)))
    }

    /// The repository or Wave's retained conversation; explicit opening only.
    public func ensurePrimarySession(wave: String? = nil, cwd: String) async throws -> SessionRecord {
        try Self.decode(SessionRecord.self, from: await write(
            .sessionResume(directory: cwd, id: nil, task: nil, wave: wave, agent: nil, intent: .start)))
    }

    /// Select the Task's ongoing conversation. Start may create it; reveal
    /// requires existing history. External ownership offers Take back.
    public func prepareTaskSession(issue: String, cwd: String?, intent: SessionOpenIntent = .start) async throws -> SessionRecord {
        try Self.decode(SessionRecord.self, from: await write(
            .sessionResume(directory: cwd ?? "/", id: nil, task: issue, wave: nil, agent: nil, intent: intent)))
    }

    /// Rust's Start after a refused reveal: only an authoritative absence of
    /// history offers it. A failed Start reports its own failure, and a failed
    /// read offers nothing.
    public func startOffer(refused intent: SessionOpenIntent, issue: String, cwd: String) async -> SessionAction? {
        guard intent == .reveal,
              let report = try? Self.decode(SessionResumeExplanation.self, from: await run(
                  ["session", "resume", "--task", issue, "--explain", "--json"], cwd)),
              report.state == nil, report.action?.intent == .start,
              report.impediments.isEmpty, report.unavailable.isEmpty else { return nil }
        return report.actions.first { $0.kind == .open && $0.unavailableReason == nil }
    }

    /// Prepare without launching. Resume transfers control; Start and reveal never
    /// replace input. Only Start may create the first provider thread.
    public func prepareSession(
        id: String,
        agent: String? = nil,
        cwd: String? = nil,
        intent: SessionOpenIntent = .start
    ) async throws -> SessionRecord {
        try Self.decode(SessionRecord.self, from: await write(
            .sessionResume(directory: cwd ?? "/", id: id, task: nil, wave: nil, agent: agent, intent: intent)))
    }

    /// Give one Session a human-assigned name and return the authoritative
    /// record. The durable Session ID selects the conversation or Graph boundary.
    public func renameSession(
        id: String,
        name: String,
        cwd: String? = nil
    ) async throws -> SessionRecord {
        let stdout = try await write(.sessionRename(directory: cwd ?? "/", id: id, name: name))
        return try Self.decode(SessionRecord.self, from: stdout)
    }

    /// Restart the conversation on a fresh provider thread seeded with what the
    /// last thread knew; the same Session keeps its history.
    public func resetSession(_ session: SessionRecord, agent: String?, cwd: String?, requestID: String = UUID().uuidString) async throws -> SessionRecord {
        try Self.decode(SessionRecord.self, from: await write(
            .sessionNew(directory: cwd ?? "/", requestID: requestID, id: session.id, agent: agent)))
    }

    /// Move the open Sessions whose login cannot take their model now to
    /// `agent` (`harness[:model]`) or `account`: idle ones at their next open,
    /// busy ones after their turn. The report says which went where.
    public func moveBlockedSessions(agent: String, cwd: String?) async throws -> SessionMoveReport {
        try Self.decode(SessionMoveReport.self, from: await write(
            .sessionMove(directory: cwd ?? "/", blocked: true, on: nil, agent: agent)))
    }

    public func recoverSessions(ids: [String], cwd: String?) async throws -> [SessionRecord] {
        try Self.decode([SessionRecord].self, from: await write(.sessionRecover(directory: cwd ?? "/", ids: ids)))
    }

    /// Create a Task around the selected conversation without launching work.
    public func createTaskFromSession(id: String, title: String, cwd: String?, requestID: String = UUID().uuidString) async throws -> TaskCreateReceipt.CreatedTask {
        try Self.decode(TaskCreateReceipt.self, from: await write(
            .taskCreate(directory: cwd ?? "/", requestID: requestID, title: title, request: nil, wave: nil, graph: nil, session: id, run: false))).task
    }

    public func previewSessionBinding(id: String, task: String) async throws -> SessionBindingPreview {
        try Self.decode(SessionBindingPreview.self, from: await write(.sessionBind(directory: "/", id: id, task: task, preview: true)))
    }

    public func bindSession(id: String, taskId: String, cwd: String?) async throws -> SessionRecord {
        let stdout = try await write(.sessionBind(directory: cwd ?? "/", id: id, task: taskId, preview: false))
        return try Self.decode(SessionRecord.self, from: stdout)
    }


    /// Direct provider-authored usage from retained Session history, optionally
    /// drilled through the shared Wave → Project → Task attribution.
    public func usage(
        days: Int = 30,
        wave: String? = nil,
        project: String? = nil,
        task: String? = nil
    ) async throws -> [SessionHistory] {
        try Self.decode([SessionHistory].self, from: await serve(
            .sessionUsage(days: days, wave: wave, project: project, task: task)))
    }

    /// The codebase on disk, as a tree of directories weighted by tokens.
    /// Mirrors Rust `CodeNode`. Runs in `repoPath` — `lf tokens` measures the
    /// repo it is invoked in.
    public func codebase(repoPath: String) async throws -> CodeNode {
        try Self.decode(CodeNode.self, from: await serve(.repoTokens(directory: repoPath, days: nil)))
    }

    /// How big the codebase was on each day it changed. Mirrors Rust
    /// `CodeSnapshot`. Blob counts are cached by sha, so only the first walk of
    /// a window pays to tokenize.
    public func codebaseHistory(repoPath: String, days: Int = 30) async throws -> [CodeSnapshot] {
        try Self.decode([CodeSnapshot].self, from: await serve(.repoTokens(directory: repoPath, days: days)))
    }

    /// How much Session history providers still keep, and what they remove next.
    /// Mirrors Rust `RetentionSummary`.
    public func retention() async throws -> RetentionSummary {
        try Self.decode(RetentionSummary.self, from: await serve(.sessionRetention))
    }

    /// The ledger's self-audit, including continuity and lineage tripwires.
    public func doctor() async throws -> DoctorReport {
        try Self.decode(DoctorReport.self, from: await serve(.machineDoctor))
    }

    /// Decode retained wire text through the decoder live reads use.
    public static func decode<T: Decodable>(_ type: T.Type, from stdout: String) throws -> T {
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

/// `lf task create REQUEST --run --json`: the created Task, the graph it took
/// up and its admitted conversation, nil after a headless run. Both are nil
/// when the run failed after the Task was filed.
public struct TaskCreateReceipt: Decodable, Sendable, Hashable {
    public struct CreatedTask: Decodable, Sendable, Hashable {
        public let id: String
        public let identifier: String
        public let title: String
    }
    public let task: CreatedTask
    public let graph: String?
    public let sessionId: String?
    public let milestone: String

    enum CodingKeys: String, CodingKey {
        case task, graph, milestone
        case sessionId = "session_id"
    }
}

/// `lf wave graph show --json`: the graph a scope's new Tasks take up, resolved
/// in Rust through your, the Wave's and the repository's `graph:`, then `code`.
/// The repository scope has no Wave.
public struct WaveGraph: Decodable, Sendable, Hashable {
    public struct Default: Decodable, Sendable, Hashable {
        public let name: String
        /// `builtin`, `repository`, `personal`, `environment` or `command_line`
        public let source: String
    }
    public let waveId: String?
    public let wave: String?
    public let `default`: Default

    enum CodingKeys: String, CodingKey {
        case wave, `default`
        case waveId = "wave_id"
    }
}

/// Durable execution authority and its mutable observed route.
public struct Machine: Decodable, Sendable, Hashable {
    public let label: String?
    public let id: String
    public let route: String
    public let createdAt: String
    public let observedAt: String

    enum CodingKeys: String, CodingKey {
        case id, route, label
        case createdAt = "created_at"
        case observedAt = "observed_at"
    }
}

/// The Work a Machine control reads and sets: a started Task or a Wave.
public enum WorkMachineTarget: Sendable, Hashable {
    case task(issue: String)
    case wave(id: String)
}

/// `WaveSnapshot` (`lf/commands/waves.rs`) — every field present, Optionals
/// explicit (no serde defaults on the wire). `lf wave list` never lists a removed
/// Wave, so the app row carries no retirement; `lf wave show` reads one.
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
    public let machine: Machine?

    public var displayName: String { toWave().displayName }

    enum CodingKeys: String, CodingKey {
        case id, name, status, goal, repo, machine
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
            parentWaveId: parentWaveId
        )
    }
}

/// `lf task list --done --json`: closed Tasks and the Task to continue after.
public struct ClosedTaskPage: Decodable, Sendable, Hashable {
    public let tasks: [RoadmapTask]
    public let next: String?
}

public struct RoadmapSnapshot: Decodable, Sendable, Hashable {
    public let generatedAt: String
    public let waves: [WaveRoadmap]
    /// Every Task once; each names its Wave.
    public let tasks: WorkEvidence<RoadmapTask>

    enum CodingKeys: String, CodingKey {
        case waves, tasks
        case generatedAt = "generated_at"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        generatedAt = try container.decode(String.self, forKey: .generatedAt)
        let tasks = try container.decode(WorkEvidence<RoadmapTask>.self, forKey: .tasks)
        self.tasks = tasks
        var list = try container.nestedUnkeyedContainer(forKey: .waves)
        var waves: [WaveRoadmap] = []
        while !list.isAtEnd {
            waves.append(try WaveRoadmap(from: list.superDecoder(), roadmapTasks: tasks))
        }
        self.waves = waves
    }
}

public struct WaveRoadmap: Sendable, Hashable {
    /// The graph the Wave's Tasks take up by default.
    public let graph: String?
    public let projectReadiness: ProjectReadiness
    public let wave: WaveSnapshot
    public let metricPortfolio: MetricPortfolio
    public let projects: WorkEvidence<ProjectPlanningSnapshot>
    public var currentProject: ProjectPlanningSnapshot? { projects.currentProject }
    /// This Wave's share of the roadmap's Tasks, which the wire sends once.
    public let tasks: WorkEvidence<RoadmapTask>
    public let unavailableTasks: [UnavailableTaskEvidence]

    public var workMap: WaveWorkMap {
        WaveWorkMap(objective: wave.goal, projects: projects, tasks: tasks)
    }

    enum CodingKeys: String, CodingKey {
        case projectReadiness = "project_readiness"
        case wave, projects, graph
        case metricPortfolio = "metric_portfolio"
        case unavailableTasks = "unavailable_tasks"
    }

    init(from decoder: Decoder, roadmapTasks: WorkEvidence<RoadmapTask>) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        graph = try container.decode(String?.self, forKey: .graph)
        projectReadiness = try container.decode(ProjectReadiness.self, forKey: .projectReadiness)
        let wave = try container.decode(WaveSnapshot.self, forKey: .wave)
        self.wave = wave
        metricPortfolio = try container.decode(MetricPortfolio.self, forKey: .metricPortfolio)
        projects = try container.decode(WorkEvidence<ProjectPlanningSnapshot>.self, forKey: .projects)
        tasks = roadmapTasks.filter { $0.task.waveId == wave.id }
        unavailableTasks = try container.decode([UnavailableTaskEvidence].self, forKey: .unavailableTasks)
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
    public let status: TaskState
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
    public let projectReadiness: ProjectReadiness
    public let wave: WaveSnapshot
    public let projects: WorkEvidence<ProjectPlanningSnapshot>
    public var currentProject: ProjectPlanningSnapshot? { projects.currentProject }
    public let tasks: WorkEvidence<RoadmapTask>
    public let metricPortfolio: MetricPortfolio
    public let unavailableTasks: [UnavailableTaskEvidence]
    public let history: WorkEvidence<SessionHistory>

    public var workMap: WaveWorkMap {
        WaveWorkMap(objective: wave.goal, projects: projects, tasks: tasks)
    }

    enum CodingKeys: String, CodingKey {
        case projectReadiness = "project_readiness"
        case wave, projects, tasks, history
        case metricPortfolio = "metric_portfolio"
        case unavailableTasks = "unavailable_tasks"
    }
}

/// Read-only history beneath a conversation's captured input; not a resumable Run.
public struct SessionHistory: Decodable, Sendable, Identifiable, Hashable {
    public var id: String {
        if let captured { return "\(sessionId):\(captured)" }
        if case .nativeTurn(let thread, let turn, _, _) = providers.first?.reference {
            return "\(sessionId):\(thread.rawValue):\(turn)"
        }
        return sessionId
    }
    public let sessionId: String
    public let captured: Int?
    public let artifactKey: String?
    public let callerArtifactKey: String?
    public let taskPrId: String?
    public let repo: String?
    public let worktree: String?
    public let taskId: String?
    public let waveId: String?
    public let taskIdentifier: String?
    public let workSource: String?
    public let waveName: String?
    public let skill: String?
    public let observedAt: Int
    public let firstProviderAttemptAt: Int?
    public let recordedOutcome: String?
    public let recordedAt: Int?
    public let providers: [ProviderHistory]
    public let usage: SessionUsage
    public let evidenceGaps: Int
    public let harness: String
    public let model: String?
    public let surface: String

    public var status: String {
        if !providers.isEmpty {
            return providers.map { record in
                switch record.reference {
                case .nativeTurn: return record.outcome ?? "Unknown"
                case .recordedAttempt: return "Recorded \(record.outcome ?? "unknown")"
                }
            }.joined(separator: " → ")
        }
        return recordedOutcome.map { "Recorded \($0)" } ?? "Unknown"
    }

    public var workLabel: String {
        if providers.contains(where: { $0.taskId != taskId || $0.waveId != waveId }) {
            return "Mixed/unknown · see provider history"
        }
        if let taskId { return "task/\(taskIdentifier ?? taskId)" }
        if let waveId { return "wave/\(waveName ?? waveId)" }
        return "—"
    }

    enum CodingKeys: String, CodingKey {
        case repo, worktree, skill, providers, usage, harness, model, surface
        case captured
        case sessionId = "session_id", artifactKey = "artifact_key", callerArtifactKey = "caller_artifact_key"
        case taskPrId = "task_pr_id", taskId = "task_id", waveId = "wave_id"
        case taskIdentifier = "task_identifier", waveName = "wave_name", workSource = "work_source"
        case observedAt = "observed_at", firstProviderAttemptAt = "first_provider_attempt_at"
        case recordedOutcome = "recorded_outcome", recordedAt = "recorded_at", evidenceGaps = "evidence_gaps"
    }
}

public struct ProviderHistory: Decodable, Sendable, Hashable {
    public let reference: ProviderHistoryReference
    public let lfProcessId: String?
    public let taskId: String?
    public let waveId: String?
    public let startedAt: Int?
    public let completedAt: Int?
    public let outcome: String?
    public let usage: SessionUsage
    enum CodingKeys: String, CodingKey {
        case reference, outcome, usage
        case lfProcessId = "lf_process_id", taskId = "task_id", waveId = "wave_id"
        case startedAt = "started_at", completedAt = "completed_at"
    }
}

public enum ProviderHistoryReference: Decodable, Sendable, Hashable {
    case nativeTurn(thread: AgentSessionId, turn: String, startSeq: Int?, completionSeq: Int?)
    case recordedAttempt(captured: Int, attemptKey: String)
    private enum Keys: String, CodingKey {
        case kind, thread, turn
        case startSeq = "start_seq", completionSeq = "completion_seq"
        case captured, attemptKey = "attempt_key"
    }
    private enum Kind: String, Decodable { case nativeTurn = "native_turn", recordedAttempt = "recorded_attempt" }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        switch try c.decode(Kind.self, forKey: .kind) {
        case .nativeTurn:
            self = .nativeTurn(thread: try c.decode(AgentSessionId.self, forKey: .thread),
                turn: try c.decode(String.self, forKey: .turn),
                startSeq: try c.decodeIfPresent(Int.self, forKey: .startSeq),
                completionSeq: try c.decodeIfPresent(Int.self, forKey: .completionSeq))
        case .recordedAttempt:
            self = .recordedAttempt(captured: try c.decode(Int.self, forKey: .captured),
                attemptKey: try c.decode(String.self, forKey: .attemptKey))
        }
    }
}

public struct SessionUsage: Decodable, Sendable, Hashable {
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

/// One source of a step's submitted input. `tokens` is nil when the record
/// cannot measure it; that is never zero.
public struct ContextSourceUsage: Decodable, Sendable, Hashable {
    public let source: String
    public let tokens: Int?
    public let count: Int?
    public let authors: [String]
    public let budgetTokens: Int?
    public let overBudget: Bool

    enum CodingKeys: String, CodingKey {
        case source, tokens, count, authors
        case budgetTokens = "budget_tokens"
        case overBudget = "over_budget"
    }
}

public struct StepContext: Decodable, Sendable, Hashable {
    public let input: String
    public let sessionId: String
    public let skill: String?
    public let harness: String
    public let observedAt: Int
    public let sources: [ContextSourceUsage]
    public let assembledTokens: Int?
    public let firstRequestTokens: Int?
    public let peakRequestTokens: Int?
    public let gaps: [String]

    enum CodingKeys: String, CodingKey {
        case input, skill, harness, sources, gaps
        case sessionId = "session_id"
        case observedAt = "observed_at"
        case assembledTokens = "assembled_tokens"
        case firstRequestTokens = "first_request_tokens"
        case peakRequestTokens = "peak_request_tokens"
    }
}

public struct ContextReport: Decodable, Sendable, Hashable {
    public let steps: [StepContext]
    public let totals: [ContextSourceUsage]
}

/// `lf history retention --json`. Mirrors Rust `RetentionSummary` exactly.
public struct RetentionSummary: Codable, Sendable, Equatable {
    public let generatedAt: Int64
    public let sessions: Int
    public let harnesses: [HarnessRetention]
    public let groups: [RetentionGroup]
    public let nextExpiries: [ExpiringSession]
    public let expiringWithinWeek: Int
    public let homes: [HomeRetention]

    enum CodingKeys: String, CodingKey {
        case sessions, harnesses, groups, homes
        case generatedAt = "generated_at"
        case nextExpiries = "next_expiries"
        case expiringWithinWeek = "expiring_within_week"
    }
}

public struct HarnessRetention: Codable, Sendable, Equatable, Identifiable {
    public var id: String { harness }

    public let harness: String
    public let provider: Int
    public let facts: Int
    public let unknown: Int
    public let nextExpiry: Int64?

    enum CodingKeys: String, CodingKey {
        case harness, provider, facts, unknown
        case nextExpiry = "next_expiry"
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(harness, forKey: .harness)
        try container.encode(provider, forKey: .provider)
        try container.encode(facts, forKey: .facts)
        try container.encode(unknown, forKey: .unknown)
        try container.encode(nextExpiry, forKey: .nextExpiry)
    }
}

public enum RetentionState: String, Codable, Sendable, Equatable {
    case provider, facts, unknown
}

public enum AgeBucket: String, Codable, Sendable, Equatable {
    case underWeek = "under_week"
    case underMonth = "under_month"
    case underQuarter = "under_quarter"
    case older
}

public struct RetentionGroup: Codable, Sendable, Equatable {
    public let harness: String
    public let account: String?
    public let age: AgeBucket
    public let state: RetentionState
    public let sessions: Int

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(harness, forKey: .harness)
        try container.encode(account, forKey: .account)
        try container.encode(age, forKey: .age)
        try container.encode(state, forKey: .state)
        try container.encode(sessions, forKey: .sessions)
    }

    enum CodingKeys: String, CodingKey {
        case harness, account, age, state, sessions
    }
}

public struct ExpiringSession: Codable, Sendable, Equatable, Identifiable {
    public var id: String { sessionID }

    public let sessionID: String
    public let title: String
    public let harness: String
    public let account: String?
    public let after: Int64

    enum CodingKeys: String, CodingKey {
        case title, harness, account, after
        case sessionID = "session_id"
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(sessionID, forKey: .sessionID)
        try container.encode(title, forKey: .title)
        try container.encode(harness, forKey: .harness)
        try container.encode(account, forKey: .account)
        try container.encode(after, forKey: .after)
    }
}

/// How long a provider keeps transcripts in one home. Mirrors Rust `ExpiryPolicy`.
public enum ExpiryPolicy: Codable, Sendable, Equatable {
    /// Claude removes conversations idle longer than `cleanupPeriodDays`.
    case claudeCleanup(days: Int, settings: String?)
    case kept

    enum CodingKeys: String, CodingKey {
        case kind, days, settings
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "claude_cleanup":
            self = .claudeCleanup(
                days: try container.decode(Int.self, forKey: .days),
                settings: try container.decodeNil(forKey: .settings)
                    ? nil : try container.decode(String.self, forKey: .settings)
            )
        case "kept":
            self = .kept
        case let kind:
            throw DecodingError.dataCorruptedError(
                forKey: .kind, in: container, debugDescription: "unknown expiry policy \(kind)")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .claudeCleanup(days, settings):
            try container.encode("claude_cleanup", forKey: .kind)
            try container.encode(days, forKey: .days)
            try container.encode(settings, forKey: .settings)
        case .kept:
            try container.encode("kept", forKey: .kind)
        }
    }
}

public struct HomeRetention: Codable, Sendable, Equatable, Identifiable {
    public var id: String { "\(harness):\(home)" }

    public let harness: String
    public let home: String
    public let policy: ExpiryPolicy
    public let transcripts: Int
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

public struct ProjectReadiness: Decodable, Sendable, Hashable {
    public enum State: String, Decodable, Sendable { case unconfigured, unavailable, inactive, ready, terminal }
    public let state: State
    public let projectId: String?
    public let observedAt: Int64?
    public let pendingSuccessor: String?
    public let activation: Activation?

    public struct Activation: Decodable, Sendable, Hashable {
        public let lfProcessId: String
        public let completedAt: Int64?
        public let outcome: String?
        public let error: String?
        enum CodingKeys: String, CodingKey {
            case lfProcessId = "lf_process_id", completedAt = "completed_at", outcome, error
        }
    }
    enum CodingKeys: String, CodingKey {
        case state, activation
        case projectId = "project_id", observedAt = "observed_at", pendingSuccessor = "pending_successor"
    }
}

/// `[flag, value]` for a present value, the bare value with no flag, nothing when absent.
private func argument(_ flag: String?, _ value: String?) -> [String] {
    guard let value else { return [] }
    return (flag.map { [$0] } ?? []) + [value]
}
