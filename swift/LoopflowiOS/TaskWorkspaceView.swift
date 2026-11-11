import Loopflow
import SwiftUI

/// One Task on the phone. Its conversation fills the screen; the Changes that
/// Desktop shows beside it open over it.
struct TaskWorkspaceView: View {
    let client: RemoteMobileClient
    let task: WorkViewPlan.Content
    let theme: ThemeDefinition
    @Binding var draft: String
    @State private var showsChanges = false

    var body: some View {
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
        do { changes = .value(try await client.changes(task: task.work.id)) } catch {
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
                    ContentUnavailableView("Can’t show this file", systemImage: "doc")
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
        do { file = .value(try await client.file(task: task.work.id, path: path)) } catch {
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
                        let lines = diff.patch.split(separator: "\n", omittingEmptySubsequences: false)
                        ForEach(lines.indices, id: \.self) { index in
                            Text(lines[index].isEmpty ? " " : String(lines[index]))
                                .font(.system(size: 12, design: .monospaced))
                                .foregroundStyle(theme.palette.text)
                                .padding(.horizontal, 12)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(tint(lines[index]))
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
        do { diff = .value(try await client.diff(task: task.work.id, path: path)) } catch {
            diff = .failed(machineConnectionMessage(error))
        }
    }

    private func tint(_ line: Substring) -> Color {
        if line.hasPrefix("+"), !line.hasPrefix("+++") { return Color.green.opacity(0.16) }
        if line.hasPrefix("-"), !line.hasPrefix("---") { return Color.red.opacity(0.16) }
        if line.hasPrefix("@@") { return theme.palette.surface }
        return .clear
    }
}
