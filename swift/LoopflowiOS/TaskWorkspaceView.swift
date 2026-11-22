import Loopflow
import SwiftUI

/// One Task on the phone: where it stands on its Graph, then its conversation.
/// The Changes that Desktop shows beside it open over it.
struct TaskWorkspaceView: View {
    let client: RemoteMobileClient
    let feed: TaskFeed
    let task: MobileTask
    let theme: ThemeDefinition
    @Binding var draft: String
    @State private var showsChanges = false
    /// The Task as the Machine last read it.
    @State private var reading = Loaded<Reading>.loading
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        VStack(spacing: 0) {
            // While the Machine is away the sentence would be an old one.
            if case .value(let reading) = reading, let graph = reading.graph, feed.pendingChanges.acceptsChanges {
                WorkflowLine(graph: graph, auto: states.contains(.auto))
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, Spacing.lg)
                    .padding(.vertical, Spacing.sm)
                    .background(theme.palette.background)
                    .overlay(alignment: .bottom) { Rectangle().fill(theme.palette.border).frame(height: 1) }
            }
            content
        }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) { actions.disabled(!feed.pendingChanges.acceptsChanges) }
        }
        .overlay { NoticeView(center: feed.notices) }
        .task(id: scenePhase == .active) { await watchGraph() }
    }

    /// What the Machine read of one Task: its Graph (nil until it takes one up),
    /// the states its row carries, and its checkout once it has one.
    struct Reading: Equatable {
        let graph: TaskGraph?
        let states: [WorkSelectionState]
        let checkout: String?
    }

    /// The feed's row for this Task, with pending changes made. The list's
    /// filter decides whether it has one: a Task just started leaves Backlog,
    /// and one opened from its creation answer may never have been listed.
    private var row: WorkViewPlan.Content? {
        feed.shownRepositories.lazy.flatMap(\.tasks).first { $0.work.id == task.id }
    }

    /// The Task's states from its own reading, which no filter hides; the
    /// list's row stands in until that reading arrives.
    private var states: [WorkSelectionState] {
        if case .value(let reading) = reading { return reading.states }
        return row?.states ?? []
    }

    /// Where on the Machine a write for this Task runs: its repository while
    /// the list shows it, else its own checkout.
    private var directory: String? {
        if let row, let path = feed.directory(of: row) { return path }
        if case .value(let reading) = reading { return reading.checkout }
        return nil
    }

    /// Follow the Task's reading while the screen shows and the app is active.
    /// A lost link resumes the same reading; one that cannot start leaves no
    /// sentence, and the Notice says why the Machine is away.
    private func watchGraph() async {
        guard scenePhase == .active else { return }
        let watch = client.observeTask(issue: task.id)
        defer { watch.cancel() }
        do {
            for try await frame in watch.updates { reading = Self.reading(in: frame, task: task.id) }
        } catch is CancellationError {
            return
        } catch {
            reading = .failed(machineConnectionMessage(error))
        }
    }

    /// One frame of the Task's reading.
    static func reading(in frame: RawJSON, task: String) -> Loaded<Reading> {
        do {
            let scope = WorkScope(repo: nil, headless: false, task: task, activity: nil, taskListWave: nil)
            let frame = try WorkFrame.decode(part: .task, scope: scope, sequence: 0, line: frame.bytes)
            guard case .task(let part?) = frame.content else {
                return .failed(frame.unavailable ?? "Task reading is unavailable")
            }
            return .value(Reading(graph: part.work.graph,
                                  states: part.row.map { $0.selectionStates.sorted { $0.rawValue < $1.rawValue } } ?? [],
                                  checkout: part.row?.reference.workspace?.worktree))
        } catch {
            return .failed(error.localizedDescription)
        }
    }

    /// One entry of "Task actions": its name in the menu and the Notice, and the write it sends.
    enum Action: Equatable {
        /// `lf task run`, under the name the Graph gives it: "Start" or "Run".
        case run(String)
        case autoShip, stopAutoShip

        var label: String {
            switch self {
            case .run(let label): label
            case .autoShip: "Auto-Ship"
            case .stopAutoShip: "Stop Auto-Ship"
            }
        }

        var symbol: String {
            switch self {
            case .run: "play.fill"
            case .autoShip: "paperplane"
            case .stopAutoShip: "pause.circle"
            }
        }

        func write(directory: String, issue: String) -> ServedWrite {
            switch self {
            case .run: .taskRun(directory: directory, issue: issue, graph: nil, from: nil, agent: nil, approve: false)
            case .autoShip: .taskAutoOn(directory: directory, issue: issue)
            case .stopAutoShip: .taskAutoOff(directory: directory, issue: issue)
            }
        }
    }

    /// What "Task actions" offers: the step that moves the Task from where its
    /// Graph stands, which needs a current reading, and its auto setting, which
    /// needs only its states.
    static func actions(reading: Loaded<Reading>, states: [WorkSelectionState], issue: String) -> [Action] {
        var actions: [Action] = []
        if case .value(let reading) = reading {
            if let graph = reading.graph {
                if let run = graph.runAction(issue: issue) { actions.append(.run(run.label)) }
            } else if states.contains(.unstarted) {
                actions.append(.run("Start"))
            }
        }
        if states.contains(.auto) {
            actions.append(.stopAutoShip)
        } else if states.contains(.working) {
            actions.append(.autoShip)
        }
        return actions
    }

    @ViewBuilder private var actions: some View {
        let issue = task.identifier ?? task.id
        let actions = Self.actions(reading: reading, states: states, issue: issue)
        if let directory, !actions.isEmpty {
            Menu("Task actions", systemImage: "ellipsis.circle") {
                ForEach(actions, id: \.label) { action in
                    Button(action.label, systemImage: action.symbol) {
                        feed.request(action.label, issue: issue, write: action.write(directory: directory, issue: issue))
                    }
                }
            }
        }
    }

    @ViewBuilder private var content: some View {
        if let session = task.primarySession {
            SessionView(client: client, task: task, session: session, theme: theme, draft: $draft) { showsChanges = true }
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button("Changes", systemImage: "plusminus") { showsChanges = true }
                    }
                }
                .sheet(isPresented: $showsChanges) {
                    NavigationStack {
                        TaskChangesView(client: client, task: task, theme: theme)
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
                    TaskChangesView(client: client, task: task, theme: theme)
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
    let theme: ThemeDefinition
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
                        TaskDiffView(client: client, task: task, path: file.path, theme: theme)
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
    let theme: ThemeDefinition
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
    let theme: ThemeDefinition
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
                NavigationLink("Whole file") { TaskFileView(client: client, task: task, path: path, theme: theme) }
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
