// Work inspectors consume the same planning reading as the compact navigator.

#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

struct WorkSurfaceView: View {
    @Bindable var model: PodiumModel

    @Environment(\.palette) private var palette
    @State private var controlError: String?
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
                VStack(alignment: .leading, spacing: 4) {
                    if let snapshot {
                        Text("Planning snapshot: \(snapshot.generatedAt)")
                            .help("Snapshot generation time; not a fresh provider sync.")
                    }
                    if let selection = model.selection, model.sessions.errorMessage == nil,
                       let records = model.sessions.value {
                        let workspace = model.workspace
                        if !records.contains(where: { workspace.subject(for: $0.id) == selection }) {
                            Text("No open Sessions for this Work.")
                                .accessibilityIdentifier("workspace-no-sessions")
                        }
                    }
                }
                .font(.system(size: 12))
                .foregroundStyle(palette.textSecondary)
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
                HStack(alignment: .top, spacing: Spacing.md) {
                    VStack(alignment: .leading, spacing: Spacing.xxs) {
                        surfaceHeader(roadmap.wave.name, subtitle: roadmap.wave.status.label)
                        if roadmap.wave.paused {
                            pausedChip(roadmap.wave.id)
                        }
                        if !roadmap.wave.goal.isEmpty {
                            Text(roadmap.wave.goal)
                                .font(Typography.body(13))
                                .foregroundStyle(palette.textSecondary)
                        }
                    }
                    Spacer()
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

                if let chapter = roadmap.chapter {
                    WaveChapterView(chapter: chapter)
                }
                Button("Chapter history") {
                    model.historyReference = nil
                    model.historyWave = roadmap.wave
                }
                switch roadmap.tasks {
                case .unavailable(let reason):
                    Text(reason).foregroundStyle(Color.statusWarning)
                case .available(let tasks, _):
                    if tasks.isEmpty { Text("No Tasks in this chapter.").foregroundStyle(palette.textSecondary) }
                    ForEach(tasks) { task in
                        RoadmapTaskRow(
                            task: task, isSelected: false, activeControlId: activeControlId,
                            onSelect: { model.select(.task(id: task.id)) },
                            onAction: { action in perform(action, on: WorkTaskSelection(wave: roadmap.wave, task: task)) },
                            onOpenWorktree: openWorktree
                        )
                    }
                }
                WaveMetricPortfolioView(portfolio: roadmap.metricPortfolio)
                ForEach(roadmap.unavailableTasks, id: \.taskId) { task in
                    Text("\(task.taskIdentifier): \(task.reason) · \(task.recovery)")
                        .foregroundStyle(Color.statusWarning)
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
            scrollingDetail(identifier: "podium-detail-task") {
                VStack(alignment: .leading, spacing: Spacing.sm) {
                    Text(found.wave.wave.name)
                        .font(Typography.caption(10).weight(.semibold))
                        .tracking(0.8)
                        .textCase(.uppercase)
                        .foregroundStyle(palette.textSecondary)
                    HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                        if let issueURL = task.reference.issueUrl {
                            Link(task.task.identifier, destination: issueURL)
                                .font(Typography.code(12).weight(.semibold))
                        } else {
                            Text(task.task.identifier)
                                .font(Typography.code(12).weight(.semibold))
                                .foregroundStyle(palette.textSecondary)
                        }
                        Text(task.task.name)
                            .font(.system(size: 14, weight: .semibold))
                            .foregroundStyle(palette.text)
                        Spacer()
                        Text(task.section.label)
                            .font(Typography.caption(10).weight(.semibold))
                            .foregroundStyle(task.section.color)
                    }
                    Text(task.condition.reason)
                        .font(Typography.body(13))
                        .foregroundStyle(palette.textSecondary)
                        .accessibilityLabel(taskConditionAccessibilityLabel(task))
                    WorkChannelChips(task: task)
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
                .padding(Spacing.lg)
                .background(palette.surface)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
                .overlay {
                    RoundedRectangle(cornerRadius: CornerRadius.lg)
                        .stroke(palette.border, lineWidth: 1)
                }
                HStack {
                    Text("Task directive").font(.system(size: 14, weight: .semibold))
                    Spacer()
                    Button("Edit directive") {
                        editingTask = WorkTaskSelection(wave: found.wave.wave, task: task)
                    }
                    .accessibilityIdentifier("workspace-edit-directive")
                }
                Text(task.task.description.isEmpty ? "No directive recorded." : task.task.description)
                    .font(Typography.body(13))
                    .textSelection(.enabled)
                    .accessibilityIdentifier("workspace-task-directive")
                if let workspace = task.reference.workspace {
                    Text(workspace.worktree)
                        .font(Typography.code(11)).textSelection(.enabled)
                }
                if let chapter = found.wave.chapter { WaveChapterView(chapter: chapter) }
            }
        } else {
            missingSelection
        }
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

    private func sectionBadge(_ section: RoadmapSection) -> some View {
        Text(section.label)
            .font(Typography.caption(9).weight(.semibold))
            .foregroundStyle(section.color)
            .padding(.horizontal, Spacing.xs)
            .padding(.vertical, 2)
            .background(section.color.opacity(0.12))
            .clipShape(Capsule())
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
            Text("Edit directive · \(task.task.identifier)")
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
                Button("Save directive") {
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
