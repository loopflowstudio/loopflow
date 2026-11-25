import Loopflow
import SwiftUI

/// One Task on the phone: where it stands on its Graph, then its conversation.
/// The Changes that Desktop shows beside it open over it.
struct TaskWorkspaceView: View {
    let client: RemoteMobileClient
    let feed: TaskFeed
    let task: MobileTask
    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss
    @Binding var draft: String
    @State private var showsChanges = false
    @State private var confirmingAbandon = false
    @State private var selectedSession: String?
    @State private var sessionDrafts: [String: String] = [:]
    @State private var outline: WorkflowOutline?
    /// The Task as the Machine last read it.
    @State private var reading = Loaded<Reading>.loading
    /// The conversation that began while the screen was showing.
    @State private var begun: String?
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        VStack(spacing: 0) {
            if case .value(let reading) = reading, let row = reading.row, feed.pendingChanges.acceptsChanges {
                let actions = reading.actions(for: row, outline: outline)
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    if let graph = reading.work.graph {
                        WorkflowLine(graph: graph, auto: actions.auto == .on, style: .steps)
                            .accessibilityHint(graph.sentence)
                    }
                    HStack {
                        Spacer(minLength: 0)
                        controls(row, actions)
                        metadata(row, sessions: reading.work.sessions)
                    }
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
            content
        }
        .overlay { NoticeView(center: feed.notices) }
        .task(id: scenePhase == .active) { await watchTask() }
    }

    /// What the Machine read of one Task: its work (the Graph it took up and
    /// its Sessions), its row, and its auto mark.
    struct Reading: Equatable {
        let work: TaskOverview
        let row: RoadmapTask?
        let execution: TaskExecution?

        func actions(for row: RoadmapTask, outline: WorkflowOutline? = nil) -> TaskActions {
            TaskActions(work: work, row: row, execution: execution, outline: outline)
        }

        /// Nil until the Task starts.
        var checkout: String? { row?.reference.workspace?.worktree }
    }

    /// Follow the Task's reading while the screen shows and the app is active.
    /// A lost link resumes the same reading; one that cannot start leaves no
    /// sentence, and the Notice says why the Machine is away.
    private func watchTask() async {
        guard scenePhase == .active else { return }
        let watch = client.observeTask(issue: task.id)
        defer { watch.cancel() }
        do {
            for try await frame in watch.updates {
                reading = Self.reading(in: frame, task: task.id)
                await openConversation()
            }
        } catch is CancellationError {
            return
        } catch {
            reading = .failed(machineConnectionMessage(error))
        }
    }

    /// A Task started from this screen gets its conversation here. A row that
    /// names none yet is asked again on the next frame.
    private func openConversation() async {
        guard task.primarySession == nil, begun == nil else { return }
        do {
            begun = try await Self.conversation(after: reading) { [client, id = task.id] repository in
                try await client.task(id: id, directory: repository)?.primarySession
            }
        } catch {
            feed.notices.post(Notice("The conversation couldn’t be opened: \(machineConnectionMessage(error))",
                                     key: "task-conversation-\(task.id)"))
        }
    }

    /// The conversation to open after `reading`: once it lists a Session, the
    /// primary one the Task's row names. `primary` reads that row in a repository.
    static func conversation(after reading: Loaded<Reading>,
                             primary: (String) async throws -> String?) async throws -> String? {
        guard case .value(let reading) = reading, !reading.work.sessions.isEmpty,
              let repository = reading.row?.task.repo else { return nil }
        return try await primary(repository)
    }

    /// One frame of the Task's reading.
    static func reading(in frame: RawJSON, task: String) -> Loaded<Reading> {
        do {
            let scope = WorkScope(repo: nil, headless: false, task: task, activity: nil, taskListWave: nil)
            let frame = try WorkFrame.decode(part: .task, scope: scope, sequence: 0, line: frame.bytes)
            guard case .task(let part?) = frame.content else {
                return .failed(frame.unavailable ?? "Task reading is unavailable")
            }
            return .value(Reading(work: part.work, row: part.row, execution: part.execution))
        } catch {
            return .failed(error.localizedDescription)
        }
    }

    private func controls(_ row: RoadmapTask, _ actions: TaskActions) -> some View {
        let (issue, directory) = (row.task.identifier, row.task.repo)
        func send(_ name: String, _ write: ServedWrite) { feed.request(name, issue: issue, write: write) }
        return TaskWorkflowControls(task: row, actions: actions, pending: feed.pendingChanges.pauseRequest(for: issue),
            run: { step in
                send("Run", .taskRun(directory: directory, issue: issue, graph: nil, from: step, agent: nil, approve: false))
            }, pause: { action in
                feed.pendingChanges.setPause(action, issue: issue, directory: directory, notices: feed.notices, show: nil) {
                    await feed.refresh()
                }
            }, interrupt: { send("Interrupt step", .taskInterrupt(directory: directory, issue: issue)) },
            toggleAuto: {
                guard let auto = actions.auto else { return }
                send(auto.offer, auto == .on ? .taskAutoOff(directory: directory, issue: issue) : .taskAutoOn(directory: directory, issue: issue))
            }, complete: { send("Complete", .taskComplete(directory: directory, issue: issue)) }) {
                if row.isCompleted {
                    Button("Reopen Task") { send("Reopen", .taskReopen(directory: directory, issue: issue)) }
                } else {
                    if row.isUnassigned {
                        Button("Claim Task") { send("Claim", .taskClaim(directory: directory, issue: issue)) }
                    } else {
                        Button("Unassign Task") { send("Unassign", .taskUnassign(directory: directory, issue: issue)) }
                    }
                    Button("Abandon…", role: .destructive) { confirmingAbandon = true }
                }
            }
            .confirmationDialog("Abandon \(issue)?", isPresented: $confirmingAbandon) {
                Button("Abandon Task", role: .destructive) { send("Abandon", .taskAbandon(directory: directory, issue: issue)) }
            } message: {
                Text("Cancels the Task, closes its PRs and deletes its branches.")
            }
    }

    private func metadata(_ row: RoadmapTask, sessions: [TaskSession]) -> some View {
        Menu("Task actions", systemImage: "ellipsis") {
            Menu("Move to Wave") {
                Button("No Wave") {
                    feed.request("Move", issue: row.task.identifier, write: .taskMove(directory: row.task.repo, issue: row.task.identifier, wave: nil, machine: nil))
                }.disabled(row.task.waveId == nil)
                ForEach(feed.waves.filter { $0.repo == row.task.repo }) { wave in
                    Button(wave.displayName) {
                        feed.request("Move", issue: row.task.identifier, write: .taskMove(directory: row.task.repo, issue: row.task.identifier, wave: wave.id, machine: nil))
                    }.disabled(wave.id == row.task.waveId)
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
                Button("Open \(session.title)") { selectedSession = session.id }
            }
        }
        .labelStyle(.iconOnly)
        .frame(width: 44, height: 44)
    }

    @ViewBuilder private var content: some View {
        if let session = selectedSession ?? task.primarySession ?? begun {
            let sessionDraft = session == (task.primarySession ?? begun) ? $draft : Binding(
                get: { sessionDrafts[session] ?? "" }, set: { sessionDrafts[session] = $0 })
            SessionView(client: client, task: task, session: session, draft: sessionDraft) { showsChanges = true }
                .id(session)
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button("Changes", systemImage: "plusminus") { showsChanges = true }
                    }
                }
                .sheet(isPresented: $showsChanges) {
                    NavigationStack {
                        TaskChangesView(client: client, task: task)
                            .navigationTitle("Changes")
                            .navigationBarTitleDisplayMode(.inline)
                            .toolbar {
                                ToolbarItem(placement: .confirmationAction) { Button("Done") { showsChanges = false } }
                            }
                    }
                    .tint(theme.palette.accentInk)
                    .presentationDetents([.medium, .large])
                }
        } else {
            Group {
                switch reading {
                case .loading:
                    ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
                case .value(let reading) where reading.checkout == nil:
                    // Nothing is checked out until the Task starts, so there is nothing to ask the Machine for.
                    StateView("No changes yet")
                case .value, .failed:
                    TaskChangesView(client: client, task: task)
                }
            }
            .background(theme.palette.background)
            .navigationTitle(task.title)
            .navigationBarTitleDisplayMode(.inline)
        }
    }
}

/// A read that is loading, failed or here.
enum Loaded<Value> {
    case loading, failed(String), value(Value)
}

/// Files the Task changed from its base, each opening its diff.
struct TaskChangesView: View {
    let client: RemoteMobileClient
    let task: MobileTask
    @Environment(\.theme) private var theme
    @State private var changes = Loaded<TaskChangesSnapshot>.loading

    var body: some View {
        Group {
            switch changes {
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
        .task { await load() }
    }

    private func load() async {
        do { changes = .value(try await client.query.taskChanges(issue: task.id)) } catch {
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
