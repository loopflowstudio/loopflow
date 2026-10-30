// RegistryQuery — typed `lf` reads over the machine registry.
//
// History and explicit lookups are one-shot queries. What a window shows is kept
// current by the same reads `--watch` streams, one served watch per part.
//
// On macOS the Work watches, `ServedRead` reads and `ServedWrite` writes run
// inside the app through Loopflow's Rust library; other commands run the `lf`
// shipped inside the app.
// Every read decodes shared wire types into app models. There is no HTTP
// fallback; remote reads use the same CLI with explicit Machine selectors.

import Foundation

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
/// `--watch` is also served as that watch.
public enum ServedRead: Equatable, Sendable, Encodable {
    case monitorPs
    case desktopPlans(directory: String)
    case desktopSessions(directory: String, headless: Bool)
    case desktopTask(directory: String, issue: String)
    case taskChanges(issue: String, base: String)
    case taskDiff(issue: String, path: String?, base: String, draft: String?)
    case taskFile(issue: String, path: String)
    case waves
    case machineId
    case portfolio
    case paneLayout(directory: String, work: String)
    case people(directory: String)
    case graphs(directory: String)
    case viewPlan(directory: String, expression: String?)
    case graphProcesses(directory: String, task: String, after: String?)
    case graphProcess(id: String)
    case usage(days: Int, wave: String?, project: String?, task: String?)
    case taskContext(task: String)
    case waveDetail(directory: String, wave: String)
    case closedTasks(directory: String, wave: String, after: String?)
    case machines
    case history(since: String, limit: Int, wave: String?, project: String?, task: String?)
    case roadmap(directory: String, wave: String?, task: String?, all: Bool)
    case planningStatus(directory: String)
    case workLink(directory: String, url: String)
    case repositoryPath(id: String)
    case waveGraph(directory: String, wave: String?)
    case sessionPage(directory: String, headless: Bool, after: String?)
    case tokens(directory: String, days: Int?)
    case retention
    case doctor
    case bindPreview(session: String, task: String)

    /// `lf desktop plan --all`: every repository's Waves and Tasks.
    public static let allPlans = ServedRead.roadmap(directory: "/", wave: nil, task: nil, all: true)

    /// The equivalent `lf` command, `--json` included.
    public var argv: [String] {
        switch self {
        case .monitorPs:
            return ["monitor", "ps", "--json"]
        case .desktopPlans:
            return ["desktop", "plans", "--json"]
        case .desktopSessions(_, let headless):
            return ["desktop", "sessions"] + (headless ? ["--interactive", "all"] : []) + ["--json"]
        case .desktopTask(_, let issue):
            return ["desktop", "task", issue, "--json"]
        case .taskChanges(let issue, let base):
            return ["task", "diff", issue, "--files", "--base", base, "--json"]
        case .taskDiff(let issue, let path, let base, let draft):
            return ["task", "diff", issue] + (path.map { [$0] } ?? []) + ["--base", base, "--json"]
                + (draft == nil ? [] : ["--draft"])
        case .taskFile(let issue, let path):
            return ["task", "file", issue, path, "--json"]
        case .waves:
            return ["wave", "list", "--all", "--current", "--json"]
        case .machineId:
            return ["machine", "id", "--json"]
        case .portfolio:
            return ["portfolio", "--json"]
        case .paneLayout(_, let work):
            return ["desktop", "layout", work]
        case .people:
            return ["ops", "people", "--json"]
        case .graphs:
            return ["graph", "list", "--json"]
        case .viewPlan(_, let expression):
            return ["desktop", "view"] + (expression.map { [$0] } ?? []) + ["--plan", "--json"]
        case .graphProcesses(_, let task, let after):
            return ["graph", "list", "--processes", "--for-task", task] + (after.map { ["--after", $0] } ?? []) + ["--json"]
        case .graphProcess(let id):
            return ["graph", "show", id, "--processes", "--json"]
        case .usage(let days, let wave, let project, let task):
            return ["usage", "--days", String(days)] + Self.work(wave: wave, project: project, task: task) + ["--json"]
        case .taskContext(let task):
            return ["usage", "--days", "0", "--task", task, "--context", "--json"]
        case .waveDetail(_, let wave):
            return ["desktop", "wave", wave, "--json"]
        case .closedTasks(_, let wave, let after):
            return ["desktop", "done", "--wave", wave, "--json"] + (after.map { ["--after", $0] } ?? [])
        case .machines:
            return ["machine", "list", "--json"]
        case .history(let since, let limit, let wave, let project, let task):
            return ["history", "--since", since, "--limit", String(limit)]
                + Self.work(wave: wave, project: project, task: task) + ["--json"]
        case .roadmap(_, let wave, let task, let all):
            return ["desktop", "plan"] + (wave.map { [$0] } ?? []) + (task.map { ["--task", $0] } ?? [])
                + (all ? ["--all"] : []) + ["--json"]
        case .planningStatus:
            return ["config", "planning", "status", "--json"]
        case .workLink(_, let url):
            return ["open", url, "--link", "--json"]
        case .repositoryPath(let id):
            return ["--repository", id, "config", "repository", "--path"]
        case .waveGraph(_, let wave):
            return ["wave", "graph", "show", wave ?? "--no-wave", "--json"]
        case .sessionPage(_, let headless, let after):
            return ["desktop", "sessions", "--json", "--page"] + (headless ? ["--interactive", "all"] : [])
                + ["--limit", "100"] + (after.map { ["--after", $0] } ?? [])
        case .tokens(_, let days):
            return ["ops", "tokens", "--json"] + (days.map { ["--days", String($0)] } ?? [])
        case .retention:
            return ["history", "retention", "--json"]
        case .doctor:
            return ["doctor", "--json"]
        case .bindPreview(let session, let task):
            return ["session", "bind", "--dry-run", "--json", "--task", task, "--", session]
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
        if case .taskDiff(_, _, _, let draft) = self { return draft }
        return nil
    }

    private var tagged: TaggedFields {
        switch self {
        case .monitorPs: TaggedFields("monitor_ps")
        case .desktopPlans(let directory): TaggedFields("desktop_plans", ["directory": directory])
        case .desktopSessions(let directory, let headless):
            TaggedFields("desktop_sessions", ["directory": directory, "headless": headless])
        case .desktopTask(let directory, let issue):
            TaggedFields("desktop_task", ["directory": directory, "issue": issue])
        case .taskChanges(let issue, let base):
            TaggedFields("task_changes", ["issue": issue, "base": base])
        case .taskDiff(let issue, let path, let base, let draft):
            TaggedFields("task_diff", ["issue": issue, "path": path, "base": base, "draft": draft])
        case .taskFile(let issue, let path):
            TaggedFields("task_file", ["issue": issue, "path": path])
        case .waves: TaggedFields("waves")
        case .machineId: TaggedFields("machine_id")
        case .portfolio: TaggedFields("portfolio")
        case .paneLayout(let directory, let work):
            TaggedFields("pane_layout", ["directory": directory, "work": work])
        case .people(let directory): TaggedFields("people", ["directory": directory])
        case .graphs(let directory): TaggedFields("graphs", ["directory": directory])
        case .viewPlan(let directory, let expression):
            TaggedFields("view_plan", ["directory": directory, "expression": expression])
        case .graphProcesses(let directory, let task, let after):
            TaggedFields("graph_processes", ["directory": directory, "task": task, "after": after])
        case .graphProcess(let id): TaggedFields("graph_process", ["id": id])
        case .usage(let days, let wave, let project, let task):
            TaggedFields("usage", ["days": days, "wave": wave, "project": project, "task": task])
        case .taskContext(let task): TaggedFields("task_context", ["task": task])
        case .waveDetail(let directory, let wave):
            TaggedFields("wave_detail", ["directory": directory, "wave": wave])
        case .closedTasks(let directory, let wave, let after):
            TaggedFields("closed_tasks", ["directory": directory, "wave": wave, "after": after])
        case .machines: TaggedFields("machines")
        case .history(let since, let limit, let wave, let project, let task):
            TaggedFields("history", ["since": since, "limit": limit, "wave": wave, "project": project, "task": task])
        case .roadmap(let directory, let wave, let task, let all):
            TaggedFields("roadmap", ["directory": directory, "wave": wave, "task": task, "all": all])
        case .planningStatus(let directory): TaggedFields("planning_status", ["directory": directory])
        case .workLink(let directory, let url):
            TaggedFields("work_link", ["directory": directory, "url": url])
        case .repositoryPath(let id): TaggedFields("repository_path", ["id": id])
        case .waveGraph(let directory, let wave):
            TaggedFields("wave_graph", ["directory": directory, "wave": wave])
        case .sessionPage(let directory, let headless, let after):
            TaggedFields("session_page", ["directory": directory, "headless": headless, "after": after])
        case .tokens(let directory, let days):
            TaggedFields("tokens", ["directory": directory, "days": days])
        case .retention: TaggedFields("retention")
        case .doctor: TaggedFields("doctor")
        case .bindPreview(let session, let task):
            TaggedFields("bind_preview", ["session": session, "task": task])
        }
    }

    public func encode(to encoder: any Encoder) throws {
        try tagged.encode(to: encoder, tag: "read")
    }
}

/// A write Loopflow makes inside the app, encoded as its Rust `Write`: what
/// `argv` does in `cwd`, given `input` on stdin. A Work change records its
/// Process as that `lf` command would.
public enum ServedWrite: Equatable, Sendable, Encodable {
    case taskComplete(directory: String, issue: String)
    case taskReopen(directory: String, issue: String)
    case taskAbandon(directory: String, issue: String)
    case taskNotes(directory: String, issue: String, notes: String)
    case taskOwnership(directory: String, issue: String, claimed: Bool)
    case taskMove(directory: String, issue: String, wave: String?)
    case taskSave(directory: String, issue: String, path: String, revision: String, content: String)
    case sessionRename(directory: String, id: String, name: String)
    case sessionBind(directory: String, id: String, task: String)
    /// The repository's conversation when it names no Session, Task or Wave.
    case sessionResume(directory: String, id: String?, task: String?, wave: String?, agent: String?)
    case sessionNew(directory: String, id: String, agent: String?)
    case waveEnsure(directory: String, wave: String)
    case planningRefresh(directory: String, wave: String)
    case newChapter(directory: String, name: String, plan: String, preview: Bool)
    case graphCustomize(directory: String, name: String)
    /// Run from `/`, as the app's `lf` ran it.
    case portfolio(change: [String])
    case dashboardQuery(directory: String, id: String, expression: String, revision: Int64?)
    case dashboardPin(directory: String, id: String, pinned: Bool, revision: Int64)
    case dashboardRename(directory: String, id: String, name: String, revision: Int64)
    case dashboardTile(directory: String, id: String, member: String, hidden: Bool)
    case dashboardSlots(directory: String, id: String, revision: Int64, slots: String)
    case tileMove(directory: String, board: String, member: String, destination: String)
    case paneLayout(directory: String, work: String, layout: String)

    /// The equivalent `lf` command, without its `--cwd`.
    public var argv: [String] {
        switch self {
        case .taskComplete(_, let issue): ["task", "complete", issue]
        case .taskReopen(_, let issue): ["task", "reopen", issue]
        case .taskAbandon(_, let issue): ["task", "abandon", issue]
        case .taskNotes(_, let issue, let notes): ["task", "edit", issue, "--notes=\(notes)"]
        case .taskOwnership(_, let issue, let claimed): ["task", claimed ? "claim" : "unassign", issue, "--json"]
        case .taskMove(_, let issue, let wave):
            ["task", "move", issue] + (wave.map { ["--wave", $0] } ?? ["--no-wave"])
        case .taskSave(_, let issue, let path, let revision, _):
            ["task", "save", issue, path, "--revision", revision, "--json"]
        case .sessionRename(_, let id, let name): ["session", "rename", "--json", "--", id, name]
        case .sessionBind(_, let id, let task): ["session", "bind", "--json", "--task", task, "--", id]
        case .sessionResume(let directory, let id, let task, let wave, let agent):
            Self.prepared(agent, ["session", "resume"] + [id].compactMap { $0 }
                + Self.option("--task", task) + Self.option("--wave", wave),
                repo: id == nil && task == nil && wave == nil ? directory : nil)
        case .sessionNew(_, let id, let agent):
            Self.prepared(agent, ["session", "new", id], repo: nil)
        case .waveEnsure(_, let wave): ["wave", "ensure", wave, "--json"]
        case .planningRefresh(_, let wave): ["wave", "status", wave, "--sync", "--json"]
        case .newChapter(_, let name, _, let preview):
            ["wave", "new-chapter", "--all", name, "--plan", "/dev/stdin", "--json"] + (preview ? ["--dry-run"] : [])
        case .graphCustomize(_, let name): ["graph", "customize", name]
        case .portfolio(let change): ["portfolio"] + change + ["--json"]
        case .dashboardQuery(_, let id, let expression, let revision):
            ["desktop", "query"] + (revision.map { ["--revision=\($0)"] } ?? []) + ["--", id, expression]
        case .dashboardPin(_, let id, let pinned, let revision):
            ["desktop", pinned ? "pin" : "unpin", "--revision=\(revision)", "--", id]
        case .dashboardRename(_, let id, let name, let revision):
            ["desktop", "rename", "--revision=\(revision)", "--", "query:" + id, name]
        case .dashboardTile(_, let id, let member, let hidden):
            ["desktop", hidden ? "hide" : "show", "-p", "query:\(id)/\(member)"]
        case .dashboardSlots(_, let id, let revision, _):
            ["desktop", "layout", "query:" + id, "--save", "--revision=\(revision)"]
        case .tileMove(_, let board, let member, let destination):
            ["desktop", "move", "-p", "\(board)/\(member)", "--destination", destination]
        case .paneLayout(_, let work, _): ["desktop", "layout", work, "--save"]
        }
    }

    /// `lf [--repo <repo>] [--agent <agent>] <command> --prepare --json`.
    private static func prepared(_ agent: String?, _ command: [String], repo: String?) -> [String] {
        option("--repo", repo) + option("--agent", agent) + command + ["--prepare", "--json"]
    }

    private static func option(_ flag: String, _ value: String?) -> [String] {
        value.map { [flag, $0] } ?? []
    }

    /// The directory `argv` runs in; Rust's `--cwd`.
    public var cwd: String {
        tagged.fields.first { $0.key == "directory" }?.value as? String ?? "/"
    }

    public var input: String? {
        switch self {
        case .taskSave(_, _, _, _, let content): content
        case .newChapter(_, _, let plan, _): plan
        case .dashboardSlots(_, _, _, let slots): slots
        case .paneLayout(_, _, let layout): layout
        default: nil
        }
    }

    private var tagged: TaggedFields {
        switch self {
        case .taskComplete(let directory, let issue):
            TaggedFields("task_complete", ["directory": directory, "issue": issue])
        case .taskReopen(let directory, let issue):
            TaggedFields("task_reopen", ["directory": directory, "issue": issue])
        case .taskAbandon(let directory, let issue):
            TaggedFields("task_abandon", ["directory": directory, "issue": issue])
        case .taskNotes(let directory, let issue, let notes):
            TaggedFields("task_notes", ["directory": directory, "issue": issue, "notes": notes])
        case .taskOwnership(let directory, let issue, let claimed):
            TaggedFields("task_ownership", ["directory": directory, "issue": issue, "claimed": claimed])
        case .taskMove(let directory, let issue, let wave):
            TaggedFields("task_move", ["directory": directory, "issue": issue, "wave": wave])
        case .taskSave(let directory, let issue, let path, let revision, let content):
            TaggedFields("task_save", ["directory": directory, "issue": issue, "path": path,
                                       "revision": revision, "content": content])
        case .sessionRename(let directory, let id, let name):
            TaggedFields("session_rename", ["directory": directory, "id": id, "name": name])
        case .sessionBind(let directory, let id, let task):
            TaggedFields("session_bind", ["directory": directory, "id": id, "task": task])
        case .sessionResume(let directory, let id, let task, let wave, let agent):
            TaggedFields("session_resume", ["directory": directory, "id": id, "task": task, "wave": wave,
                                            "agent": agent])
        case .sessionNew(let directory, let id, let agent):
            TaggedFields("session_new", ["directory": directory, "id": id, "agent": agent])
        case .waveEnsure(let directory, let wave):
            TaggedFields("wave_ensure", ["directory": directory, "wave": wave])
        case .planningRefresh(let directory, let wave):
            TaggedFields("planning_refresh", ["directory": directory, "wave": wave])
        case .newChapter(let directory, let name, let plan, let preview):
            TaggedFields("new_chapter", ["directory": directory, "name": name, "plan": plan, "preview": preview])
        case .graphCustomize(let directory, let name):
            TaggedFields("graph_customize", ["directory": directory, "name": name])
        case .portfolio(let change):
            TaggedFields("portfolio", ["change": change])
        case .dashboardQuery(let directory, let id, let expression, let revision):
            TaggedFields("dashboard_query", ["directory": directory, "id": id, "expression": expression,
                                             "revision": revision])
        case .dashboardPin(let directory, let id, let pinned, let revision):
            TaggedFields("dashboard_pin", ["directory": directory, "id": id, "pinned": pinned, "revision": revision])
        case .dashboardRename(let directory, let id, let name, let revision):
            TaggedFields("dashboard_rename", ["directory": directory, "id": id, "name": name, "revision": revision])
        case .dashboardTile(let directory, let id, let member, let hidden):
            TaggedFields("dashboard_tile", ["directory": directory, "id": id, "member": member, "hidden": hidden])
        case .dashboardSlots(let directory, let id, let revision, let slots):
            TaggedFields("dashboard_slots", ["directory": directory, "id": id, "revision": revision, "slots": slots])
        case .tileMove(let directory, let board, let member, let destination):
            TaggedFields("tile_move", ["directory": directory, "board": board, "member": member,
                                       "destination": destination])
        case .paneLayout(let directory, let work, let layout):
            TaggedFields("pane_layout", ["directory": directory, "work": work, "layout": layout])
        }
    }

    public func encode(to encoder: any Encoder) throws {
        try tagged.encode(to: encoder, tag: "write")
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
    private let observeWork: (@Sendable () async throws -> WorkObservation)?

    public init(
        runWithInput: @escaping @Sendable ([String], String?, String) async throws -> String = { _, _, _ in
            throw RegistryQueryError("Draft comparison is unavailable on this transport")
        },
        serve: (@Sendable (ServedRead) async throws -> String)? = nil,
        write: (@Sendable (ServedWrite) async throws -> String)? = nil,
        watchFiles: (@Sendable ([String], String?) async throws -> TaskFilesObservation)? = nil,
        watchWork: (@Sendable () async throws -> WorkObservation)? = nil,
        run: @escaping RegistryRunner
    ) {
        self.runWithInput = runWithInput
        self.serve = serve ?? { query in
            if let input = query.input { return try await runWithInput(query.argv, query.cwd, input) }
            return try await run(query.argv, query.cwd)
        }
        self.write = write ?? { change in
            if let input = change.input { return try await runWithInput(change.argv, change.cwd, input) }
            return try await run(change.argv, change.cwd)
        }
        self.run = run
        self.observeFiles = watchFiles
        self.observeWork = watchWork
    }

    public func viewPlan(expression: String?, repoPath: String) async throws -> WorkViewPlan {
        try Self.decode(WorkViewPlan.self, from: await serve(.viewPlan(directory: repoPath, expression: expression)))
    }

    public func editDashboardQuery(id: String, expression: String, revision: Int64?, repoPath: String) async throws -> Dashboard {
        try Self.decode(Dashboard.self, from: await write(
            .dashboardQuery(directory: repoPath, id: id, expression: expression, revision: revision)))
    }

    public func pinDashboard(id: String, pinned: Bool, revision: Int64, repoPath: String) async throws -> Dashboard {
        try Self.decode(Dashboard.self, from: await write(
            .dashboardPin(directory: repoPath, id: id, pinned: pinned, revision: revision)))
    }

    public func renameDashboard(id: String, name: String, revision: Int64, repoPath: String) async throws -> Dashboard {
        try Self.decode(Dashboard.self, from: await write(
            .dashboardRename(directory: repoPath, id: id, name: name, revision: revision)))
    }

    /// Hide or show a Dashboard tile in its shared record; hiding keeps its place and mode.
    public func setDashboardTile(id: String, member: WorkWorkspaceKey, hidden: Bool, repoPath: String) async throws -> Dashboard {
        try Self.decode(Dashboard.self, from: await write(
            .dashboardTile(directory: repoPath, id: id, member: member.selector, hidden: hidden)))
    }

    /// Nil restores the default slot layout.
    public func saveDashboardSlots(id: String, slots: DashboardSlotLayout?, revision: Int64, repoPath: String) async throws -> Dashboard {
        let input = String(decoding: try JSONEncoder().encode(slots), as: UTF8.self)
        return try Self.decode(Dashboard.self, from: await write(
            .dashboardSlots(directory: repoPath, id: id, revision: revision, slots: input)))
    }

    /// Move `member`'s tile into `target`'s place in a Dashboard's order, as the
    /// repository order's drop does; Rust applies the move and decides the mode.
    public func reorderDashboard(id: String, member: WorkWorkspaceKey, to target: WorkWorkspaceKey, repoPath: String) async throws -> Dashboard {
        try Self.decode(Dashboard.self, from: await write(
            .tileMove(directory: repoPath, board: "query:" + id, member: member.selector, destination: target.selector)))
    }

    /// Move a Working Tasks member into `target`'s place in the repository's one Task order.
    public func reorderTask(_ task: String, to target: String, repoPath: String) async throws -> [String] {
        try Self.decode([String].self, from: await write(
            .tileMove(directory: repoPath, board: "working-tasks", member: "task:" + task, destination: "task:" + target)))
    }

    public func paneLayout(work: String, repoPath: String) async throws -> WorkPaneLayout? {
        try Self.decode(WorkPaneLayout?.self, from: await serve(.paneLayout(directory: repoPath, work: work)))
    }

    public func savePaneLayout(_ layout: WorkPaneLayout, repoPath: String) async throws -> WorkPaneLayout {
        let input = String(decoding: try JSONEncoder().encode(layout), as: UTF8.self)
        return try Self.decode(WorkPaneLayout.self, from: await write(
            .paneLayout(directory: repoPath, work: layout.work.selector, layout: input)))
    }

    public func portfolio(_ arguments: [String] = []) async throws -> Portfolio {
        if arguments.isEmpty { return try Self.decode(Portfolio.self, from: await serve(.portfolio)) }
        return try Self.decode(Portfolio.self, from: await write(.portfolio(change: arguments)))
    }

    /// Keep execution selectors off local filesystem paths. The existing CLI owns
    /// connection identity, repository lookup and every operation on that Machine.
    public func onMachine(_ machine: String, repository: String) -> RegistryQuery {
        let prefix = ["--machine", machine, "--repository", repository]
        var query = RegistryQuery(runWithInput: { [runWithInput] args, _, input in
            try await runWithInput(prefix + args, nil, input)
        }, watchFiles: { [observeFiles] args, _ in
            guard let observeFiles else { throw RegistryQueryError("Live file observation is unavailable on this transport") }
            return try await observeFiles(prefix + args, nil)
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
        return query
    }

    /// A request-bound observation, not admission or checkout preparation.
    public func taskLocation(task: String, repository: String, machine: String) async throws -> TaskLocationObservation {
        // The request token is opaque and returns byte-exact; identities compare canonically.
        let request = UUID().uuidString
        let readings = try Self.decode([TaskLocationObservation].self, from: await run(
            ["task", "location", task, "--request", request, "--json"], nil))
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
    /// Registration writes the store, so `lf` runs it, not the app's library.
    public func repositoryIdentity(cwd: String) async throws -> RepositoryIdentity {
        try Self.decode(RepositoryIdentity.self, from: await run(["config", "repository", "--json"], cwd))
    }

    public func repositoryPath(id: String) async throws -> String {
        try await serve(.repositoryPath(id: id)).trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// Current Waves across the machine. The shared
    /// reader excludes historical registrations; callers only slice by repo.
    public func allWaves() async throws -> [Wave] {
        let stdout = try await serve(.waves)
        let snapshots = try Self.decode([WaveSnapshot].self, from: stdout)
        return snapshots.map { $0.toWave() }
    }

    /// One page of a Wave's closed Tasks, most recently closed first. The
    /// Wave's status carries only its open Tasks.
    public func closedTasks(wave: String, after: String? = nil, cwd: String) async throws -> ClosedTaskPage {
        try Self.decode(ClosedTaskPage.self, from: await serve(.closedTasks(directory: cwd, wave: wave, after: after)))
    }

    /// One Wave's Project/Task work and recorded execution state.
    public func status(wave: String, cwd: String?) async throws
        -> WaveDetailSnapshot {
        // Without a repository the Wave resolves from `/`, where the app runs.
        let stdout = try await serve(.waveDetail(directory: cwd ?? "/", wave: wave))
        return try Self.decode(WaveDetailSnapshot.self, from: stdout)
    }


    /// Refresh one Wave's planning from its provider (Linear, or the
    /// repository's Git planning ref), retrying pending deliveries.
    public func syncPlanning(wave: String, cwd: String) async throws {
        _ = try await write(.planningRefresh(directory: cwd, wave: wave))
    }

    /// Explicit opening/retry operation. Periodic status reads remain observational.
    public func ensureProject(wave: String, cwd: String) async throws {
        _ = try await write(.waveEnsure(directory: cwd, wave: wave))
    }

    public func realignProjects(name: String, plan: String, preview: Bool, cwd: String) async throws -> ProjectRotationPreview {
        try Self.decode(ProjectRotationPreview.self, from: await write(
            .newChapter(directory: cwd, name: name, plan: plan, preview: preview)))
    }

    public func roadmap(wave: String? = nil) async throws -> RoadmapSnapshot {
        try Self.decode(RoadmapSnapshot.self, from: await serve(
            .roadmap(directory: "/", wave: wave, task: nil, all: wave == nil)))
    }

    /// Resolve a public Work address through Rust's repository and Work owners.
    /// Reads only: no checkout, Session or window is prepared.
    public func workLink(_ url: URL) async throws -> WorkLink {
        let links = try Self.decode([WorkLink].self, from: await serve(.workLink(directory: "/", url: url.absoluteString)))
        guard let link = links.first, links.count == 1 else {
            throw RegistryQueryError("The Work link did not resolve to one destination.")
        }
        return link
    }

    /// Without a repository the Task resolves across every repository.
    public func taskDestination(issue: String, repo: String?) async throws -> RoadmapSnapshot {
        try Self.decode(RoadmapSnapshot.self, from: await serve(
            .roadmap(directory: repo ?? "/", wave: nil, task: issue, all: repo == nil)))
    }

    public func peerPlanningStatus(cwd: String) async throws -> [PeerPlanningStatus] {
        struct Status: Decodable { let destinations: [PeerPlanningStatus] }
        return try Self.decode(Status.self, from: await serve(.planningStatus(directory: cwd))).destinations
    }

    public func localMachineId() async throws -> String {
        struct MachineIdentity: Decodable { let id: String }
        return try Self.decode(MachineIdentity.self, from: await serve(.machineId)).id
    }

    /// Whether this transport keeps a workspace current by itself. Without one,
    /// a caller reads once and shows that reading until it asks again.
    public var streamsWork: Bool { observeWork != nil }

    public func watchWork() async throws -> WorkObservation {
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
            .history(since: since, limit: limit, wave: wave, project: project, task: task)))
    }

    // Task content reads and saves address the Task by issue and run outside its
    // checkout, so a Desktop poll is never counted as the Task's execution.

    /// Files changed by one Task, classified across commits, index, worktree,
    /// and untracked state relative to the Task's recorded base.
    public func taskChanges(issue: String, base: String = "parent") async throws -> TaskChangesSnapshot {
        let stdout = try await serve(.taskChanges(issue: issue, base: base))
        return try Self.decode(TaskChangesSnapshot.self, from: stdout)
    }

    /// Read a Task's captured Graphs through its content-owned transport.
    public func taskGraphProcesses(task: String) async throws -> [GraphProcessDetail] {
        struct Page: Decodable { let entries: [GraphProcessInventoryEntry]; let next: String? }
        var after: String?
        var result: [GraphProcessDetail] = []
        repeat {
            // Without a repository the inventory resolves from `/`, where the app runs.
            let page = try Self.decode(Page.self, from: await serve(.graphProcesses(directory: "/", task: task, after: after)))
            for entry in page.entries {
                let detail = try Self.decode(GraphProcessDetail.self, from: await serve(.graphProcess(id: entry.id)))
                result.append(detail)
            }
            after = page.next
        } while after != nil
        return result
    }

    /// Every selectable Graph with the topology it would capture, via the shared loader.
    public func graphCatalog(cwd: String?) async throws -> [GraphCatalogEntry] {
        // Without a repository the catalog resolves from `/`, where the app runs.
        let stdout = try await serve(.graphs(directory: cwd ?? "/"))
        return try Self.decode([GraphCatalogEntry].self, from: stdout)
    }

    /// The repository's Task owners with their GitHub users; lf caches each email's lookup.
    public func repoPeople(cwd: String?) async throws -> [RepoPerson] {
        let stdout = try await serve(.people(directory: cwd ?? "/"))
        return try Self.decode([RepoPerson].self, from: stdout)
    }

    public func customizeGraph(_ name: String, cwd: String?) async throws -> String {
        // Without a repository it customizes from `/`, where the app runs.
        try await write(.graphCustomize(directory: cwd ?? "/", name: name)).trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// Run the Task's graph until its next interactive step; returns once it
    /// runs. `graph` takes up another graph; `from` starts at that step instead,
    /// and an interactive `from` only puts the Task there.
    public func runTask(issue: String, graph: String?, from: String? = nil, cwd: String?) async throws {
        var args = ["task", "run", issue]
        if let graph { args.append(graph) }
        if let from { args += ["--from", from] }
        _ = try await run(args, cwd)
    }

    /// New Task: Rust files the request in `wave` (nil files it in the
    /// repository), starts it as `lf task run` does and admits its conversation
    /// with the request as the first turn. Returns once the Graph registers.
    /// A run that fails after the Task was filed throws `TaskCreateRunFailed`.
    public func createTask(request: String, wave: String?, graph: String?, cwd: String?) async throws -> TaskCreateReceipt {
        var args = ["task", "create"] + (wave.map { ["--wave", $0] } ?? ["--no-wave"])
        if let graph { args += ["--graph", graph] }
        args += ["--run", "--json", "--", request]
        let stdout: String
        do { stdout = try await run(args, cwd) } catch let error as RegistryQueryError {
            guard let receipt = try? Self.decode(TaskCreateReceipt.self, from: error.stdout) else { throw error }
            throw TaskCreateRunFailed(receipt: receipt, message: error.message)
        }
        return try Self.decode(TaskCreateReceipt.self, from: stdout)
    }

    /// The graph a new Task in `wave`, or the repository when nil, would take up.
    public func waveGraph(wave: String?, cwd: String?) async throws -> WaveGraph {
        try Self.decode(WaveGraph.self, from: await serve(.waveGraph(directory: cwd ?? "/", wave: wave)))
    }

    /// Ship the Task: run the rest of its graph to the end headless, standing
    /// in at each interactive step. Returns once it is shipping; it runs on until the Task lands.
    public func shipTask(issue: String, cwd: String?) async throws {
        _ = try await run(["--task", issue, "ship"], cwd)
    }

    /// Withdraw a Task's Auto-Ship approval: its ship process stops and any
    /// auto-merge Loopflow armed is disabled.
    public func stopShipping(issue: String, cwd: String?) async throws {
        _ = try await run(["--task", issue, "ship", "--stop"], cwd)
    }

    /// Reopen every conversation on this Machine that a restart or crash cut off,
    /// each in its own owner with no viewer; panes attach when they show it.
    public func resumeInterrupted() async throws {
        _ = try await run(["resume", "--all"], nil)
    }

    public func moveTask(issue: String, wave: String?, cwd: String?) async throws {
        _ = try await write(.taskMove(directory: cwd ?? "/", issue: issue, wave: wave))
    }

    /// Your registered Machines. This Machine appears only when a default names it.
    public func machines() async throws -> [Machine] {
        try Self.decode([Machine].self, from: await serve(.machines))
    }

    /// Move a Task to `machine` through `lf task move --machine`, or set your
    /// Machine for a Wave through `lf set machine`.
    public func useMachine(_ machine: String, for target: WorkMachineTarget, cwd: String?) async throws {
        switch target {
        case .task(let issue): _ = try await run(["task", "move", issue, "--machine", machine, "--json"], cwd)
        case .wave(let id): _ = try await run(["--wave", id, "set", "machine", machine], cwd)
        }
    }

    /// `lf task claim` or `lf task unassign`.
    public func assignTask(issue: String, claimed: Bool, cwd: String?) async throws {
        _ = try await write(.taskOwnership(directory: cwd ?? "/", issue: issue, claimed: claimed))
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
        _ = try await write(.taskNotes(directory: cwd, issue: id, notes: text))
    }

    /// One Task's complete patch, or the patch for a selected changed file.
    public func taskDiff(
        issue: String,
        path: String?,
        base: String = "parent",
        draft: String? = nil
    ) async throws -> TaskDiffSnapshot {
        let stdout = try await serve(.taskDiff(issue: issue, path: path, base: base, draft: draft))
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
            .sessionPage(directory: cwd ?? "/", headless: includingHeadless, after: after)))
    }

    /// The repository or Wave's retained conversation; explicit opening only.
    public func ensurePrimarySession(wave: String? = nil, cwd: String) async throws -> SessionRecord {
        try Self.decode(SessionRecord.self, from: await write(
            .sessionResume(directory: cwd, id: nil, task: nil, wave: wave, agent: nil)))
    }

    /// The Task's one ongoing conversation: the one already chosen, else its
    /// only or most recently used one, else a new one in its checkout. The
    /// choice is remembered as the Task's primary. External ownership is returned
    /// as a Move here action without transferring control.
    public func prepareTaskSession(issue: String, cwd: String?) async throws -> SessionRecord {
        try Self.decode(SessionRecord.self, from: await write(
            .sessionResume(directory: cwd ?? "/", id: nil, task: issue, wave: nil, agent: nil)))
    }

    /// Prepare one Session’s terminal command without starting its provider. The
    /// command takes the conversation over from whatever holds it.
    public func prepareSession(
        id: String,
        agent: String? = nil,
        cwd: String? = nil
    ) async throws -> SessionRecord {
        try Self.decode(SessionRecord.self, from: await write(
            .sessionResume(directory: cwd ?? "/", id: id, task: nil, wave: nil, agent: agent)))
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
    public func resetSession(_ session: SessionRecord, agent: String?, cwd: String?) async throws -> SessionRecord {
        try Self.decode(SessionRecord.self, from: await write(
            .sessionNew(directory: cwd ?? "/", id: session.id, agent: agent)))
    }

    public func previewSessionBinding(id: String, task: String) async throws -> SessionBindingPreview {
        try Self.decode(SessionBindingPreview.self, from: await serve(.bindPreview(session: id, task: task)))
    }

    public func bindSession(id: String, taskId: String, cwd: String?) async throws -> SessionRecord {
        let stdout = try await write(.sessionBind(directory: cwd ?? "/", id: id, task: taskId))
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
            .usage(days: days, wave: wave, project: project, task: task)))
    }

    /// The codebase on disk, as a tree of directories weighted by tokens.
    /// Mirrors Rust `CodeNode`. Runs in `repoPath` — `lf tokens` measures the
    /// repo it is invoked in.
    public func codebase(repoPath: String) async throws -> CodeNode {
        try Self.decode(CodeNode.self, from: await serve(.tokens(directory: repoPath, days: nil)))
    }

    /// How big the codebase was on each day it changed. Mirrors Rust
    /// `CodeSnapshot`. Blob counts are cached by sha, so only the first walk of
    /// a window pays to tokenize.
    public func codebaseHistory(repoPath: String, days: Int = 30) async throws -> [CodeSnapshot] {
        try Self.decode([CodeSnapshot].self, from: await serve(.tokens(directory: repoPath, days: days)))
    }

    /// How much Session history providers still keep, and what they remove next.
    /// Mirrors Rust `RetentionSummary`.
    public func retention() async throws -> RetentionSummary {
        try Self.decode(RetentionSummary.self, from: await serve(.retention))
    }

    /// The ledger's self-audit, including continuity and lineage tripwires.
    public func doctor() async throws -> DoctorReport {
        try Self.decode(DoctorReport.self, from: await serve(.doctor))
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

    enum CodingKeys: String, CodingKey {
        case task, graph
        case sessionId = "session_id"
    }
}

/// New Task filed `receipt.task`, then its run failed with `message`.
public struct TaskCreateRunFailed: LocalizedError, Sendable {
    public let receipt: TaskCreateReceipt
    public let message: String
    public var errorDescription: String? { message }
}

/// `lf wave graph show --json`: the graph a scope's new Tasks take up, resolved
/// in Rust through your, the Wave's and the repository's `graph:`, then `code`.
/// The repository scope has no Wave.
public struct WaveGraph: Decodable, Sendable, Hashable {
    public struct Default: Decodable, Sendable, Hashable {
        public let name: String
        /// `personal`, `wave`, `repo`, `global` or `default`
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
            parentWaveId: parentWaveId,
            retiredAt: retiredAt,
            supersededByWaveId: supersededByWaveId,
            retirementReason: retirementReason
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

/// Display projection of the rotation report; provider bodies stay owned by Rust.
public struct ProjectRotationPreview: Decodable, Sendable {
    public let name: String
    public let waves: [WaveRotation]

    public struct WaveRotation: Decodable, Sendable {
        public let wave: String
        public let successor_id: String
        public let predecessor: Project?
        public let successor: Project?
        public let tasks: [Task]
    }

    public struct Project: Decodable, Sendable {
        public let id: String
        public let name: String
    }

    public struct Task: Decodable, Sendable {
        public let task: Issue
        public let disposition: String
        public let reason: String
    }

    public struct Issue: Decodable, Sendable {
        public let id: String
        public let identifier: String
        public let title: String
    }
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
