import Loopflow
import SwiftUI

/// One Task on the phone. Its conversation fills the screen; the Brief and the
/// Changes that Desktop shows beside it open over it.
struct TaskWorkspaceView: View {
    enum Face: String, Identifiable {
        case brief, changes
        var id: String { rawValue }
    }

    let client: RemoteMobileClient
    let task: WorkViewPlan.Content
    let theme: ThemeDefinition
    @Binding var draft: String
    @State private var face: Face?

    var body: some View {
        Group {
            if task.primarySession == nil {
                TaskBriefView(client: client, task: task, theme: theme)
                    .navigationTitle(task.title)
                    .navigationBarTitleDisplayMode(.inline)
            } else {
                SessionView(client: client, task: task, theme: theme, draft: $draft)
            }
        }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    if task.primarySession != nil {
                        Button("Brief", systemImage: "doc.text") { face = .brief }
                    }
                    Button("Changes", systemImage: "plusminus") { face = .changes }
                } label: { Image(systemName: "ellipsis.circle") }
                    .accessibilityLabel("Brief and Changes")
            }
        }
        .sheet(item: $face) { face in
            NavigationStack {
                Group {
                    switch face {
                    case .brief: TaskBriefView(client: client, task: task, theme: theme).navigationTitle("Brief")
                    case .changes: TaskChangesView(client: client, task: task, theme: theme).navigationTitle("Changes")
                    }
                }
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .confirmationAction) { Button("Done") { self.face = nil } }
                }
            }
            .tint(theme.palette.accentInk)
            .presentationDetents([.medium, .large])
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

/// What the Task is, where it stands on its Graph, its pull request and comments.
struct TaskBriefView: View {
    let client: RemoteMobileClient
    let task: WorkViewPlan.Content
    let theme: ThemeDefinition
    @State private var status = Loaded<WorkFrame.TaskStatusReading>.loading

    var body: some View {
        ScrollView {
            switch status {
            case .loading: ProgressView().padding(.top, 80)
            case .failed(let message): LoadFailure(message: message) { Task { await load() } }
            case .value(let status): brief(status)
            }
        }
        .frame(maxWidth: .infinity)
        .background(theme.palette.background)
        .task { await load() }
        .refreshable { await load() }
    }

    private func load() async {
        do { status = .value(try await client.status(task: task.work.id)) } catch {
            status = .failed(machineConnectionMessage(error))
        }
    }

    private func brief(_ status: WorkFrame.TaskStatusReading) -> some View {
        VStack(alignment: .leading, spacing: 20) {
            VStack(alignment: .leading, spacing: 6) {
                if let row = status.row {
                    Text("\(row.task.identifier) · \(row.section.rawValue.capitalized)")
                        .font(Typography.caption(13))
                        .foregroundStyle(theme.palette.textSecondary)
                }
                Text(task.title).font(Typography.strong(22)).foregroundStyle(theme.palette.text)
            }
            if let row = status.row, !row.nextMove.reason.isEmpty {
                fact("Next", row.nextMove.reason)
            }
            if let graph = status.overview?.graph {
                fact("Graph", "\(graph.name) · \(position(graph.position))")
            }
            if let pull = status.row?.pr?.publication?.github {
                Link(destination: pull.url) {
                    Label("Pull request #\(pull.number)", systemImage: "arrow.triangle.pull")
                        .font(Typography.strong(16))
                        .frame(minHeight: 44)
                }
            }
            if let description = status.row?.task.description, !description.isEmpty {
                Text(prose(description)).font(Typography.body(16)).foregroundStyle(theme.palette.text)
            }
            if let comments = status.comments?.comments, !comments.isEmpty {
                Divider()
                ForEach(comments) { comment in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(author(comment.author))
                            .font(Typography.caption(13))
                            .foregroundStyle(theme.palette.textSecondary)
                        Text(prose(comment.body)).font(Typography.body(15)).foregroundStyle(theme.palette.text)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(20)
        .textSelection(.enabled)
    }

    private func fact(_ name: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(name).font(Typography.caption(13)).foregroundStyle(theme.palette.textSecondary)
            Text(value).font(Typography.body(16)).foregroundStyle(theme.palette.text)
        }
    }

    private func position(_ position: TaskGraph.Position) -> String {
        switch position {
        case .at(let step): "at \(step)"
        case .run(let step, _, let running): running ? "running \(step)" : "stopped at \(step)"
        }
    }

    private func author(_ author: TaskCommentAuthor) -> String {
        switch author {
        case .person(let name): name ?? "Someone"
        case .integration: "Integration"
        case .session(_, let requester): requester.map { "A Session for \($0)" } ?? "A Session"
        }
    }

    private func prose(_ markdown: String) -> AttributedString {
        (try? AttributedString(markdown: markdown, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)))
            ?? AttributedString(markdown)
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
