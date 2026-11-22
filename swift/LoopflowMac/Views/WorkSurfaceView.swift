// Work inspectors consume the same planning reading as the compact navigator.

#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// A plan with no Waves and no Tasks. A repository's stands in for its page
/// (`WorkModel.repositoryIsEmpty`) and says what to do next.
struct NoPlannedWork: View {
    let model: WorkModel
    /// Opens the repository's conversation in its workspace; nil outside a repository.
    var openConversation: (() -> Void)?

    var body: some View {
        Group {
            if let openConversation {
                EmptyState(title: "No planned Work in this repository",
                           sentence: "Say what you want built, or talk it through first.") {
                    Button("New Task ⌘N") { model.navigation.palette = .newTask }
                        .buttonStyle(LoopflowButtonStyle(.accent))
                        .accessibilityIdentifier("loopflow-work-empty-new-task")
                    Button("Open the repository conversation", action: openConversation)
                        .accessibilityIdentifier("loopflow-work-empty-conversation")
                }
            } else {
                EmptyState(title: "No planned Work yet")
            }
        }
        .accessibilityIdentifier("loopflow-work-empty")
    }
}

/// Nothing is selected in a plan that has Work.
struct ChooseWork: View {
    var body: some View {
        EmptyState(title: "Choose Work",
                   sentence: "Pick a Task or a Wave in the sidebar, or press ⌘K to find one in any repository.")
    }
}

struct WorkSurfaceView: View {
    @Bindable var model: WorkModel
    var onOpenSession: (SessionRecord) -> Void = { _ in }
    /// Starts an independent Task-context conversation in the Task's checkout.
    var onNewSession: (@MainActor (String) async throws -> Void)?

    @Environment(\.palette) private var palette
    @State private var editingTask: RoadmapTask?

    private var snapshot: RoadmapSnapshot? { model.roadmap.value }
    private var visibleWaves: [WaveRoadmap] { model.visibleRoadmaps }

    var body: some View {
        content
            .frame(maxWidth: .infinity, maxHeight: .infinity)
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
        } else if snapshot == nil, model.roadmap.isLoading {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .accessibilityIdentifier("loopflow-work-loading")
        } else if snapshot == nil {
            Color.clear
        } else {
            switch model.selection?.kind {
            case nil:
                ChooseWork()
            case .wave:
                if let selection = model.selection { BacklogView(model: model, view: .wave(selection.id)) }
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
                model.visibleSessions.filter { $0.names(task: task.id) }
            }
            scrollingDetail(identifier: "loopflow-detail-task") {
                VStack(alignment: .leading, spacing: Spacing.sm) {
                    HStack(alignment: .firstTextBaseline, spacing: Spacing.md) {
                        Text(task.task.name)
                            .serif(.title)
                            .foregroundStyle(palette.text)
                            .textSelection(.enabled)
                            .fixedSize(horizontal: false, vertical: true)
                            .accessibilityIdentifier("task-title")
                        Spacer(minLength: Spacing.sm)
                        if let onNewSession {
                            Button {
                                model.newTaskSession(task.id, open: onNewSession)
                            } label: {
                                Label("New session", systemImage: "plus.bubble")
                            }
                            .buttonStyle(LoopflowButtonStyle())
                            .disabled(model.navigation.startingTaskSessions.contains(task.id))
                            .help("Open an independent conversation with this Task's context in its worktree. No Graph is started.")
                            .accessibilityIdentifier("task-new-session")
                        }
                    }
                    HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                        Text(task.condition.reason)
                            .font(Typography.text)
                            .foregroundStyle(palette.textSecondary)
                            .accessibilityLabel(taskConditionAccessibilityLabel(task))
                        Spacer()
                        if let workspace = task.reference.workspace {
                            Button("Worktree") { model.openWorktree(workspace) }
                                .buttonStyle(WorkPlainButtonStyle())
                                .font(Typography.text)
                        }
                        if let parent = model.taskExecution[task.id].value?.stackParent {
                            TaskStackParentLabel(parent: parent)
                        }
                        if let github = task.pr?.publication?.github {
                            Link("PR #\(github.number)", destination: github.url)
                                .buttonStyle(WorkLinkStyle())
                                .font(Typography.text)
                        }
                    }
                    .tint(palette.actionInk)
                    .foregroundStyle(palette.actionInk)
                    TaskPlanningFactsView(task: task)
                    if task.task.trackerState.state != .available {
                        Text("Tracker state: \(task.task.trackerState.state.rawValue)")
                            .font(Typography.meta)
                            .foregroundStyle(palette.textSecondary)
                    }
                    TrackerSyncView(sync: task.task.sync, repo: task.task.repo)
                    TaskWorkflowHeader(model: model, task: task, work: model.taskWork[task.id])
                        .id(task.id)
                }
                if let unavailable = found.wave?.unavailableTasks.first(where: { $0.taskId == task.id }) {
                    FailureLine("Planning is unavailable for this Task.", machine: unavailable.reason)
                        .padding(.horizontal, Spacing.xl)
                        .padding(.vertical, Spacing.sm)
                }
                TaskHistoryView(model: model, task: task)
                    .id(task.id)
                // Raw records, for when the Workflow and the Graph process log disagree with them.
                TaskDiagnosticsView(model: model, task: task)
                    .id(task.id)
                    .font(Typography.text)
                    .foregroundStyle(palette.textSecondary)
                    .accessibilityIdentifier("task-debug")
                // A Task conversation that is not running, or that the server started again, prints the
                // server's sentence: starting, coming back, or the command that opens it.
                if let conversation = (sessions ?? []).first(where: \.taskPrimary),
                   conversation.outlook != nil || conversation.start != nil,
                   let line = conversation.statusLine {
                    Label("Conversation: \(line)", systemImage: "arrow.clockwise")
                        .font(Typography.text)
                        .foregroundStyle(palette.textSecondary)
                        .accessibilityIdentifier("task-conversation-line")
                }
                // Exact Available links preserve access to secondary conversations
                // needing input without restoring a Session inventory.
                ForEach((sessions ?? []).filter { $0.availability == .available }) { session in
                    Button {
                        Perf.userAction(Perf.taskWorkspaceReady, scenario: "session", id: session.id,
                                        repository: model.repoPath, liveSessions: model.liveSessionCount,
                                        owner: model.navigation.performanceOwner) { onOpenSession(session) }
                    } label: {
                        Label("Input needed in \(session.title)", systemImage: "bubble.left")
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
                        .font(Typography.text)
                        .foregroundStyle(palette.textTertiary)
                        .accessibilityIdentifier("work-edit-directive")
                    }
                    Group {
                        if task.task.description.isEmpty {
                            Text("No description recorded.").emptyLine()
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
                Perf.stateObserved(Perf.taskWorkspaceReady, id: id, owner: model.navigation.performanceOwner, inputReady: false)
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
        EmptyState(title: "Selected Work is unavailable",
                   sentence: "The latest plan does not include it. Its workspace and conversations are kept.") {
            Button("Read the plan again") { model.rescanWork() }
                .buttonStyle(LoopflowButtonStyle(.accent))
                .accessibilityIdentifier("loopflow-work-reread")
        }
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
                .font(Typography.textStrong)
            TextEditor(text: $draft)
                .font(Typography.text)
                .scrollContentBackground(.hidden)
                .padding(Spacing.sm)
                .background(palette.surfaceMuted)
                .accessibilityIdentifier("task-directive-draft")
                .disabled(isSaving)
            if let saveError {
                FailureLine("Not saved.", machine: saveError)
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

/// The open Task a stacked Task builds on, as `lf task status` names it.
struct TaskStackParentLabel: View {
    let parent: TaskStackParent
    @Environment(\.palette) private var palette

    var help: String {
        let pr = parent.prNumber.map { " (PR #\($0))" } ?? ""
        return "Builds on \(parent.identifier)'s branch \(parent.branch)\(pr). lf sync main detaches it."
    }

    var body: some View {
        Text("Stacked on \(parent.identifier)")
            .font(Typography.text)
            .foregroundStyle(palette.textSecondary)
            .help(help)
            .accessibilityIdentifier("task-stack-parent")
    }
}

#endif
