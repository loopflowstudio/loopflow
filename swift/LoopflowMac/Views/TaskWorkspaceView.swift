#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

struct TaskTerminal: Identifiable, Hashable {
    let id: String
    let title: String
}

@MainActor
final class TaskTerminalStore: ObservableObject {
    let surfaces = GhosttySurfacePool()
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
    func addTerminal(taskId: String) -> TaskTerminal {
        let number = tabs(for: taskId).count + 1
        let tab = TaskTerminal(
            id: "terminal-\(UUID().uuidString.lowercased())",
            title: "Shell \(number)"
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
        surfaces.release(.taskTerminal(tab.id))
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

struct TaskWorkspaceView: View {
    let task: TaskPlanningSnapshot
    let reference: TaskReferenceSnapshot
    let runtime: TaskRuntimeSnapshot?
    let prURL: URL?
    @ObservedObject var terminalStore: TaskTerminalStore
    @Environment(SessionsWorkspaceRegistry.self) private var workspaces

    @Environment(\.palette) private var palette
    @State private var section = TaskWorkspaceSection.changes

    init(
        task: TaskPlanningSnapshot,
        reference: TaskReferenceSnapshot,
        runtime: TaskRuntimeSnapshot?,
        prURL: URL?,
        terminalStore: TaskTerminalStore,
        initialSection: TaskWorkspaceSection = .changes
    ) {
        self.task = task
        self.reference = reference
        self.runtime = runtime
        self.prURL = prURL
        self.terminalStore = terminalStore
        _section = State(initialValue: initialSection)
    }

    private var workspace: TaskWorkspaceSnapshot? { reference.workspace }
    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            if let runtime, let workspace {
                switch section {
                case .changes:
                    TaskFilesView(store: workspaces.workspace(for: workspace.worktree).files(taskId: task.id,
                                                             issue: task.identifier, cwd: workspace.worktree), prURL: prURL)
                case .terminal:
                    TaskTerminalWorkspaceView(
                        taskId: runtime.workId,
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
        }
        .padding(Spacing.md)
    }

    private func openWarp() {
        guard let workspace else { return }
        var components = URLComponents()
        components.scheme = "warp"
        components.host = "action"
        components.path = "/new_window"
        components.queryItems = [URLQueryItem(name: "path", value: workspace.worktree)]
        guard let url = components.url, NSWorkspace.shared.open(url) else {
            NSSound.beep()
            return
        }
    }
}

private struct TaskTerminalWorkspaceView: View {
    let taskId: String
    let worktree: String
    @ObservedObject var store: TaskTerminalStore

    @Environment(\.palette) private var palette

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
                    store.addTerminal(taskId: taskId)
                } label: {
                    Image(systemName: "plus")
                }
                .buttonStyle(.borderless)
                .help("New Task terminal")
            }
            .padding(Spacing.sm)
            Divider()
            if let selected {
                GhosttyTerminalView(
                    workingDirectory: worktree,
                    terminal: .taskTerminal(selected.id),
                    surfacePool: store.surfaces
                )
                .id(selected.id)
                .background(TerminalPalette.background)
            }
        }
        .task(id: taskId) {
            guard tabs.isEmpty else { return }
            store.addTerminal(taskId: taskId)
        }
    }

}
#endif
