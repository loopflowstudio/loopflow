#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

enum RoadmapTaskAction: Equatable {
    case run
    case openPr

    var label: String {
        switch self {
        case .run: "Start"
        case .openPr: "Open PR"
        }
    }
}

/// Pick the one contextual Task action from Rust's legal-action model. The
/// server decides what is legal and recommends one move; this maps that
/// recommendation onto the affordance the app can offer, and never re-derives
/// it from status.
func roadmapTaskAction(_ task: RoadmapTask) -> RoadmapTaskAction? {
    let startable = task.runControl.unavailable == nil
    guard task.runtime != nil else { return startable ? .run : nil }
    switch task.actions.recommended {
    case .resume:
        // Continuing a Task is a fresh Flow launch; Rust's Start legality governs it.
        if startable { return .run }
    case .openPr:
        if task.activePr?.publication?.github != nil { return .openPr }
    case .startNextPr, .noAction, .none:
        break
    }
    return nil
}

/// One query, two shapes. Both read the single `lf wave show` snapshot: NOW
/// re-shapes it into a flat, cross-wave, condition-grouped list; ROADMAP keeps
/// the Wave › Task tree.
enum WorkLens: String, CaseIterable, Identifiable {
    case now
    case roadmap

    var id: String { rawValue }
    var title: String {
        switch self {
        case .now: "Now"
        case .roadmap: "Roadmap"
        }
    }
}

/// A Task row is actionable when the user can click something on it — the one
/// contextual action, or Interrupt. When neither applies the row is not hidden;
/// it is greyed with its `next_move.reason` attached, so nothing silently
/// disappears (the OmniFocus failure mode). One hiding mechanism, never two.
func roadmapTaskIsActionable(_ task: RoadmapTask) -> Bool {
    roadmapTaskAction(task) != nil
}

/// Loopflow Desktop's shared Work surface: one machine-wide `lf wave show --json`
/// read, rendered without re-querying each Wave or inventing another work model.
struct RoadmapView: View {
    let onOpenWave: (WaveSnapshot) -> Void

    @Environment(\.palette) private var palette
    private let model: WorkModel
    @Binding private var selection: WorkReference?
    @State private var lens: WorkLens = .now
    @State private var controlError: String?
    @State private var activeControlId: String?

    /// Renders the window's model; that window keeps it current.
    init(
        model: WorkModel,
        onOpenWave: @escaping (WaveSnapshot) -> Void
    ) {
        self.model = model
        _selection = .constant(nil)
        self.onOpenWave = onOpenWave
    }

    private var repoPath: String? { model.repoPath }
    private var snapshot: RoadmapSnapshot? { model.roadmap.value }
    private var queryError: String? { model.roadmap.errorMessage }
    private var isRefreshing: Bool { model.isRefreshing }

    private var visibleWaves: [WaveRoadmap] {
        model.visibleRoadmaps
    }

    private var selectedWaveRoadmap: WaveRoadmap? {
        guard let selection, let waveId = model.waveId(for: selection) else { return nil }
        return model.wave(id: waveId)
    }

    private var selectedRosterWave: Wave? {
        guard let selection, selection.kind == .wave else { return nil }
        return model.rosterWave(id: selection.id)?.api
    }

    private var selectedTask: (
        wave: WaveRoadmap,
        task: RoadmapTask
    )? {
        guard let selection, selection.kind == .task else { return nil }
        return model.task(id: selection.id)
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            if repoPath != nil {
                PeerPlanningView(reading: model.peerPlanning)
                    .padding(.horizontal, Spacing.xl)
                    .padding(.bottom, Spacing.sm)
            }
            Divider()
            if let queryError {
                evidenceBanner(
                    title: snapshot == nil ? "Roadmap unavailable" : "Refresh failed — showing the last roadmap",
                    detail: queryError
                )
            }
            if let controlError {
                evidenceBanner(title: "Control failed", detail: controlError)
            }
            content
        }
        .background(palette.background)
    }

    private var header: some View {
        HStack(alignment: .center, spacing: Spacing.md) {
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                if selection != nil {
                    workBreadcrumb
                }
                Text(workTitle)
                    .font(Typography.sectionTitle(20))
                    .foregroundStyle(palette.text)
                Text(workSubtitle)
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(1)
            }
            Spacer()
            if selection == nil {
                Picker("Lens", selection: $lens) {
                    ForEach(WorkLens.allCases) { lens in
                        Text(lens.title).tag(lens)
                    }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .fixedSize()
                .accessibilityIdentifier("work-lens")
            }
            if isRefreshing {
                ProgressView()
                    .controlSize(.small)
            }
            Button {
                Task { await refresh() }
            } label: {
                Image(systemName: "arrow.clockwise")
            }
            .buttonStyle(.borderless)
            .disabled(isRefreshing)
            .help("Refresh work")
            .accessibilityLabel("Refresh work")
        }
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.md)
    }

    @ViewBuilder
    private var workBreadcrumb: some View {
        HStack(spacing: Spacing.xs) {
            Button("Work") { selection = nil }
                .buttonStyle(.plain)
            if let waveId = selectedWaveRoadmap?.wave.id ?? selectedRosterWave?.id,
               let waveName = selectedWaveRoadmap?.wave.name ?? selectedRosterWave?.name {
                breadcrumbSeparator
                Button(waveName) { selection = .wave(id: waveId) }
                    .buttonStyle(.plain)
                    .disabled(selection == .wave(id: waveId))
            }
            if let task = selectedTask?.task {
                breadcrumbSeparator
                Text(task.task.identifier)
            }
        }
        .font(Typography.caption(9).weight(.semibold))
        .foregroundStyle(palette.textSecondary)
        .accessibilityIdentifier("loopflow-work-breadcrumb")
    }

    private var breadcrumbSeparator: some View {
        Image(systemName: "chevron.right")
            .font(.system(size: 7, weight: .bold))
            .foregroundStyle(palette.textSecondary.opacity(0.7))
    }

    private var workTitle: String {
        switch selection {
        case nil:
            "Work"
        case .some(let work):
            switch work.kind {
            case .wave:
                selectedWaveRoadmap?.wave.name ?? selectedRosterWave?.name ?? "Wave"
            case .project:
                selectedWaveRoadmap?.wave.name ?? "Wave"
            case .task:
                selectedTask?.task.task.name ?? "Task"
            }
        }
    }

    private var workSubtitle: String {
        switch selection {
        case nil:
            repoPath == nil ? "All planned Work" : "Planned Work in this repository"
        case .some(let work):
            switch work.kind {
            case .wave:
                selectedWaveRoadmap?.wave.goal ?? "No readable plan for this Wave"
            case .project:
                selectedWaveRoadmap?.wave.goal ?? "Wave unavailable"
            case .task:
                selectedTask?.task.condition.reason ?? "Task unavailable"
            }
        }
    }

    @ViewBuilder
    private var content: some View {
        if snapshot == nil, queryError == nil {
            ProgressView(WorkReadingStatus.loading.message ?? "")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .accessibilityIdentifier("loopflow-work-loading")
        } else if snapshot == nil {
            ContentUnavailableView(
                "Work unavailable",
                systemImage: "exclamationmark.triangle",
                description: Text("Couldn't read the latest work. Refresh to try again.")
            )
            .accessibilityIdentifier("loopflow-work-unavailable")
        } else if visibleWaves.isEmpty {
            ContentUnavailableView(
                repoPath == nil ? "No planned Work yet" : "No planned Work in this repository",
                systemImage: "map",
                description: Text("Waves without readable Projects remain in the Waves sidebar.")
            )
            .accessibilityIdentifier("loopflow-work-empty")
        } else {
            if selection != nil {
                focusedContent
            } else {
                switch lens {
                case .now: nowContent
                case .roadmap: roadmapContent
                }
            }
        }
    }

    @ViewBuilder
    private var focusedContent: some View {
        switch selection?.kind {
        case .wave:
            if let roadmap = selectedWaveRoadmap {
                focusedScroll {
                    waveCard(roadmap)
                }
            } else {
                missingFocus("No planned Work for this Wave")
            }
        case .project:
            if let roadmap = selectedWaveRoadmap { focusedScroll { waveCard(roadmap) } }
            else { missingFocus("Project unavailable") }
        case .task:
            if let selectedTask {
                focusedScroll {
                    taskCard(selectedTask.task, wave: selectedTask.wave.wave)
                }
            } else {
                missingFocus("Task unavailable")
            }
        case nil:
            EmptyView()
        }
    }

    private func focusedScroll<Content: View>(
        @ViewBuilder content: () -> Content
    ) -> some View {
        ScrollView {
            content()
                .padding(Spacing.xl)
                .frame(maxWidth: 920, alignment: .leading)
                .frame(maxWidth: .infinity, alignment: .center)
        }
    }

    private func missingFocus(_ title: String) -> some View {
        ContentUnavailableView(
            title,
            systemImage: "map",
            description: Text("Return to Work or refresh the latest roadmap.")
        )
    }

    private var nowSectionsList: [NowSection] {
        nowSections(from: visibleWaves)
    }

    @ViewBuilder
    private var nowContent: some View {
        if nowSectionsList.isEmpty {
            ContentUnavailableView(
                "Nothing needs action",
                systemImage: "checkmark.circle",
                description: Text("No live or stopped work across these Waves. Switch to Roadmap for the full plan.")
            )
        } else {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: Spacing.lg) {
                    ForEach(nowSectionsList) { section in
                        NowSectionView(
                            section: section,
                            selection: selection,
                            activeControlId: activeControlId,
                            onSelect: { row in
                                selection = .task(id: row.task.id)
                            },
                            onTaskAction: { row, action in
                                perform(action, task: row.task, wave: row.wave)
                            },
                            onOpenWorktree: openWorktree
                        )
                    }
                }
                .padding(Spacing.xl)
                .frame(maxWidth: 920, alignment: .leading)
                .frame(maxWidth: .infinity, alignment: .center)
            }
            .accessibilityIdentifier("work-now")
        }
    }

    @ViewBuilder
    private var roadmapContent: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: Spacing.lg) {
                ForEach(visibleWaves, id: \.wave.id) { roadmap in
                    waveCard(roadmap)
                }
            }
            .padding(Spacing.xl)
            .frame(maxWidth: 920, alignment: .leading)
            .frame(maxWidth: .infinity, alignment: .center)
        }
        .accessibilityIdentifier("wave-roadmap")
    }

    private func waveCard(_ roadmap: WaveRoadmap) -> some View {
        RoadmapWaveCard(
            roadmap: roadmap,
            selection: selection,
            activeControlId: activeControlId,
            onSelect: { selected in selection = selected },
            onOpen: {
                selection = .wave(id: roadmap.wave.id)
                openWave(roadmap.wave)
            },
            onRefresh: { await refresh() },
            onError: { controlError = $0 },
            onTaskAction: { task, action in
                perform(action, task: task, wave: roadmap.wave)
            },
            onOpenWorktree: openWorktree
        )
    }

    private func taskCard(_ task: RoadmapTask, wave: WaveSnapshot) -> some View {
        RoadmapTaskRow(
            task: task,
            isSelected: true,
            activeControlId: activeControlId,
            onSelect: {},
            onAction: { action in
                perform(action, task: task, wave: wave)
            },
            onOpenWorktree: openWorktree
        )
        .padding(Spacing.lg)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
        .overlay {
            RoundedRectangle(cornerRadius: CornerRadius.lg)
                .stroke(palette.border, lineWidth: 1)
        }
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

    @MainActor
    private func refresh() async {
        await model.refresh()
    }

    /// Navigate to the Wave. Starting a stopped Wave remains the Machine control's
    /// job; this only opens the detail.
    private func openWave(_ wave: WaveSnapshot) {
        onOpenWave(wave)
    }

    private func perform(_ action: RoadmapTaskAction, task: RoadmapTask, wave: WaveSnapshot) {
        switch action {
        case .openPr:
            if let github = task.activePr?.publication?.github {
                NSWorkspace.shared.open(github.url)
            }
        case .run:
            let controlId = "task:\(task.id)"
            activeControlId = controlId
            controlError = nil
            Task {
                do {
                    let repo = wave.repo
                    let issue = task.task.identifier
                    try await Task.detached(priority: .userInitiated) {
                        try LocalWaveAgentLauncher.runTask(repoPath: repo, issue: issue)
                    }.value
                    await refresh()
                } catch {
                    controlError = error.localizedDescription
                }
                if activeControlId == controlId {
                    activeControlId = nil
                }
            }
        }
    }

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

private struct RoadmapWaveCard: View {
    let roadmap: WaveRoadmap
    let selection: WorkReference?
    let activeControlId: String?
    let onSelect: (WorkReference) -> Void
    let onOpen: () -> Void
    let onRefresh: () async -> Void
    let onError: (String) -> Void
    let onTaskAction: (RoadmapTask, RoadmapTaskAction) -> Void
    let onOpenWorktree: (TaskWorktreeSnapshot) -> Void

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            HStack(alignment: .top, spacing: Spacing.md) {
                VStack(alignment: .leading, spacing: Spacing.xxs) {
                    HStack(spacing: Spacing.sm) {
                        Text(roadmap.wave.name)
                            .font(Typography.sectionTitle(18))
                            .foregroundStyle(palette.text)
                        Text(roadmap.wave.status.label)
                            .font(Typography.caption(10))
                            .foregroundStyle(palette.textSecondary)

                    }
                    if !roadmap.wave.goal.isEmpty {
                        Text(roadmap.wave.goal)
                            .font(Typography.body(12))
                            .foregroundStyle(palette.textSecondary)
                            .lineLimit(2)
                    }
                }
                .contentShape(Rectangle())
                .onTapGesture { onSelect(.wave(id: roadmap.wave.id)) }
                .accessibilityAddTraits(
                    selection == .wave(id: roadmap.wave.id) ? [.isSelected] : []
                )
                Spacer()
                Text(roadmap.wave.machine.map { $0.route == "local" ? $0.id : $0.route } ?? "Unplaced")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
            }

            if let chapter = roadmap.currentProject { WaveProjectView(project: chapter) }
            switch roadmap.tasks {
            case .unavailable(let reason):
                Label(reason, systemImage: "exclamationmark.triangle").foregroundStyle(Color.statusWarning)
            case .available(let tasks, let truncated):
                if tasks.isEmpty { Text("No Tasks in this Project.").foregroundStyle(palette.textSecondary) }
                ForEach(tasks) { task in
                    RoadmapTaskRow(task: task, isSelected: selection == .task(id: task.id),
                        activeControlId: activeControlId, onSelect: { onSelect(.task(id: task.id)) },
                        onAction: { action in onTaskAction(task, action) }, onOpenWorktree: onOpenWorktree)
                }
                if truncated { Text("Older Tasks are truncated.").foregroundStyle(Color.statusWarning) }
            }
            ForEach(roadmap.unavailableTasks, id: \.taskId) { task in
                Text("\(task.taskIdentifier): \(task.reason) · \(task.recovery)").foregroundStyle(Color.statusWarning)
            }
        }
        .padding(Spacing.lg)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
        .overlay {
            RoundedRectangle(cornerRadius: CornerRadius.lg)
                .stroke(
                    selection == .wave(id: roadmap.wave.id)
                        ? Color.loopflowBurgundy : palette.border,
                    lineWidth: selection == .wave(id: roadmap.wave.id) ? 2 : 1
                )
        }
        .accessibilityIdentifier("loopflow-wave-\(roadmap.wave.id)")
    }
}

struct RoadmapTaskRow: View {
    let task: RoadmapTask
    let isSelected: Bool
    let activeControlId: String?
    let onSelect: () -> Void
    let onAction: (RoadmapTaskAction) -> Void
    let onOpenWorktree: (TaskWorktreeSnapshot) -> Void

    @Environment(\.palette) private var palette

    private var isActing: Bool { activeControlId == "task:\(task.id)" }

    var body: some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            Image(systemName: task.task.isSuccessful ? "checkmark.circle.fill" : "circle")
                .font(Typography.caption(11))
                .foregroundStyle(task.task.isSuccessful ? Color.statusSuccess : task.section.color)
                .frame(width: 14)
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                    if let issueURL = task.reference.issueUrl {
                        Link(task.task.identifier, destination: issueURL)
                            .font(Typography.caption(10).weight(.semibold))
                    } else {
                        Text(task.task.identifier)
                            .font(Typography.caption(10).weight(.semibold))
                            .foregroundStyle(palette.textSecondary)
                    }
                    Text(task.task.name)
                        .font(Typography.caption(12))
                        .foregroundStyle(palette.text)
                        .lineLimit(2)
                    Spacer()
                    Text(task.section.label)
                        .font(Typography.caption(9).weight(.semibold))
                        .foregroundStyle(task.section.color)
                }
                Text(task.condition.reason)
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(2)
                    .accessibilityLabel(taskConditionAccessibilityLabel(task))
                WorkChannelChips(task: task)
                TaskActionCluster(
                    task: task,
                    isActing: isActing,
                    controlsDisabled: activeControlId != nil,
                    onAction: onAction,
                    onOpenWorktree: onOpenWorktree
                )
            }
        }
        .padding(Spacing.sm)
        .background(
            isSelected ? Color.loopflowBurgundy.opacity(0.1) : palette.background.opacity(0.6)
        )
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
        .contentShape(Rectangle())
        .onTapGesture { onSelect() }
        .accessibilityAddTraits(isSelected ? [.isSelected] : [])
        .accessibilityIdentifier("loopflow-task-\(task.id)")
        .opacity(roadmapTaskIsActionable(task) ? 1 : 0.55)
    }
}

/// The shared Task condition plus its orthogonal planning facts. `runtime == nil`
/// means Work has not started.
struct WorkChannelChips: View {
    let task: RoadmapTask
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.xs) {
            channel("Condition", task.condition.state.rawValue, task.condition.state.color)
            channel("PM", task.task.terminalLabel ?? "open",
                    task.task.isSuccessful ? Color.statusSuccess : palette.textSecondary)
            if let runtime = task.runtime {
                channel("Status", runtime.status.label, statusColor(runtime.status))
            } else {
                channel("Work", "none", palette.textSecondary)
            }
        }
    }

    private func channel(_ label: String, _ value: String, _ color: Color) -> some View {
        HStack(spacing: 2) {
            Text(label).foregroundStyle(palette.textSecondary)
            Text(value).foregroundStyle(color)
        }
        .font(Typography.caption(9))
        .padding(.horizontal, Spacing.xs)
        .padding(.vertical, 1)
        .background(palette.surfaceMuted.opacity(0.5))
        .clipShape(Capsule())
    }

    private func statusColor(_ status: TaskState) -> Color {
        status.isTerminal ? .statusNeutral : .statusInfo
    }
}

/// The one contextual action plus the always-available Worktree/PR affordances,
/// shared by the ROADMAP tree and the NOW list so both drive the exact same
/// audited lifecycle verbs.
struct TaskActionCluster: View {
    let task: RoadmapTask
    let isActing: Bool
    let controlsDisabled: Bool
    let onAction: (RoadmapTaskAction) -> Void
    let onOpenWorktree: (TaskWorktreeSnapshot) -> Void

    var body: some View {
        HStack(spacing: Spacing.xs) {
            if let action = roadmapTaskAction(task) {
                Button(action.label) { onAction(action) }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    .disabled(controlsDisabled)
            }
            if let workspace = task.reference.workspace {
                Button("Worktree") { onOpenWorktree(workspace) }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
            }
            if let github = task.activePr?.publication?.github {
                Link("PR #\(github.number)", destination: github.url)
                    .font(Typography.caption(10))
            }
            if isActing {
                ProgressView().controlSize(.small)
            }
        }
    }
}

struct NowSectionView: View {
    let section: NowSection
    let selection: WorkReference?
    let activeControlId: String?
    let onSelect: (NowRow) -> Void
    let onTaskAction: (NowRow, RoadmapTaskAction) -> Void
    let onOpenWorktree: (TaskWorktreeSnapshot) -> Void

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.sm) {
                Text(section.group.title)
                    .font(Typography.sectionTitle(15))
                    .foregroundStyle(nowColor(section.group))
                Text("\(section.rows.count)")
                    .font(Typography.caption(10).weight(.semibold))
                    .foregroundStyle(palette.textSecondary)
                    .padding(.horizontal, Spacing.xs)
                    .padding(.vertical, 1)
                    .background(palette.surfaceMuted.opacity(0.6))
                    .clipShape(Capsule())
                Spacer()
            }
            ForEach(section.rows) { row in
                NowRowView(
                    row: row,
                    isSelected: selection == .task(id: row.task.id),
                    activeControlId: activeControlId,
                    onSelect: { onSelect(row) },
                    onAction: { action in onTaskAction(row, action) },
                    onOpenWorktree: onOpenWorktree
                )
            }
        }
        .accessibilityIdentifier("loopflow-now-\(section.group.rawValue)")
    }

    private func nowColor(_ group: TaskConditionState) -> Color {
        switch group {
        case .waiting: WaveLensColor.blue.glow
        case .blocked: .statusError
        case .clear: .statusNeutral
        case .unknown: .statusWarning
        }
    }
}

private struct NowRowView: View {
    let row: NowRow
    let isSelected: Bool
    let activeControlId: String?
    let onSelect: () -> Void
    let onAction: (RoadmapTaskAction) -> Void
    let onOpenWorktree: (TaskWorktreeSnapshot) -> Void

    @Environment(\.palette) private var palette

    private var task: RoadmapTask { row.task }
    private var isActing: Bool { activeControlId == "task:\(task.id)" }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xxs) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                if let issueURL = task.reference.issueUrl {
                    Link(task.task.identifier, destination: issueURL)
                        .font(Typography.caption(10).weight(.semibold))
                } else {
                    Text(task.task.identifier)
                        .font(Typography.caption(10).weight(.semibold))
                        .foregroundStyle(palette.textSecondary)
                }
                Text(task.task.name)
                    .font(Typography.caption(12))
                    .foregroundStyle(palette.text)
                    .lineLimit(2)
                Spacer()
                Text(row.wave.name)
                    .font(Typography.caption(9))
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(1)
            }
            Text(task.condition.reason)
                .font(Typography.caption(10))
                .foregroundStyle(palette.textSecondary)
                .lineLimit(2)
                .accessibilityLabel(taskConditionAccessibilityLabel(task))
            WorkChannelChips(task: task)
            TaskActionCluster(
                task: task,
                isActing: isActing,
                controlsDisabled: activeControlId != nil,
                onAction: onAction,
                onOpenWorktree: onOpenWorktree
            )
        }
        .padding(Spacing.sm)
        .background(isSelected ? Color.loopflowBurgundy.opacity(0.1) : palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
        .overlay {
            RoundedRectangle(cornerRadius: CornerRadius.sm)
                .stroke(palette.border, lineWidth: 1)
        }
        .contentShape(Rectangle())
        .onTapGesture { onSelect() }
        .accessibilityAddTraits(isSelected ? [.isSelected] : [])
        .accessibilityIdentifier("loopflow-task-\(task.id)")
        .opacity(roadmapTaskIsActionable(task) ? 1 : 0.55)
    }
}

extension RoadmapSection {
    var label: String {
        switch self {
        case .now: "Now"
        case .waiting: "Waiting"
        case .available: "Available"
        case .later: "Later"
        }
    }

    var color: Color {
        switch self {
        case .now: .statusSuccess
        case .waiting: .statusWarning
        case .available: .statusInfo
        case .later: .statusNeutral
        }
    }
}

private extension TaskConditionState {
    var color: Color {
        switch self {
        case .blocked: .statusError
        case .waiting: WaveLensColor.blue.glow
        case .clear: .statusNeutral
        case .unknown: .statusWarning
        }
    }
}
#endif
