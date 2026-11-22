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
    /// The Task's Graph as the Machine last read it; a Task that has taken up none reads nil.
    @State private var graph = Loaded<TaskGraph?>.loading
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        VStack(spacing: 0) {
            // While the Machine is away the sentence would be an old one.
            if case .value(let graph?) = graph, feed.pendingChanges.acceptsChanges {
                WorkflowLine(graph: graph, auto: row?.states.contains(.auto) == true)
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

    /// The feed's row for this Task, with pending changes made; a Task opened
    /// from its creation answer has none until the feed lists it.
    private var row: WorkViewPlan.Content? {
        feed.shownRepositories.lazy.flatMap(\.tasks).first { $0.work.id == task.id }
    }

    /// Follow the Task's reading while the screen shows and the app is active.
    /// A lost link resumes the same reading; one that cannot start leaves no
    /// sentence, and the Notice says why the Machine is away.
    private func watchGraph() async {
        guard scenePhase == .active else { return }
        let watch = client.observeTask(issue: task.id)
        defer { watch.cancel() }
        do {
            for try await frame in watch.updates { graph = Self.graph(in: frame, task: task.id) }
        } catch is CancellationError {
            return
        } catch {
            graph = .failed(machineConnectionMessage(error))
        }
    }

    /// The Graph in one frame of the Task's reading.
    static func graph(in frame: RawJSON, task: String) -> Loaded<TaskGraph?> {
        do {
            let scope = WorkScope(repo: nil, headless: false, task: task, activity: nil, taskListWave: nil)
            let frame = try WorkFrame.decode(part: .task, scope: scope, sequence: 0, line: frame.bytes)
            guard case .task(let reading?) = frame.content else {
                return .failed(frame.unavailable ?? "Task reading is unavailable")
            }
            return .value(reading.work.graph)
        } catch {
            return .failed(error.localizedDescription)
        }
    }

    /// One entry of "Task actions".
    enum Action: Equatable {
        /// `lf task run`, under the name the Graph gives it: "Start" or "Run".
        case run(String)
        case autoShip, stopAutoShip
    }

    /// What "Task actions" offers: the step that moves the Task from where its
    /// Graph stands, which needs a current reading, and its auto setting, which
    /// needs only the row.
    static func actions(graph: Loaded<TaskGraph?>, states: [WorkSelectionState], issue: String) -> [Action] {
        var actions: [Action] = []
        switch graph {
        case .value(let graph?):
            if let run = graph.runAction(issue: issue) { actions.append(.run(run.label)) }
        case .value(nil):
            if states.contains(.unstarted) { actions.append(.run("Start")) }
        case .loading, .failed:
            break
        }
        if states.contains(.auto) {
            actions.append(.stopAutoShip)
        } else if states.contains(.working) {
            actions.append(.autoShip)
        }
        return actions
    }

    @ViewBuilder private var actions: some View {
        if let row {
            let actions = Self.actions(graph: graph, states: row.states, issue: row.identifier ?? row.work.id)
            if !actions.isEmpty {
                Menu("Task actions", systemImage: "ellipsis.circle") {
                    ForEach(actions.indices, id: \.self) { index in
                        switch actions[index] {
                        case .run(let label):
                            Button(label, systemImage: "play.fill") {
                                feed.request(label, row) {
                                    .taskRun(directory: $0, issue: $1, graph: nil, from: nil, agent: nil, approve: false)
                                }
                            }
                        case .autoShip:
                            Button("Auto-Ship", systemImage: "paperplane") {
                                feed.request("Auto-Ship", row) { .taskAutoOn(directory: $0, issue: $1) }
                            }
                        case .stopAutoShip:
                            Button("Stop Auto-Ship", systemImage: "pause.circle") {
                                feed.request("Stop Auto-Ship", row) { .taskAutoOff(directory: $0, issue: $1) }
                            }
                        }
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
            TaskChangesView(client: client, task: task, theme: theme)
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
                    ScrollView([.vertical, .horizontal]) {
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
                ScrollView([.vertical, .horizontal]) {
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
