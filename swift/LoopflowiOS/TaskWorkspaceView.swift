import Loopflow
import SwiftUI

/// One Task on the phone: where it stands on its Graph and which Machine runs
/// it, then its Session, Files, Changes and Shell. All four come from the
/// paired Machine, which reaches the Task wherever it runs.
struct TaskWorkspaceView: View {
    let client: RemoteMobileClient
    let feed: TaskFeed
    let task: MobileTask
    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss
    @Binding var draft: String
    /// The pane chosen here; until then the Session, or Changes before there is one.
    @State private var chosen: Pane?
    @State private var confirmingAbandon = false
    @State private var selectedSession: String?
    @State private var sessionDrafts: [String: String] = [:]
    @State private var outline: WorkflowOutline?
    @State private var shellDraft = ""
    /// The Task as the Machine last read it.
    @State private var reading = Loaded<Reading>.loading
    /// The conversation the Machine running the Task names, when the list's row named none.
    @State private var begun: String?
    @State private var files: TaskFilesTree
    @Environment(\.scenePhase) private var scenePhase

    init(client: RemoteMobileClient, feed: TaskFeed, task: MobileTask, draft: Binding<String>) {
        self.client = client
        self.feed = feed
        self.task = task
        _draft = draft
        _files = State(initialValue: TaskFilesTree(client: client, issue: task.id))
    }

    enum Pane: String, CaseIterable, Identifiable {
        case session = "Session", files = "Files", changes = "Changes", shell = "Shell"
        var id: Self { self }
    }

    private var primarySession: String? { task.primarySession ?? begun }
    private var session: String? { selectedSession ?? primarySession }
    private var pane: Pane { chosen ?? (session == nil ? .changes : .session) }

    var body: some View {
        VStack(spacing: 0) {
            header
            Picker("View", selection: Binding(get: { pane }, set: { chosen = $0 })) {
                ForEach(Pane.allCases) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.segmented)
            .padding(.horizontal, Spacing.lg)
            .padding(.vertical, Spacing.sm)
            content
        }
        .background(theme.palette.background)
        .navigationTitle(task.title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if let (reading, row) = patchedReading {
                ToolbarItem(placement: .topBarTrailing) {
                    metadata(row, sessions: reading.work.sessions)
                }
            }
        }
        .overlay { NoticeView(center: feed.notices) }
        .task(id: scenePhase == .active) {
            guard scenePhase == .active else { return }
            async let files: Void = files.follow()
            await watchTask()
            await files
        }
    }

    private var patchedReading: (reading: Reading, row: RoadmapTask)? {
        guard case .value(let reading) = reading, let rawRow = reading.row else { return nil }
        let row = feed.pendingChanges.apply(to: [rawRow], readAt: reading.readAt).first ?? rawRow
        return (reading, row)
    }

    @ViewBuilder private var header: some View {
        if let (reading, row) = patchedReading, feed.pendingChanges.acceptsChanges {
            let actions = reading.actions(for: row, outline: outline,
                                          autoPending: feed.pendingChanges.autoRequest(for: row.task.identifier))
            VStack(alignment: .leading, spacing: Spacing.xs) {
                if let machine = reading.machine(pairedWith: client.savedMachine.machineID) {
                    Label("On \(machine)", systemImage: "desktopcomputer")
                        .font(Typography.meta)
                        .foregroundStyle(theme.palette.textSecondary)
                        .accessibilityIdentifier("task-machine")
                }
                controls(row, actions)
                if let reason = actions.primaryRefusal ?? (row.lifecycle == .paused ? nil : actions.autoRefusal) {
                    Text(reason).font(Typography.meta).foregroundStyle(theme.palette.warning)
                        .accessibilityIdentifier("task-actions-refusal")
                }
            }
            .font(Typography.text)
            .padding(.horizontal, Spacing.sm)
            .padding(.vertical, Spacing.xs)
            .overlay(alignment: .bottom) { Rectangle().fill(theme.palette.border).frame(height: 1) }
            .task(id: reading.work.graph == nil ? row.graphName : nil) {
                outline = nil
                if reading.work.graph == nil, let name = row.graphName {
                    let catalog = try? await client.query.graphCatalog(cwd: row.task.repo)
                    if !Task.isCancelled { outline = catalog?.first { $0.name == name }?.workflow }
                }
            }
        }
    }

    /// What a Machine read of one Task: its work (the Graph it took up and
    /// its Sessions), its row, and its auto mark.
    struct Reading: Equatable {
        let work: TaskOverview
        let row: RoadmapTask?
        let execution: TaskExecution?
        let readAt: Revisions?

        func actions(for row: RoadmapTask, outline: WorkflowOutline? = nil, autoPending: Bool? = nil) -> TaskActions {
            TaskActions(work: work, row: row, execution: execution, outline: outline, autoPending: autoPending)
        }

        /// The Machine whose checkout this is, by name, when it is not `paired`.
        func machine(pairedWith paired: String) -> String? {
            guard let workspace = row?.reference.workspace, let id = workspace.machineId,
                  StructuredIdentity.prefixed(id, "home_") != StructuredIdentity.prefixed(paired, "home_") else { return nil }
            return workspace.machineLabel ?? id
        }
    }

    /// Follow the Task's reading while the screen shows and the app is active.
    /// A lost link resumes the same reading; one that cannot start leaves no
    /// sentence, and the Notice says why the Machine is away.
    private func watchTask() async {
        guard scenePhase == .active else { return }
        let watch = client.observeTask(issue: task.id)
        let key = "task-\(task.id)"
        defer {
            watch.cancel()
            feed.pendingChanges.ended(key)
        }
        do {
            for try await frame in watch.updates {
                let next = Self.reading(in: frame, task: task.id)
                reading = next
                if case .value(let value) = next {
                    feed.pendingChanges.read(key, Loopflow.Reading.available(value.work, readAt: value.readAt))
                }
                await openConversation()
            }
        } catch is CancellationError {
            return
        } catch {
            reading = .failed(machineConnectionMessage(error))
        }
    }

    /// A Task whose row named no conversation gets one here once the Machine
    /// running it names its primary. A reading that lists none yet asks nothing.
    private func openConversation() async {
        guard task.primarySession == nil, begun == nil else { return }
        do {
            begun = try await Self.conversation(after: reading) { [client, id = task.id] in
                try await client.primarySession(task: id)
            }
        } catch {
            feed.notices.post(Notice("The conversation couldn’t be opened: \(machineConnectionMessage(error))",
                                     key: "task-conversation-\(task.id)"))
        }
    }

    /// The conversation to open after `reading`: once it lists a Session, the
    /// primary one the Machine running the Task names.
    static func conversation(after reading: Loaded<Reading>,
                             primary: () async throws -> String?) async throws -> String? {
        guard case .value(let reading) = reading, !reading.work.sessions.isEmpty else { return nil }
        return try await primary()
    }

    /// One frame of the Task's reading.
    static func reading(in frame: RawJSON, task: String) -> Loaded<Reading> {
        do {
            let scope = WorkScope(repo: nil, headless: false, task: task, activity: nil, taskListWave: nil)
            let frame = try WorkFrame.decode(part: .task, scope: scope, sequence: 0, line: frame.bytes)
            guard case .task(let part?) = frame.content else {
                return .failed(frame.unavailable ?? "Task reading is unavailable")
            }
            return .value(Reading(work: part.work, row: part.row, execution: part.execution, readAt: frame.readAt))
        } catch {
            return .failed(error.localizedDescription)
        }
    }

    private func controls(_ row: RoadmapTask, _ actions: TaskActions) -> some View {
        let (issue, directory) = (row.task.identifier, row.task.repo)
        func change(_ command: PendingCommand, _ verb: String, _ write: ServedWrite) {
            feed.change(command, issue: issue, verb: verb, write: write)
        }
        return TaskWorkflowControls(task: row, actions: actions, pending: feed.pendingChanges.controlRequest(for: issue),
            run: { step in
                change(.run(task: issue), "started",
                       .taskRun(directory: directory, issue: issue, graph: nil, from: step, agent: nil, approve: false, atArrival: step == nil ? actions.breakpointArrival : nil, decision: nil, reason: nil))
            }, pause: { action in
                feed.pendingChanges.setPause(action, issue: issue, directory: directory, notices: feed.notices, show: nil) {
                    await feed.refresh()
                }
            }, interrupt: { change(.interrupt(task: issue), "interrupted", .taskInterrupt(directory: directory, issue: issue)) },
            toggleAuto: {
                guard let auto = actions.auto else { return }
                if auto == .on {
                    change(.auto(task: issue, on: false), "taken off auto", .taskAutoOff(directory: directory, issue: issue))
                } else {
                    change(.auto(task: issue, on: true), "put on auto", .taskAutoOn(directory: directory, issue: issue))
                }
            }, complete: { feed.complete(issue: issue, directory: directory) }) {
                if row.isCompleted {
                    Button("Reopen Task") { change(.reopen(task: issue), "reopened", .taskReopen(directory: directory, issue: issue)) }
                } else {
                    if row.isUnassigned {
                        Button("Claim Task") { change(.claim(task: issue, owner: nil), "claimed", .taskClaim(directory: directory, issue: issue)) }
                    } else {
                        Button("Unassign Task") { change(.unassign(task: issue), "unassigned", .taskUnassign(directory: directory, issue: issue)) }
                    }
                    Button("Abandon…", role: .destructive) { confirmingAbandon = true }
                }
            }
            .confirmationDialog("Abandon \(issue)?", isPresented: $confirmingAbandon) {
                Button("Abandon Task", role: .destructive) { change(.abandon(task: issue), "abandoned", .taskAbandon(directory: directory, issue: issue)) }
            } message: {
                Text("Cancels the Task, closes its PRs and deletes its branches.")
            }
    }

    private func metadata(_ row: RoadmapTask, sessions: [TaskSession]) -> some View {
        Menu {
            Menu("Move to Wave") {
                Button("No Wave") { move(row, to: nil) }.disabled(row.task.waveId == nil)
                ForEach(feed.waves.filter { $0.repo == row.task.repo }) { wave in
                    Button(wave.displayName) { move(row, to: wave.id) }.disabled(wave.id == row.task.waveId)
                }
            }
            if let wave = row.task.waveId {
                Button("Open owning Wave") { feed.wave = wave; dismiss() }
            }
            Button("Copy Task ID") { UIPasteboard.general.string = row.task.identifier }
            if let link = row.reference.issueUrl {
                Button("Copy Task link") { UIPasteboard.general.url = link }
            }
            if let branch = row.reference.workspace?.branch {
                Button("Copy branch") { UIPasteboard.general.string = branch }
            }
            if let pr = row.pr?.publication?.github { Link("Open PR #\(pr.number)", destination: pr.url) }
            ForEach(sessions.filter(\.interactive)) { session in
                Button("Open \(session.title)") {
                    selectedSession = session.id == primarySession ? nil : session.id
                    chosen = .session
                }
            }
        } label: {
            Image(systemName: "ellipsis")
        }
        .accessibilityLabel("Task actions")
        .accessibilityIdentifier("task-metadata-menu")
    }

    private func move(_ row: RoadmapTask, to wave: String?) {
        let issue = row.task.identifier
        feed.change(.move(task: issue, wave: wave), issue: issue, verb: "moved",
                    write: .taskMove(directory: row.task.repo, issue: issue, wave: wave, machine: nil))
    }

    /// The Machine of the latest reading: Changes reads again when it moves.
    private var machine: String? {
        guard case .value(let reading) = reading else { return nil }
        return reading.row?.reference.workspace?.machineId
    }

    @ViewBuilder private var content: some View {
        switch pane {
        case .session:
            if let session {
                let sessionDraft = session == primarySession ? $draft : Binding(
                    get: { sessionDrafts[session] ?? "" }, set: { sessionDrafts[session] = $0 })
                TerminalView(client: client, task: task, target: .session(session), draft: sessionDraft) { chosen = .changes }
                    .id(session)
            } else {
                StateView("No conversation yet", sentence: "Run the Task to start one.")
            }
        case .files:
            TaskFilesTreeView(client: client, task: task, tree: files, path: "")
        case .changes:
            TaskChangesView(client: client, task: task, machine: machine)
        case .shell:
            TerminalView(client: client, task: task, target: .shell, draft: $shellDraft) { chosen = .changes }
        }
    }
}

/// A read that is loading, failed or here.
enum Loaded<Value> {
    case loading, failed(String), value(Value)
}

/// Files the Task changed from its base, each opening its diff. It reads
/// again when the Task's checkout moves to another Machine.
struct TaskChangesView: View {
    let client: RemoteMobileClient
    let task: MobileTask
    let machine: String?
    @Environment(\.theme) private var theme
    @State private var changes = Loaded<TaskChangesSnapshot>.loading
    @State private var notStarted = false

    var body: some View {
        Group {
            switch changes {
            case _ where notStarted:
                StateView("Not started", sentence: "The Task has no checkout yet.") {
                    Button("Try again") { Task { await load() } }.buttonStyle(LoopflowButtonStyle(.accent))
                }
            case .loading: ProgressView()
            case .failed(let message):
                StateView("Couldn’t load", sentence: message) {
                    Button("Try again") { Task { await load() } }.buttonStyle(LoopflowButtonStyle(.accent))
                }
            case .value(let changes) where changes.files.isEmpty:
                StateView("No changes yet")
            case .value(let changes):
                List(changes.files) { file in
                    NavigationLink {
                        TaskDiffView(client: client, task: task, path: file.path)
                    } label: {
                        VStack(alignment: .leading, spacing: Spacing.xs) {
                            Text((file.path as NSString).lastPathComponent)
                                .font(Typography.textStrong)
                                .foregroundStyle(theme.palette.text)
                            Text(detail(file))
                                .font(Typography.meta)
                                .foregroundStyle(theme.palette.textSecondary)
                                .lineLimit(2)
                        }
                    }
                    .listRowBackground(theme.palette.background)
                }
                .listStyle(.plain)
                .refreshable { await load() }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme.palette.background)
        .task(id: machine) { await load() }
    }

    private func load() async {
        do {
            changes = .value(try await client.query.taskChanges(issue: task.id))
            notStarted = false
        } catch let refusal as ServerError where refusal.notStarted {
            notStarted = true
        } catch {
            notStarted = false
            changes = .failed(machineConnectionMessage(error))
        }
    }

    private func detail(_ file: TaskChangedFile) -> String {
        let folder = (file.path as NSString).deletingLastPathComponent
        let state = file.untracked ? "new" : file.unstaged || file.staged ? "uncommitted" : "committed"
        return folder.isEmpty ? state : "\(folder) · \(state)"
    }
}

/// Code that scrolls both ways and starts at the top left, as the states beside it do.
/// A two-axis scroll view centres content shorter or narrower than itself.
private struct CodeScroll<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        GeometryReader { proxy in
            ScrollView([.vertical, .horizontal]) {
                content.frame(minWidth: proxy.size.width, minHeight: proxy.size.height, alignment: .topLeading)
            }
        }
    }
}

/// One file as it stands in the Task's checkout.
struct TaskFileView: View {
    let client: RemoteMobileClient
    let task: MobileTask
    let path: String
    @Environment(\.theme) private var theme
    @State private var file = Loaded<TaskFileSnapshot>.loading

    var body: some View {
        Group {
            switch file {
            case .loading: ProgressView()
            case .failed(let message):
                StateView("Couldn’t load", sentence: message) {
                    Button("Try again") { Task { await load() } }.buttonStyle(LoopflowButtonStyle(.accent))
                }
            case .value(let file):
                if let content = file.content {
                    CodeScroll {
                        Text(content)
                            .font(Typography.mono)
                            .foregroundStyle(theme.palette.text)
                            .textSelection(.enabled)
                            .padding(Spacing.md)
                    }
                } else {
                    StateView(file.state.title)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme.palette.background)
        .navigationTitle((path as NSString).lastPathComponent)
        .navigationBarTitleDisplayMode(.inline)
        .task { await load() }
    }

    private func load() async {
        do { file = .value(try await client.query.taskFile(issue: task.id, path: path)) } catch {
            file = .failed(machineConnectionMessage(error))
        }
    }
}

struct TaskDiffView: View {
    let client: RemoteMobileClient
    let task: MobileTask
    let path: String
    @Environment(\.theme) private var theme
    @State private var diff = Loaded<TaskDiffSnapshot>.loading

    var body: some View {
        Group {
            switch diff {
            case .loading: ProgressView()
            case .failed(let message):
                StateView("Couldn’t load", sentence: message) {
                    Button("Try again") { Task { await load() } }.buttonStyle(LoopflowButtonStyle(.accent))
                }
            case .value(let diff) where diff.binary:
                StateView("Binary file")
            case .value(let diff):
                CodeScroll {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        let lines = Self.lines(diff.patch)
                        ForEach(lines.indices, id: \.self) { index in
                            Text(lines[index].text.isEmpty ? " " : String(lines[index].text))
                                .font(Typography.mono)
                                .foregroundStyle(theme.palette.text)
                                .padding(.horizontal, Spacing.md)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(tint(lines[index].kind))
                        }
                        if diff.truncated {
                            Text("The rest of this diff is too long to show.")
                                .font(Typography.meta)
                                .foregroundStyle(theme.palette.textSecondary)
                                .padding(Spacing.md)
                        }
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme.palette.background)
        .navigationTitle((path as NSString).lastPathComponent)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                NavigationLink("Whole file") { TaskFileView(client: client, task: task, path: path) }
            }
        }
        .task { await load() }
    }

    private func load() async {
        do { diff = .value(try await client.query.taskDiff(issue: task.id, path: path)) } catch {
            diff = .failed(machineConnectionMessage(error))
        }
    }

    static func lines(_ patch: String) -> [(text: Substring, kind: PatchLineKind)] {
        var inHunk = false
        return patch.split(separator: "\n", omittingEmptySubsequences: false).map {
            ($0, PatchLineKind.next($0, inHunk: &inHunk))
        }
    }

    private func tint(_ kind: PatchLineKind) -> Color {
        switch kind {
        case .added: theme.palette.successTone.fill
        case .removed: theme.palette.failureTone.fill
        case .hunk: theme.palette.surface
        case .header, .context: .clear
        }
    }
}
