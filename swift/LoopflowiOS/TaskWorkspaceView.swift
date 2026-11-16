import Loopflow
import SwiftUI

/// One Task on the phone. Its conversation fills the screen; the Changes that
/// Desktop shows beside it open over it.
struct TaskWorkspaceView: View {
    let client: RemoteMobileClient
    let task: WorkViewPlan.Content
    let theme: ThemeDefinition
    @Binding var draft: String
    var complete: (() -> Void)? = nil
    var abandon: (() -> Void)? = nil
    var move: ((String?) -> Void)? = nil
    var edit: ((WorkInteraction.TaskEdit) -> Void)? = nil
    var facts: WorkMembershipPlan.Facts? = nil
    var waves: [Wave] = []
    var completionDirectory: String? = nil

    private var completionPending: Bool {
        if facts?.disposition.value.completionRequest != nil { return true }
        guard let completionDirectory else { return false }
        return client.interaction.isCompleting(issue: task.identifier ?? task.work.id, directory: completionDirectory)
    }
    @State private var showsChanges = false
    @State private var confirmingAbandon = false

    var body: some View {
        Group {
            if task.primarySession == nil {
                TaskChangesView(client: client, task: task, theme: theme)
                    .navigationTitle(task.title)
                    .navigationBarTitleDisplayMode(.inline)
            } else {
                SessionView(client: client, task: task, theme: theme, draft: $draft)
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
            }
        }
        .toolbar {
            if abandon != nil {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        if let edit {
                            TaskOwnershipActions(completed: (facts?.states.contains(.done) ?? task.states.contains(.done)) || completionPending,
                                owned: facts?.owned, edit: edit)
                        }
                        if let move {
                            TaskMoveMenu(waves: waves, move: move)
                        }
                        Button("Abandon", role: .destructive) { confirmingAbandon = true }
                    } label: {
                        Image(systemName: "ellipsis")
                    }.accessibilityLabel("Task actions")
                }
            }
            if let complete, !(facts?.states.contains(.done) ?? task.states.contains(.done)) || completionPending {
                ToolbarItem(placement: .topBarTrailing) {
                    TaskCompleteButton(complete: complete, pending: completionPending)
                }
            }
        }
        .confirmationDialog("Abandon \(task.identifier ?? task.work.id)?", isPresented: $confirmingAbandon) {
            Button("Abandon Task", role: .destructive) { abandon?() }
        } message: {
            Text("Cancels the Task, closes its PRs and deletes its branches. Unassign it instead to keep its work for later.")
        }
        .task(id: client.interaction.creations.map { "\($0.id):\($0.taskID ?? "")" }) {
            try? await client.interaction.presented(.task(task.work.id))
        }
        .onAppear { PerfClock.interaction(.taskSelect, .contentMounted, id: task.work.id) }
    }
}

struct TaskOwnershipActions: View {
    let completed: Bool
    let owned: Bool?
    let edit: (WorkInteraction.TaskEdit) -> Void

    var body: some View {
        if completed {
            Button("Reopen") { edit(.reopen) }.accessibilityIdentifier("task-reopen")
        } else {
            Button(owned == true ? "Unassign" : "Claim") { edit(owned == true ? .unassign : .claim) }
                .disabled(owned == nil).accessibilityIdentifier("task-claim-unassign")
        }
    }
}

struct TaskMoveMenu: View {
    let waves: [Wave]
    let move: (String?) -> Void
    var body: some View {
        Menu("Move to Wave") {
            Button("No Wave") { move(nil) }
            ForEach(waves) { wave in Button(wave.displayName) { move(wave.id) } }
        }
    }
}

struct TaskCompleteButton: View {
    let complete: () -> Void
    var pending = false
    var body: some View {
        Button(pending ? OperationDeclaration.completionProgress : "Complete", action: complete)
            .disabled(pending).accessibilityIdentifier("task-complete")
    }
}

/// A read that is loading, failed or here.
private enum Loaded<Value> {
    case loading, failed(String), value(Value)
}

private struct LoadFailure: View {
    let message: String
    let retry: () -> Void

    var body: some View {
        ContentUnavailableView {
            Label("Couldn’t load", systemImage: "wifi.exclamationmark")
        } description: {
            Text(message)
        } actions: {
            Button("Try again", action: retry)
        }
    }
}

/// Files the Task changed from its base, each opening its diff.
struct TaskChangesView: View {
    let client: RemoteMobileClient
    let task: WorkViewPlan.Content
    let theme: ThemeDefinition
    @State private var changes = Loaded<TaskChangesSnapshot>.loading

    var body: some View {
        Group {
            switch changes {
            case .loading: ProgressView()
            case .failed(let message): LoadFailure(message: message) { Task { await load() } }
            case .value(let changes) where changes.files.isEmpty:
                ContentUnavailableView("No changes yet", systemImage: "plusminus")
            case .value(let changes):
                List(changes.files) { file in
                    NavigationLink {
                        TaskDiffView(client: client, task: task, path: file.path, theme: theme)
                    } label: {
                        VStack(alignment: .leading, spacing: 4) {
                            Text((file.path as NSString).lastPathComponent)
                                .font(Typography.strong(16))
                                .foregroundStyle(theme.palette.text)
                            Text(detail(file))
                                .font(Typography.caption(13))
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
        do { changes = .value(try await client.query.taskChanges(issue: task.work.id)) } catch {
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
    let task: WorkViewPlan.Content
    let path: String
    let theme: ThemeDefinition
    @State private var file = Loaded<TaskFileSnapshot>.loading

    var body: some View {
        Group {
            switch file {
            case .loading: ProgressView()
            case .failed(let message): LoadFailure(message: message) { Task { await load() } }
            case .value(let file):
                if let content = file.content {
                    ScrollView([.vertical, .horizontal]) {
                        Text(content)
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundStyle(theme.palette.text)
                            .textSelection(.enabled)
                            .padding(12)
                    }
                } else {
                    ContentUnavailableView(file.state.title, systemImage: "doc")
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
        do { file = .value(try await client.query.taskFile(issue: task.work.id, path: path)) } catch {
            file = .failed(machineConnectionMessage(error))
        }
    }
}

struct TaskDiffView: View {
    let client: RemoteMobileClient
    let task: WorkViewPlan.Content
    let path: String
    let theme: ThemeDefinition
    @State private var diff = Loaded<TaskDiffSnapshot>.loading

    var body: some View {
        Group {
            switch diff {
            case .loading: ProgressView()
            case .failed(let message): LoadFailure(message: message) { Task { await load() } }
            case .value(let diff) where diff.binary:
                ContentUnavailableView("Binary file", systemImage: "doc")
            case .value(let diff):
                ScrollView([.vertical, .horizontal]) {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        let lines = Self.lines(diff.patch)
                        ForEach(lines.indices, id: \.self) { index in
                            Text(lines[index].text.isEmpty ? " " : String(lines[index].text))
                                .font(.system(size: 12, design: .monospaced))
                                .foregroundStyle(theme.palette.text)
                                .padding(.horizontal, 12)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(tint(lines[index].kind))
                        }
                        if diff.truncated {
                            Text("The rest of this diff is too long to show.")
                                .font(Typography.caption(13))
                                .foregroundStyle(theme.palette.textSecondary)
                                .padding(12)
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
        do { diff = .value(try await client.query.taskDiff(issue: task.work.id, path: path)) } catch {
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
