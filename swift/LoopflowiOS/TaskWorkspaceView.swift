import Loopflow
import SwiftUI

/// One Task on the phone: where it stands on its Graph, then its conversation.
/// The Changes that Desktop shows beside it open over it.
struct TaskWorkspaceView: View {
    let client: RemoteMobileClient
    let feed: TaskFeed
    let task: MobileTask
    @Environment(\.theme) private var theme
    @Binding var draft: String
    @State private var showsChanges = false
    /// The Task as the Machine last read it.
    @State private var reading = Loaded<Reading>.loading
    /// The conversation that began while the screen was showing.
    @State private var begun: String?
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        VStack(spacing: 0) {
            // While the Machine is away the sentence would be an old one.
            if case .value(let reading) = reading, feed.pendingChanges.acceptsChanges {
                let refusal = reading.actions?.refusal
                if reading.work.graph != nil || refusal != nil {
                    VStack(alignment: .leading, spacing: Spacing.xs) {
                        if let graph = reading.work.graph {
                            WorkflowLine(graph: graph, auto: reading.actions?.auto == .on)
                        }
                        // A menu item holds two lines; Rust's reason says what to restore, so it is set in full.
                        if let refusal {
                            Text(refusal)
                                .font(Typography.meta)
                                .foregroundStyle(theme.palette.warning)
                                .accessibilityIdentifier("task-actions-refusal")
                        }
                    }
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, Spacing.lg)
                    .padding(.vertical, Spacing.sm)
                    .background(theme.palette.background)
                    .overlay(alignment: .bottom) { Rectangle().fill(theme.palette.border).frame(height: 1) }
                }
            }
            content
        }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) { actions.disabled(!feed.pendingChanges.acceptsChanges) }
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

        var actions: TaskActions? { row.map { TaskActions(work: work, row: $0, execution: execution) } }
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

    /// "Task actions": what `TaskActions` offers, each sending its write from
    /// the Task's repository. A refused action is disabled; the reason is under the sentence.
    @ViewBuilder private var actions: some View {
        if case .value(let reading) = reading, let row = reading.row, let actions = reading.actions,
           actions.run != nil || actions.pause != nil || actions.auto != nil {
            let (issue, directory) = (row.task.identifier, row.task.repo)
            let requesting = feed.pendingChanges.pauseRequest(for: issue) != nil
            Menu("Task actions", systemImage: "ellipsis.circle") {
                if let run = actions.run {
                    item(run.label, symbol: "play.fill", disabled: requesting || actions.runRefusal != nil, issue: issue,
                         write: .taskRun(directory: directory, issue: issue, graph: nil, from: nil, agent: nil, approve: false))
                }
                if let pause = actions.pause {
                    Button(pause.rawValue, systemImage: pause == .pause ? "pause" : "play") {
                        feed.pendingChanges.setPause(pause, row: row, directory: directory, notices: feed.notices, show: nil) {
                            await feed.refresh()
                        }
                    }
                    .disabled(requesting || actions.pauseRefusal != nil)
                }
                if let auto = actions.auto {
                    item(auto.offer, symbol: auto == .on ? "pause.circle" : "paperplane", disabled: (requesting && auto == .off) || actions.autoRefusal != nil, issue: issue,
                         write: auto == .on ? .taskAutoOff(directory: directory, issue: issue)
                             : .taskAutoOn(directory: directory, issue: issue))
                }
            }
        }
    }

    private func item(_ name: String, symbol: String, disabled: Bool, issue: String, write: ServedWrite) -> some View {
        Button {
            feed.request(name, issue: issue, write: write)
        } label: {
            Label(name, systemImage: symbol)
        }
        .disabled(disabled)
    }

    private var paused: Bool {
        if case .value(let reading) = reading, let row = reading.row { return row.lifecycle == .paused }
        return task.paused
    }

    @ViewBuilder private var content: some View {
        if let session = task.primarySession ?? begun {
            Group {
                if paused {
                    PausedSessionHistory(query: client.query, session: session).id(session)
                } else {
                    SessionView(client: client, task: task, session: session, draft: $draft) { showsChanges = true }
                }
            }
                .navigationTitle(task.title)
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

/// The primary conversation's saved history while its provider is closed.
private struct PausedSessionHistory: View {
    let query: RegistryQuery
    let session: String
    @State private var history = Loaded<String>.loading

    var body: some View {
        Group {
            switch history {
            case .loading: ProgressView()
            case .value(let text):
                ScrollView {
                    Text(verbatim: text).textSelection(.enabled).font(Typography.text)
                        .frame(maxWidth: .infinity, alignment: .leading).padding()
                }
            case .failed(let reason): StateView("History unavailable", sentence: reason)
            }
        }
        .task {
            do { history = .value(try await query.sessionHistoryText(id: session)) }
            catch { history = .failed(machineConnectionMessage(error)) }
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
