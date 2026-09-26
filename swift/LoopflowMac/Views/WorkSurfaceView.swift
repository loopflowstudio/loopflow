// Work inspectors consume the same planning reading as the compact navigator.

#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

struct WorkSurfaceView: View {
    @Bindable var model: PodiumModel
    var onOpenSession: (SessionRecord) -> Void = { _ in }
    /// Opens a Task the way the sidebar does: its one Session, else its overview.
    var onOpenTask: ((WorkReference) -> Void)?
    /// Starts an independent Task-context conversation in the Task's checkout.
    var onNewSession: ((String) async throws -> Void)?

    @Environment(\.palette) private var palette
    @State private var controlError: String?
    @State private var startingSession = false
    @State private var activeControlId: String?
    @State private var editingTask: WorkTaskSelection?

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
            content
        }
        .background(palette.background)
        .sheet(item: $model.historyWave) { wave in
            ChapterHistoryView(wave: wave.name, repo: wave.repo, sourceReference: model.historyReference)
        }
        .sheet(item: $editingTask) { selection in
            TaskDirectiveEditor(model: model, task: selection.task, wave: selection.wave)
        }
    }

    // MARK: - Content routing

    @ViewBuilder
    private var content: some View {
        if snapshot == nil, queryError == nil {
            ProgressView("Reading roadmap…")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .accessibilityIdentifier("podium-work-loading")
        } else if snapshot == nil {
            ContentUnavailableView(
                "Work unavailable",
                systemImage: "exclamationmark.triangle",
                description: Text("Couldn't read the latest work. Refresh to try again.")
            )
            .accessibilityIdentifier("podium-work-unavailable")
        } else if visibleWaves.isEmpty, model.visibleWaves.isEmpty {
            ContentUnavailableView(
                model.repoPath == nil ? "No planned Work yet" : "No planned Work in this repository",
                systemImage: "map",
                description: Text("Author a Wave to put work on the surface.")
            )
            .accessibilityIdentifier("podium-work-empty")
        } else {
            switch model.selection?.kind {
            case nil:
                ContentUnavailableView("Choose Work", systemImage: "list.bullet")
            case .wave:
                waveDetail
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
            VStack(alignment: .leading, spacing: Spacing.lg) {
                body()
            }
            .padding(Spacing.xl)
            .frame(maxWidth: 920, alignment: .leading)
            .frame(maxWidth: .infinity, alignment: .center)
        }
        .accessibilityIdentifier(identifier)
    }

    // MARK: - Wave detail

    @ViewBuilder
    private var waveDetail: some View {
        if let selection = model.selection, let roadmap = model.wave(id: selection.id) {
            scrollingDetail(identifier: "podium-detail-wave") {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.md) {
                    Text(roadmap.wave.name)
                        .font(Typography.sectionTitle(30))
                        .foregroundStyle(palette.text)
                        .accessibilityIdentifier("wave-title")
                    if roadmap.wave.paused {
                        pausedChip(roadmap.wave.id)
                    }
                    Spacer()
                }
                if !roadmap.wave.goal.isEmpty {
                    Text(roadmap.wave.goal)
                        .font(Typography.body(14))
                        .foregroundStyle(palette.text)
                        .textSelection(.enabled)
                        .accessibilityIdentifier("wave-objective")
                }

                sectionHeading("Current KRs")
                if let chapter = roadmap.chapter {
                    WaveChapterView(chapter: chapter)
                } else {
                    Text("No current chapter plan.").foregroundStyle(palette.textSecondary)
                }

                switch roadmap.tasks {
                case .unavailable(let reason):
                    sectionHeading("Tasks")
                    Text(reason).foregroundStyle(Color.statusWarning)
                case .available(let tasks, let truncated):
                    sectionHeading("Tasks · \(tasks.count)")
                    if tasks.isEmpty { Text("No Tasks in this chapter.").foregroundStyle(palette.textSecondary) }
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(tasks.sorted { $0.task.rank < $1.task.rank }) { task in
                            planRow(task)
                        }
                    }
                    if truncated {
                        Text("Planning is partial; more Tasks exist.").foregroundStyle(Color.statusWarning)
                    }
                }
                ForEach(roadmap.unavailableTasks, id: \.taskId) { task in
                    Text("\(task.taskIdentifier): \(task.reason) · \(task.recovery)")
                        .foregroundStyle(Color.statusWarning)
                        .textSelection(.enabled)
                }
                WaveMetricPortfolioView(portfolio: roadmap.metricPortfolio)
                Button("Chapter history") {
                    model.historyReference = nil
                    model.historyWave = roadmap.wave
                }
                .buttonStyle(.link)
                DisclosureGroup("Wave controls") {
                    HomeControl(
                        wave: roadmap.wave,
                        onOpen: {},
                        onRefresh: { await model.refresh() },
                        onSetPaused: { paused in
                            try await model.setWavePaused(waveId: roadmap.wave.id, paused: paused)
                        },
                        onError: { controlError = $0 }
                    )
                }
            }
        } else if let selection = model.selection, let roster = model.rosterWave(id: selection.id) {
            // Authored but never served: there is no roadmap to show yet.
            scrollingDetail(identifier: "podium-detail-wave") {
                surfaceHeader(roster.displayName, subtitle: "Authored — not yet served")
                Text("Planning evidence is not available for this authored Wave.")
                    .font(Typography.body(12))
                    .foregroundStyle(palette.textSecondary)
            }
        } else {
            missingSelection
        }
    }

    // MARK: - Task detail

    @ViewBuilder
    private var taskDetail: some View {
        if let selection = model.selection, let found = model.task(id: selection.id) {
            let task = found.task
            let sessions = model.workspace.waves.lazy.flatMap(\.tasks)
                .first { $0.id.work == selection }?.sessions
            scrollingDetail(identifier: "podium-detail-task") {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.md) {
                    Text(task.task.name)
                        .font(Typography.body(22).weight(.semibold))
                        .foregroundStyle(palette.text)
                        .textSelection(.enabled)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityIdentifier("task-title")
                    Spacer(minLength: Spacing.sm)
                    if let onNewSession {
                        Button {
                            startingSession = true
                            controlError = nil
                            Task {
                                defer { startingSession = false }
                                do { try await onNewSession(task.id) }
                                catch { controlError = error.localizedDescription }
                            }
                        } label: {
                            Label("New session", systemImage: "plus.bubble")
                        }
                        .disabled(startingSession)
                        .help("Open an independent conversation with this Task's context in its worktree. The managed Flow is not started.")
                        .accessibilityIdentifier("task-new-session")
                    }
                }
                HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                    Text(task.condition.reason)
                        .font(Typography.body(13))
                        .foregroundStyle(palette.textSecondary)
                        .accessibilityLabel(taskConditionAccessibilityLabel(task))
                    Spacer()
                    TaskActionCluster(
                        task: task,
                        isActing: activeControlId == "task:\(task.id)",
                        controlsDisabled: activeControlId != nil,
                        onAction: { action in
                            perform(action, on: WorkTaskSelection(wave: found.wave.wave, task: task))
                        },
                        onOpenWorktree: openWorktree
                    )
                }
                if let sessions, !sessions.isEmpty {
                    sectionHeading("Sessions · \(sessions.count)")
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(sessions) { session in
                            Button { onOpenSession(session) } label: {
                                HStack(spacing: Spacing.sm) {
                                    Image(systemName: "bubble.left").foregroundStyle(palette.textSecondary)
                                    Text(session.title).font(.system(size: 13, weight: .medium))
                                    Text(session.flowMembership.label)
                                        .font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
                                    Spacer()
                                    Text(session.state.rawValue.capitalized)
                                        .font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
                                }
                                .padding(.vertical, 6)
                                .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                            .accessibilityIdentifier("task-session-\(session.id)")
                        }
                    }
                } else if sessions != nil, model.sessions.value != nil, model.sessions.errorMessage == nil {
                    Text("No open Sessions.")
                        .font(Typography.body(12))
                        .foregroundStyle(palette.textSecondary)
                        .accessibilityIdentifier("workspace-no-sessions")
                }
                HStack {
                    sectionHeading("Description")
                    Spacer()
                    Button("Edit description") {
                        editingTask = WorkTaskSelection(wave: found.wave.wave, task: task)
                    }
                    .buttonStyle(.link)
                    .accessibilityIdentifier("workspace-edit-directive")
                }
                if task.task.description.isEmpty {
                    Text("No description recorded.").foregroundStyle(palette.textSecondary)
                        .accessibilityIdentifier("workspace-task-directive")
                } else {
                    MarkdownBlocks(source: task.task.description)
                        .accessibilityIdentifier("workspace-task-directive")
                }
            }
        } else {
            missingSelection
        }
    }

    private func sectionHeading(_ text: String) -> some View {
        Text(text)
            .font(Typography.caption(11).weight(.semibold))
            .tracking(0.8)
            .textCase(.uppercase)
            .foregroundStyle(palette.textSecondary)
            .padding(.top, Spacing.sm)
    }

    private func planRow(_ task: RoadmapTask) -> some View {
        Button {
            if let onOpenTask { onOpenTask(.task(id: task.id)) } else { model.select(.task(id: task.id)) }
        } label: {
            HStack(spacing: Spacing.sm) {
                Image(systemName: task.task.completed ? "checkmark.circle.fill" : "circle")
                    .foregroundStyle(task.task.completed ? Color.statusSuccess : palette.textSecondary)
                    .frame(width: 14)
                Text(task.task.name)
                    .font(.system(size: 13))
                    .foregroundStyle(palette.text)
                    .lineLimit(1)
                    .truncationMode(.tail)
                Spacer(minLength: Spacing.sm)
                Text(task.task.identifier)
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
            }
            .padding(.vertical, 5)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(task.task.name)
        .accessibilityIdentifier("podium-task-\(task.id)")
    }

    // MARK: - Shared pieces

    private var missingSelection: some View {
        ContentUnavailableView(
            "Selected Work is unavailable",
            systemImage: "questionmark.circle",
            description: Text("This planning read cannot resolve the selection. Its Sessions remain accessible in the work list.")
        )
    }

    private func surfaceHeader(_ title: String, subtitle: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xxs) {
            Text(title)
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(palette.text)
            Text(subtitle)
                .font(Typography.caption(11))
                .foregroundStyle(palette.textSecondary)
        }
    }

    private func pausedChip(_ waveId: String) -> some View {
        Text("paused")
            .font(Typography.caption(9).weight(.semibold))
            .foregroundStyle(WaveLensColor.blue.glow)
            .padding(.horizontal, Spacing.xs)
            .padding(.vertical, 1)
            .background(WaveLensColor.blue.glow.opacity(0.12))
            .clipShape(Capsule())
            .accessibilityIdentifier("wave-paused-\(waveId)")
    }

    private func evidenceBanner(title: String, detail: String) -> some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(Color.statusWarning)
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                Text(title)
                    .font(Typography.caption(11).weight(.semibold))
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
        .background(Color.statusWarning.opacity(0.12))
    }

    // MARK: - Controls

    private struct WorkTaskSelection: Identifiable {
        let wave: WaveSnapshot
        let task: RoadmapTask

        var id: String { "\(wave.id):\(task.id)" }
    }

    private enum TaskControl {
        case run
        case resume
    }

    private func perform(_ action: RoadmapTaskAction, on selection: WorkTaskSelection) {
        switch action {
        case .run:
            perform(TaskControl.run, on: selection)
        case .resume:
            perform(TaskControl.resume, on: selection)
        case .openPr:
            if let github = selection.task.activePr?.publication?.github {
                NSWorkspace.shared.open(github.url)
            }
        }
    }

    private func perform(_ control: TaskControl, on selection: WorkTaskSelection) {
        let controlId = "task:\(selection.task.id)"
        activeControlId = controlId
        controlError = nil
        Task {
            do {
                let repo = selection.wave.repo
                let issue = selection.task.task.identifier
                try await Task.detached(priority: .userInitiated) {
                    switch control {
                    case .run:
                        try LocalWaveAgentLauncher.runTask(repoPath: repo, issue: issue)
                    case .resume:
                        try LocalWaveAgentLauncher.resumeTask(repoPath: repo, issue: issue)
                    }
                }.value
                await model.refresh()
            } catch {
                controlError = error.localizedDescription
            }
            if activeControlId == controlId {
                activeControlId = nil
            }
        }
    }

    private func openWorktree(_ workspace: TaskWorkspaceSnapshot) {
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
    let model: PodiumModel
    let task: RoadmapTask
    let wave: WaveSnapshot

    @Environment(\.dismiss) private var dismiss
    @Environment(\.palette) private var palette
    @State private var draft: String
    @State private var isSaving = false
    @State private var saveError: String?

    init(model: PodiumModel, task: RoadmapTask, wave: WaveSnapshot) {
        self.model = model
        self.task = task
        self.wave = wave
        _draft = State(initialValue: task.task.description)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            Text("Edit description · \(task.task.identifier)")
                .font(.system(size: 14, weight: .semibold))
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
                    .foregroundStyle(Color.statusWarning)
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
                            try await model.updateTaskDirective(task: task, wave: wave, text: draft)
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
