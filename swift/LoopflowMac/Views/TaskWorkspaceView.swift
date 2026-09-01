#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

struct TaskTerminal: Identifiable, Hashable {
    let id: String
    let taskId: String
    let issueIdentifier: String
    let worktree: String
    let title: String
    let tmuxName: String
}

@MainActor
final class TaskTerminalStore: ObservableObject {
    static let shared = TaskTerminalStore()

    @Published private var tabsByTask: [String: [TaskTerminal]] = [:]
    @Published private var selectedTabByTask: [String: String] = [:]

    func tabs(for taskId: String) -> [TaskTerminal] {
        tabsByTask[taskId] ?? []
    }

    func selectedTab(for taskId: String) -> TaskTerminal? {
        let tabs = tabs(for: taskId)
        guard let selected = selectedTabByTask[taskId] else { return tabs.first }
        return tabs.first { $0.id == selected } ?? tabs.first
    }

    @discardableResult
    func addTerminal(
        taskId: String,
        issueIdentifier: String,
        worktree: String
    ) async throws -> TaskTerminal {
        let number = tabs(for: taskId).count + 1
        let token = UUID().uuidString.lowercased().prefix(8)
        let taskToken = taskId
            .lowercased()
            .filter { $0.isLetter || $0.isNumber || $0 == "-" }
            .prefix(18)
        let tmuxName = "lf-task-ui-\(taskToken)-\(token)"
        try await TmuxSession(sessionName: tmuxName, worktreePath: worktree).ensureBaseSession()
        let tab = TaskTerminal(
            id: "terminal-\(UUID().uuidString.lowercased())",
            taskId: taskId,
            issueIdentifier: issueIdentifier,
            worktree: worktree,
            title: "Shell \(number)",
            tmuxName: tmuxName
        )
        tabsByTask[taskId, default: []].append(tab)
        selectedTabByTask[taskId] = tab.id
        return tab
    }

    func select(_ tab: TaskTerminal, taskId: String) {
        guard tabs(for: taskId).contains(tab) else { return }
        selectedTabByTask[taskId] = tab.id
    }

    func close(_ tab: TaskTerminal, taskId: String) {
        GhosttyManager.shared.destroySession(tab.id)
        TmuxSessionRegistry.shared.killSession(named: tab.tmuxName)
        tabsByTask[taskId]?.removeAll { $0.id == tab.id }
        if selectedTabByTask[taskId] == tab.id {
            selectedTabByTask[taskId] = tabs(for: taskId).last?.id
        }
    }
}

enum TaskWorkspaceSection: String, CaseIterable, Identifiable, Hashable {
    case changes = "Changes"
    case terminal = "Terminal"

    var id: String { rawValue }
}

private enum TaskPreviewMode: String, CaseIterable, Identifiable {
    case diff = "Diff"
    case file = "File"

    var id: String { rawValue }
}

struct TaskWorkspaceView: View {
    let task: TaskPlanningSnapshot
    let reference: TaskReferenceSnapshot
    let runtime: TaskRuntimeSnapshot?
    let repoPath: String
    @ObservedObject var terminalStore: TaskTerminalStore

    @Environment(\.palette) private var palette
    @State private var section = TaskWorkspaceSection.changes
    @State private var changes: TaskChangesSnapshot?
    @State private var selectedPath: String?
    @State private var previewMode = TaskPreviewMode.diff
    @State private var diff: TaskDiffSnapshot?
    @State private var file: TaskFileSnapshot?
    @State private var error: String?
    @State private var loading = false

    init(
        task: TaskPlanningSnapshot,
        reference: TaskReferenceSnapshot,
        runtime: TaskRuntimeSnapshot?,
        repoPath: String,
        terminalStore: TaskTerminalStore,
        initialSection: TaskWorkspaceSection = .changes
    ) {
        self.task = task
        self.reference = reference
        self.runtime = runtime
        self.repoPath = repoPath
        self.terminalStore = terminalStore
        _section = State(initialValue: initialSection)
    }

    private var workspace: TaskWorkspaceSnapshot? { reference.workspace }
    private var previewIdentity: String {
        "\(task.identifier)|\(selectedPath ?? "")|\(previewMode.rawValue)"
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            if let runtime, let workspace {
                switch section {
                case .changes:
                    changesView
                case .terminal:
                    TaskTerminalWorkspaceView(
                        taskId: runtime.workId,
                        issueIdentifier: task.identifier,
                        worktree: workspace.worktree,
                        store: terminalStore
                    )
                }
            } else {
                ContentUnavailableView(
                    "Task has not started",
                    systemImage: "hammer",
                    description: Text("Start the Task before opening a workspace.")
                )
            }
        }
        .frame(minWidth: 820, minHeight: 560)
        .background(palette.background)
        .task(id: runtime?.workId) { await loadChanges() }
        .task(id: previewIdentity) { await loadPreview() }
    }

    private var header: some View {
        HStack(spacing: Spacing.md) {
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                Text("\(task.identifier) · \(task.name)")
                    .font(Typography.sectionTitle(15))
                    .foregroundStyle(palette.text)
                if let workspace {
                    Text(workspace.worktree)
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundStyle(palette.textSecondary)
                        .textSelection(.enabled)
                }
            }
            Spacer()
            Picker("Workspace", selection: $section) {
                ForEach(TaskWorkspaceSection.allCases) { section in
                    Text(section.rawValue).tag(section)
                }
            }
            .pickerStyle(.segmented)
            .frame(width: 210)
            Button("Warp") { openWarp() }
                .disabled(workspace == nil)
            Button {
                Task { await loadChanges() }
            } label: {
                Image(systemName: "arrow.clockwise")
            }
            .buttonStyle(.borderless)
            .help("Refresh Task changes")
            .disabled(runtime == nil || workspace == nil || loading)
        }
        .padding(Spacing.md)
    }

    private var changesView: some View {
        HSplitView {
            VStack(spacing: 0) {
                if let changes, changes.files.isEmpty {
                    ContentUnavailableView(
                        "No changes",
                        systemImage: "checkmark.circle",
                        description: Text("This Task matches its recorded base commit.")
                    )
                } else {
                    List(selection: $selectedPath) {
                        ForEach(changes?.files ?? []) { changedFile in
                            changedFileRow(changedFile)
                                .tag(changedFile.path)
                        }
                    }
                    .listStyle(.sidebar)
                }
            }
            .frame(minWidth: 220, idealWidth: 280, maxWidth: 340)

            VStack(alignment: .leading, spacing: 0) {
                HStack {
                    Picker("Preview", selection: $previewMode) {
                        ForEach(TaskPreviewMode.allCases) { mode in
                            Text(mode.rawValue).tag(mode)
                        }
                    }
                    .pickerStyle(.segmented)
                    .frame(width: 150)
                    Spacer()
                    if loading { ProgressView().controlSize(.small) }
                }
                .padding(Spacing.sm)
                Divider()
                preview
            }
            .frame(minWidth: 480, maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private func changedFileRow(_ changedFile: TaskChangedFile) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xxs) {
            Text(changedFile.path)
                .font(.system(size: 11, design: .monospaced))
                .lineLimit(2)
            Text(changeLabels(changedFile).joined(separator: " · "))
                .font(Typography.caption(9))
                .foregroundStyle(palette.textSecondary)
        }
        .padding(.vertical, Spacing.xxs)
    }

    @ViewBuilder
    private var preview: some View {
        if let error {
            ContentUnavailableView(
                "Workspace unavailable",
                systemImage: "exclamationmark.triangle",
                description: Text(error)
            )
        } else if selectedPath == nil {
            ContentUnavailableView(
                "Select a changed file",
                systemImage: "doc.text.magnifyingglass"
            )
        } else {
            let text = previewMode == .diff ? diff?.patch : file?.content
            let isBinary = previewMode == .diff ? (diff?.binary ?? false) : (file?.binary ?? false)
            if isBinary {
                ContentUnavailableView("Binary file", systemImage: "doc.badge.ellipsis")
            } else if let text {
                ScrollView([.horizontal, .vertical]) {
                    Text(text.isEmpty ? "No textual changes." : text)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(palette.text)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .topLeading)
                        .padding(Spacing.md)
                }
                .overlay(alignment: .bottomTrailing) {
                    if (previewMode == .diff ? diff?.truncated : file?.truncated) == true {
                        Text("Truncated at 1 MB")
                            .font(Typography.caption(9))
                            .padding(Spacing.xs)
                            .background(.regularMaterial)
                            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
                            .padding(Spacing.sm)
                    }
                }
            } else if loading {
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
    }

    private func changeLabels(_ file: TaskChangedFile) -> [String] {
        var labels: [String] = []
        if file.committed { labels.append("committed") }
        if file.staged { labels.append("staged") }
        if file.unstaged { labels.append("unstaged") }
        if file.untracked { labels.append("untracked") }
        return labels
    }

    @MainActor
    private func loadChanges() async {
        guard runtime != nil else { return }
        loading = true
        defer { loading = false }
        do {
            let next = try await RegistryQueryLocal.shared.taskChanges(
                issue: task.identifier,
                cwd: repoPath
            )
            changes = next
            if !next.files.contains(where: { $0.path == selectedPath }) {
                selectedPath = next.files.first?.path
            }
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
    }

    @MainActor
    private func loadPreview() async {
        guard runtime != nil, let selectedPath else { return }
        loading = true
        defer { loading = false }
        do {
            switch previewMode {
            case .diff:
                diff = try await RegistryQueryLocal.shared.taskDiff(
                    issue: task.identifier,
                    path: selectedPath,
                    cwd: repoPath
                )
            case .file:
                file = try await RegistryQueryLocal.shared.taskFile(
                    issue: task.identifier,
                    path: selectedPath,
                    cwd: repoPath
                )
            }
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func openWarp() {
        guard let workspace else { return }
        var components = URLComponents()
        components.scheme = "warp"
        components.host = "action"
        components.path = "/new_window"
        components.queryItems = [URLQueryItem(name: "path", value: workspace.worktree)]
        guard let url = components.url, NSWorkspace.shared.open(url) else {
            error = "Warp could not open the Task worktree."
            return
        }
    }
}

private struct TaskTerminalWorkspaceView: View {
    let taskId: String
    let issueIdentifier: String
    let worktree: String
    @ObservedObject var store: TaskTerminalStore

    @Environment(\.palette) private var palette
    @State private var error: String?
    @State private var preparing = false

    private var tabs: [TaskTerminal] { store.tabs(for: taskId) }
    private var selected: TaskTerminal? { store.selectedTab(for: taskId) }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: Spacing.xs) {
                ScrollView(.horizontal) {
                    HStack(spacing: Spacing.xs) {
                        ForEach(tabs) { tab in
                            HStack(spacing: Spacing.xxs) {
                                Button(tab.title) { store.select(tab, taskId: taskId) }
                                    .buttonStyle(.borderless)
                                    .font(Typography.caption(10))
                                Button {
                                    store.close(tab, taskId: taskId)
                                } label: {
                                    Image(systemName: "xmark")
                                        .font(Typography.caption(8))
                                }
                                .buttonStyle(.borderless)
                            }
                            .padding(.horizontal, Spacing.sm)
                            .padding(.vertical, Spacing.xs)
                            .background(tab.id == selected?.id ? palette.surfaceMuted : Color.clear)
                            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
                        }
                    }
                }
                Button {
                    Task { await addTerminal() }
                } label: {
                    Image(systemName: "plus")
                }
                .buttonStyle(.borderless)
                .help("New Task terminal")
                .disabled(preparing)
            }
            .padding(Spacing.sm)
            Divider()
            if let error {
                ContentUnavailableView(
                    "Terminal unavailable",
                    systemImage: "exclamationmark.triangle",
                    description: Text(error)
                )
            } else if let selected {
                GhosttyTerminalView(
                    workingDirectory: worktree,
                    argv: ["tmux", "attach-session", "-t", selected.tmuxName],
                    sessionId: selected.id
                )
                .id(selected.id)
                .background(LoopflowPalette.dark.background)
            } else if preparing {
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .task(id: taskId) {
            guard tabs.isEmpty else { return }
            await addTerminal()
        }
    }

    @MainActor
    private func addTerminal() async {
        preparing = true
        defer { preparing = false }
        do {
            _ = try await store.addTerminal(
                taskId: taskId,
                issueIdentifier: issueIdentifier,
                worktree: worktree
            )
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
    }
}
#endif
