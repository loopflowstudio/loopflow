// Work inspectors consume the same planning reading as the compact navigator.

#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

struct WorkSurfaceView: View {
    @Bindable var model: WorkModel
    var onOpenSession: (SessionRecord) -> Void = { _ in }
    /// Starts an independent Task-context conversation in the Task's checkout.
    var onNewSession: ((String) async throws -> Void)?

    @Environment(\.palette) private var palette
    @State private var controlError: String?
    @State private var editingTask: RoadmapTask?

    private var snapshot: RoadmapSnapshot? { model.roadmap.value }
    private var queryError: String? { model.roadmap.errorMessage }
    private var visibleWaves: [WaveRoadmap] { model.visibleRoadmaps }

    var body: some View {
        VStack(spacing: 0) {
            if let queryError {
                evidenceBanner(
                    title: snapshot == nil
                        ? "Roadmap unavailable"
                        : "Refresh failed — showing the last roadmap",
                    detail: queryError
                )
            }
            if let controlError {
                evidenceBanner(title: "Control failed", detail: controlError)
            }
            if let selection = model.selection, selection.kind == .task,
               let error = model.navigation.taskSessionErrors[selection.id] {
                evidenceBanner(title: "New session failed", detail: error)
            }
            content
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background(palette.background)
        .sheet(item: $editingTask) { task in
            TaskDirectiveEditor(model: model, task: task)
        }
    }

    // MARK: - Content routing

    @ViewBuilder
    private var content: some View {
        if let selection = model.selection, selection.kind == .task,
           model.navigation.selectedTaskEvidence?.task.id == selection.id {
            taskDetail
        } else if snapshot == nil, queryError == nil {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .accessibilityIdentifier("loopflow-work-loading")
        } else if snapshot == nil {
            ContentUnavailableView(
                "Work unavailable",
                systemImage: "exclamationmark.triangle",
                description: Text("Couldn't read the latest work. Refresh to try again.")
            )
            .accessibilityIdentifier("loopflow-work-unavailable")
        } else if visibleWaves.isEmpty, model.visibleWaves.isEmpty, model.visibleTasks.isEmpty {
            ContentUnavailableView(
                model.repoPath == nil ? "No planned Work yet" : "No planned Work in this repository",
                systemImage: "map",
                description: Text("Open the repository conversation to decide what to work on.")
            )
            .accessibilityIdentifier("loopflow-work-empty")
        } else {
            switch model.selection?.kind {
            case nil:
                ContentUnavailableView("Choose Work", systemImage: "list.bullet")
            case .wave:
                if let selection = model.selection { WaveStandupBoard(model: model, view: .wave(selection.id)) }
            case .project:
                missingSelection
            case .task:
                taskDetail
            }
        }
    }

    private func scrollingDetail<Body: View>(
        identifier: String,
        @ViewBuilder body: () -> Body
    ) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 26) {
                body()
            }
            .padding(.horizontal, 36)
            .padding(.top, 30)
            .padding(.bottom, 72)
            .frame(maxWidth: 1160, alignment: .leading)
            .frame(maxWidth: .infinity, alignment: .center)
        }
        .accessibilityIdentifier(identifier)
    }

    // MARK: - Task detail

    @ViewBuilder
    private var taskDetail: some View {
        if let selection = model.selection, let found = model.task(id: selection.id) {
            let task = found.task
            let sessions = model.sessions.value.map { _ in
                model.visibleSessions.filter { record in
                    guard record.primaryScope == nil else { return false }
                    return task.runtime.map {
                        record.taskIds.contains($0.workId) || record.workspace?.taskId == $0.workId
                    } ?? false
                }
            }
            scrollingDetail(identifier: "loopflow-detail-task") {
                VStack(alignment: .leading, spacing: Spacing.sm) {
                    HStack(alignment: .firstTextBaseline, spacing: Spacing.md) {
                        Text(task.task.name)
                            .font(Typography.title)
                            .foregroundStyle(palette.text)
                            .textSelection(.enabled)
                            .fixedSize(horizontal: false, vertical: true)
                            .accessibilityIdentifier("task-title")
                        Spacer(minLength: Spacing.sm)
                        if let onNewSession {
                            Button {
                                let navigation = model.navigation
                                guard navigation.startingTaskSessions.insert(task.id).inserted else { return }
                                navigation.taskSessionErrors[task.id] = nil
                                Task {
                                    defer { navigation.startingTaskSessions.remove(task.id) }
                                    do { try await onNewSession(task.id) }
                                    catch { navigation.taskSessionErrors[task.id] = error.localizedDescription }
                                }
                            } label: {
                                Label("New session", systemImage: "plus.bubble")
                            }
                            .buttonStyle(WorkOutlineButtonStyle())
                            .disabled(model.navigation.startingTaskSessions.contains(task.id))
                            .help("Open an independent conversation with this Task's context in its worktree. No Graph is started.")
                            .accessibilityIdentifier("task-new-session")
                        }
                    }
                    HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                        Text(task.condition.reason)
                            .font(Typography.body(13))
                            .foregroundStyle(palette.textSecondary)
                            .accessibilityLabel(taskConditionAccessibilityLabel(task))
                        Spacer()
                        if let workspace = task.reference.workspace {
                            Button("Worktree") { openWorktree(workspace) }
                                .buttonStyle(WorkPlainButtonStyle())
                                .font(Typography.body(12.5))
                        }
                        if let github = task.pr?.publication?.github {
                            Link("PR #\(github.number)", destination: github.url)
                                .buttonStyle(WorkLinkStyle())
                                .font(Typography.body(12.5))
                        }
                    }
                    .tint(palette.actionInk)
                    .foregroundStyle(palette.actionInk)
                    TaskPlanningFactsView(task: task)
                    if task.task.acquisition.state != .available {
                        Text("Planning evidence: \(task.task.acquisition.state.rawValue)")
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                    }
                    PlanningSyncView(sync: task.task.sync)
                    TaskWorkflowHeader(model: model, task: task, work: model.taskWork[task.id])
                        .id(task.id)
                }
                if let unavailable = found.wave?.unavailableTasks.first(where: { $0.taskId == task.id }) {
                    evidenceBanner(title: "Retained Task · planning unavailable", detail: unavailable.reason)
                }
                TaskHistoryView(model: model, task: task)
                    .id(task.id)
                // Raw records, for when the Workflow and the Graph process log disagree with them.
                TaskDiagnosticsView(model: model, task: task)
                    .id(task.id)
                    .font(Typography.body(12))
                    .foregroundStyle(palette.textSecondary)
                    .accessibilityIdentifier("task-debug")
                // Exact Available links preserve access to secondary conversations
                // needing input without restoring a Session inventory.
                ForEach((sessions ?? []).filter { $0.availability == .available }) { session in
                    Button { onOpenSession(session) } label: {
                        Label(session.readySummary ?? "Input needed in \(session.title)", systemImage: "bubble.left")
                    }
                    .buttonStyle(WorkPlainButtonStyle())
                    .accessibilityIdentifier("task-available-\(session.id)")
                }
                section {
                    WorkSectionHeading(title: "Description") {
                        Button("Edit description") {
                            editingTask = task
                        }
                        .buttonStyle(WorkPlainButtonStyle())
                        .font(Typography.body(12.5))
                        .foregroundStyle(palette.textTertiary)
                        .accessibilityIdentifier("work-edit-directive")
                    }
                    Group {
                        if task.task.description.isEmpty {
                            Text("No description recorded.").foregroundStyle(palette.textSecondary)
                        } else {
                            MarkdownBlocks(source: task.task.description).equatable()
                        }
                    }
                    .frame(maxWidth: 680, alignment: .leading)
                    .accessibilityIdentifier("work-task-directive")
                }
                TaskCommentsView(model: model, task: task)
            }
            .onChange(of: task.id, initial: true) { _, id in
                Perf.endAfterCommit(Perf.taskWorkspaceReady, id: id)
            }
        } else {
            missingSelection
        }
    }

    /// A heading and its content, spaced as one unit.
    private func section<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 10) { content() }
    }

    // MARK: - Shared pieces

    private var missingSelection: some View {
        ContentUnavailableView(
            "Selected Work is unavailable",
            systemImage: "questionmark.circle",
            description: Text("This planning read cannot resolve the selection. Its workspace and conversations remain retained.")
        )
    }

    private func evidenceBanner(title: String, detail: String) -> some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(palette.warning)
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                Text(title)
                    .font(Typography.strong(11))
                    .foregroundStyle(palette.text)
                Text(detail)
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
            }
            Spacer()
        }
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.sm)
        .background(palette.warning.opacity(0.12))
    }

    // MARK: - Controls

    private func openWorktree(_ workspace: TaskWorktreeSnapshot) {
        var components = URLComponents()
        components.scheme = "warp"
        components.host = "action"
        components.path = "/new_window"
        components.queryItems = [URLQueryItem(name: "path", value: workspace.worktree)]
        guard let url = components.url, NSWorkspace.shared.open(url) else {
            controlError = "Warp could not open \(workspace.worktree)."
            return
        }
        controlError = nil
    }
}

struct TaskDirectiveEditor: View {
    let model: WorkModel
    let task: RoadmapTask

    @Environment(\.dismiss) private var dismiss
    @Environment(\.palette) private var palette
    @State private var draft: String
    @State private var isSaving = false
    @State private var saveError: String?

    init(model: WorkModel, task: RoadmapTask) {
        self.model = model
        self.task = task
        _draft = State(initialValue: task.task.description)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            Text("Edit description · \(task.task.identifier)")
                .font(Typography.strong(14))
            TextEditor(text: $draft)
                .font(Typography.body(13))
                .scrollContentBackground(.hidden)
                .padding(Spacing.sm)
                .background(palette.surfaceMuted)
                .accessibilityIdentifier("task-directive-draft")
                .disabled(isSaving)
            if let saveError {
                Text(saveError)
                    .font(Typography.body(12))
                    .foregroundStyle(palette.warning)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("task-directive-error")
            }
            HStack {
                Button("Cancel") { dismiss() }
                    .keyboardShortcut(.cancelAction)
                    .disabled(isSaving)
                Spacer()
                if isSaving { ProgressView().controlSize(.small) }
                Button("Save description") {
                    isSaving = true
                    saveError = nil
                    Task {
                        defer { isSaving = false }
                        do {
                            try await model.updateTaskDirective(task: task, text: draft)
                            dismiss()
                        } catch {
                            saveError = error.localizedDescription
                        }
                    }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(isSaving)
            }
        }
        .padding(Spacing.xl)
        .frame(minWidth: 520, minHeight: 360)
        .foregroundStyle(palette.text)
        .background(palette.background)
        .interactiveDismissDisabled(isSaving)
    }
}
#endif
