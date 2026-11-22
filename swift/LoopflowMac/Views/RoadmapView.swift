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
    case .run:
        // Continuing a Task is a fresh Graph launch; Rust's Start legality governs it.
        if startable { return .run }
    case .openPr:
        if task.pr?.publication?.github != nil { return .openPr }
    case .noAction, .none:
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

/// Loopflow Desktop's shared Work surface: one machine-wide `lf wave show --json`
/// read, rendered without re-querying each Wave or inventing another work model.
struct RoadmapView: View {
    let onOpenWave: (WaveSnapshot) -> Void
    let onOpenTask: (String, String) -> Void

    @Environment(\.palette) private var palette
    private let model: WorkModel
    @State private var lens: WorkLens = .now

    /// Renders the window's model; that window keeps it current.
    init(
        model: WorkModel,
        onOpenWave: @escaping (WaveSnapshot) -> Void,
        onOpenTask: @escaping (String, String) -> Void
    ) {
        self.model = model
        self.onOpenWave = onOpenWave
        self.onOpenTask = onOpenTask
    }

    private var repoPath: String? { model.repoPath }
    private var snapshot: RoadmapSnapshot? { model.roadmap.value }
    private var isRefreshing: Bool { model.isRefreshing }

    private var visibleWaves: [WaveRoadmap] {
        model.visibleRoadmaps
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            if repoPath != nil {
                RepositoryPlanView(reading: model.repositoryPlan)
                    .padding(.horizontal, Spacing.xl)
                    .padding(.bottom, Spacing.sm)
            }
            Divider()
            content
        }
        .background(palette.background)
    }

    private var header: some View {
        HStack(alignment: .center, spacing: Spacing.md) {
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                Text("Work")
                    .font(Typography.lede)
                    .foregroundStyle(palette.text)
                Text(repoPath == nil ? "All planned Work" : "Planned Work in this repository")
                    .font(Typography.meta)
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(1)
            }
            Spacer()
            Picker("Lens", selection: $lens) {
                ForEach(WorkLens.allCases) { lens in
                    Text(lens.title).tag(lens)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .fixedSize()
            .accessibilityIdentifier("work-lens")
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
    private var content: some View {
        if snapshot == nil, model.roadmap.isLoading {
            ProgressView(WorkReadingStatus.loading.message ?? "")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .accessibilityIdentifier("loopflow-work-loading")
        } else if snapshot == nil {
            Color.clear
        } else if visibleWaves.isEmpty {
            NoPlannedWork(model: model)
        } else {
            switch lens {
            case .now: nowContent
            case .roadmap: roadmapContent
            }
        }
    }

    private var nowSectionsList: [NowSection] {
        nowSections(from: model.visibleTasks, waves: visibleWaves)
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
                            onSelect: { row in
                                onOpenTask(row.task.task.repo, row.task.id)
                            },
                            onTaskAction: { row, action in
                                perform(action, task: row.task)
                            },
                            onOpenWorktree: model.openWorktree
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
            onOpenWave: { onOpenWave(roadmap.wave) },
            onOpenTask: { onOpenTask(roadmap.wave.repo, $0) },
            onTaskAction: { task, action in
                perform(action, task: task)
            },
            onOpenWorktree: model.openWorktree
        )
    }

    @MainActor
    private func refresh() async {
        await model.refresh()
    }

    private func perform(_ action: RoadmapTaskAction, task: RoadmapTask) {
        switch action {
        case .openPr:
            if let github = task.pr?.publication?.github {
                NSWorkspace.shared.open(github.url)
            }
        case .run:
            model.startTask(nil, task: task)
        }
    }
}

private struct RoadmapWaveCard: View {
    let roadmap: WaveRoadmap
    let onOpenWave: () -> Void
    let onOpenTask: (String) -> Void
    let onTaskAction: (RoadmapTask, RoadmapTaskAction) -> Void
    let onOpenWorktree: (TaskWorktreeSnapshot) -> Void

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            HStack(alignment: .top, spacing: Spacing.md) {
                VStack(alignment: .leading, spacing: Spacing.xxs) {
                    HStack(spacing: Spacing.sm) {
                        Text(roadmap.wave.name)
                            .font(Typography.lede)
                            .foregroundStyle(palette.text)
                        Text(roadmap.wave.status.label)
                            .font(Typography.meta)
                            .foregroundStyle(palette.textSecondary)

                    }
                    if !roadmap.wave.goal.isEmpty {
                        Text(roadmap.wave.goal)
                            .font(Typography.text)
                            .foregroundStyle(palette.textSecondary)
                            .lineLimit(2)
                    }
                }
                .contentShape(Rectangle())
                .onTapGesture(perform: onOpenWave)
                Spacer()
                Text(roadmap.wave.machine.map { $0.route == "local" ? $0.id : $0.route } ?? "Unplaced")
                    .font(Typography.meta)
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
            }

            if let project = roadmap.currentProject { WaveProjectView(project: project, repo: roadmap.wave.repo) }
            switch roadmap.tasks {
            case .unavailable(let reason):
                Label(reason, systemImage: "exclamationmark.triangle").foregroundStyle(palette.warning)
            case .available(let tasks, let truncated):
                if tasks.isEmpty { Text("No Tasks in this Project.").foregroundStyle(palette.textSecondary) }
                ForEach(tasks) { task in
                    RoadmapTaskRow(task: task, onSelect: { onOpenTask(task.id) },
                        onAction: { action in onTaskAction(task, action) }, onOpenWorktree: onOpenWorktree)
                }
                if truncated { Text("Older Tasks are truncated.").foregroundStyle(palette.warning) }
            }
            ForEach(roadmap.unavailableTasks, id: \.taskId) { task in
                Text("\(task.taskIdentifier): \(task.reason) · \(task.recovery)").foregroundStyle(palette.warning)
            }
        }
        .padding(Spacing.lg)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.pane))
        .overlay {
            RoundedRectangle(cornerRadius: CornerRadius.pane)
                .stroke(palette.border, lineWidth: 1)
        }
        .accessibilityIdentifier("loopflow-wave-\(roadmap.wave.id)")
    }
}

struct RoadmapTaskRow: View {
    let task: RoadmapTask
    let onSelect: () -> Void
    let onAction: (RoadmapTaskAction) -> Void
    let onOpenWorktree: (TaskWorktreeSnapshot) -> Void

    @Environment(\.palette) private var palette

    var body: some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            Image(systemName: task.lifecycle == .done ? "checkmark.circle.fill" : "circle")
                .font(Typography.meta)
                .foregroundStyle(task.lifecycle == .done ? palette.success : task.section.color(in: palette))
                .frame(width: 14)
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                    if let issueURL = task.reference.issueUrl {
                        Link(task.task.identifier, destination: issueURL)
                            .font(Typography.meta.weight(.semibold))
                    } else {
                        Text(task.task.identifier)
                            .font(Typography.meta.weight(.semibold))
                            .foregroundStyle(palette.textSecondary)
                    }
                    Text(task.task.name)
                        .font(Typography.text)
                        .foregroundStyle(palette.text)
                        .lineLimit(2)
                    Spacer()
                    Text(task.section.label)
                        .font(Typography.meta.weight(.semibold))
                        .foregroundStyle(task.section.color(in: palette))
                }
                Text(task.condition.reason)
                    .font(Typography.meta)
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(2)
                    .accessibilityLabel(taskConditionAccessibilityLabel(task))
                WorkChannelChips(task: task)
                TaskActionCluster(
                    task: task,
                    onAction: onAction,
                    onOpenWorktree: onOpenWorktree
                )
            }
        }
        .padding(Spacing.sm)
        .background(palette.background)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.control))
        .contentShape(Rectangle())
        .onTapGesture { onSelect() }
        .accessibilityIdentifier("loopflow-task-\(task.id)")
    }
}

/// The shared Task condition, lifecycle and orthogonal planning facts.
struct WorkChannelChips: View {
    let task: RoadmapTask
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.xs) {
            channel("Condition", task.condition.state.rawValue, task.condition.state.color(in: palette))
            channel("PM", task.task.terminalLabel ?? "open",
                    task.task.isSuccessful ? palette.success : palette.textSecondary)
            channel("Status", task.lifecycle.label, statusColor(task.lifecycle))
        }
    }

    private func channel(_ label: String, _ value: String, _ color: Color) -> some View {
        HStack(spacing: 2) {
            Text(label).foregroundStyle(palette.textSecondary)
            Text(value).foregroundStyle(color)
        }
        .font(Typography.meta)
        .padding(.horizontal, Spacing.xs)
        .padding(.vertical, 1)
        .background(palette.surfaceMuted.opacity(0.5))
        .clipShape(Capsule())
    }

    private func statusColor(_ status: TaskState) -> Color {
        status.isTerminal ? palette.textTertiary : palette.working
    }
}

/// The one contextual action plus the always-available Worktree/PR affordances,
/// shared by the ROADMAP tree and the NOW list so both drive the exact same
/// audited lifecycle verbs.
struct TaskActionCluster: View {
    let task: RoadmapTask
    let onAction: (RoadmapTaskAction) -> Void
    let onOpenWorktree: (TaskWorktreeSnapshot) -> Void

    var body: some View {
        HStack(spacing: Spacing.xs) {
            if let action = roadmapTaskAction(task) {
                Button(action.label) { onAction(action) }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
            }
            if let workspace = task.reference.workspace {
                Button("Worktree") { onOpenWorktree(workspace) }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
            }
            if let github = task.pr?.publication?.github {
                Link("PR #\(github.number)", destination: github.url)
                    .font(Typography.meta)
            }
        }
    }
}

struct NowSectionView: View {
    let section: NowSection
    let onSelect: (NowRow) -> Void
    let onTaskAction: (NowRow, RoadmapTaskAction) -> Void
    let onOpenWorktree: (TaskWorktreeSnapshot) -> Void

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.sm) {
                Text(section.group.title)
                    .font(Typography.lede)
                    .foregroundStyle(section.group.color(in: palette))
                Text("\(section.rows.count)")
                    .font(Typography.meta.weight(.semibold))
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
                    onSelect: { onSelect(row) },
                    onAction: { action in onTaskAction(row, action) },
                    onOpenWorktree: onOpenWorktree
                )
            }
        }
        .accessibilityIdentifier("loopflow-now-\(section.group.rawValue)")
    }
}

private struct NowRowView: View {
    let row: NowRow
    let onSelect: () -> Void
    let onAction: (RoadmapTaskAction) -> Void
    let onOpenWorktree: (TaskWorktreeSnapshot) -> Void

    @Environment(\.palette) private var palette

    private var task: RoadmapTask { row.task }
    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xxs) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                if let issueURL = task.reference.issueUrl {
                    Link(task.task.identifier, destination: issueURL)
                        .font(Typography.meta.weight(.semibold))
                } else {
                    Text(task.task.identifier)
                        .font(Typography.meta.weight(.semibold))
                        .foregroundStyle(palette.textSecondary)
                }
                Text(task.task.name)
                    .font(Typography.text)
                    .foregroundStyle(palette.text)
                    .lineLimit(2)
                Spacer()
                Text(row.wave?.name ?? "No Wave")
                    .font(Typography.meta)
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(1)
            }
            Text(task.condition.reason)
                .font(Typography.meta)
                .foregroundStyle(palette.textSecondary)
                .lineLimit(2)
                .accessibilityLabel(taskConditionAccessibilityLabel(task))
            WorkChannelChips(task: task)
            TaskActionCluster(
                task: task,
                onAction: onAction,
                onOpenWorktree: onOpenWorktree
            )
        }
        .padding(Spacing.sm)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.control))
        .overlay {
            RoundedRectangle(cornerRadius: CornerRadius.control)
                .stroke(palette.border, lineWidth: 1)
        }
        .contentShape(Rectangle())
        .onTapGesture { onSelect() }
        .accessibilityIdentifier("loopflow-task-\(task.id)")
    }
}

extension TaskSection {
    var label: String { rawValue.capitalized }

    func color(in palette: LoopflowPalette) -> Color {
        switch self {
        case .working: palette.accent
        case .backlog, .done, .abandoned: palette.textTertiary
        }
    }
}

private extension TaskConditionState {
    func color(in palette: LoopflowPalette) -> Color {
        switch self {
        case .blocked, .waiting: palette.warning
        case .clear, .unknown: palette.textTertiary
        }
    }
}
#endif
