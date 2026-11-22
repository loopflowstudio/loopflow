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
            .buttonStyle(LoopflowButtonStyle())
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
            ContentUnavailableView(
                repoPath == nil ? "No planned Work yet" : "No planned Work in this repository",
                systemImage: "map",
                description: Text("Waves without readable Projects remain in the Waves sidebar.")
            )
            .accessibilityIdentifier("loopflow-work-empty")
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
                                model.perform(action, task: row.task)
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
                ForEach(Array(visibleWaves.enumerated()), id: \.element.wave.id) { index, roadmap in
                    if index > 0 { Divider() }
                    waveSection(roadmap)
                }
            }
            .padding(Spacing.xl)
            .frame(maxWidth: 920, alignment: .leading)
            .frame(maxWidth: .infinity, alignment: .center)
        }
        .accessibilityIdentifier("wave-roadmap")
    }

    private func waveSection(_ roadmap: WaveRoadmap) -> some View {
        RoadmapWaveSection(
            roadmap: roadmap,
            onOpenWave: { onOpenWave(roadmap.wave) },
            onOpenTask: { onOpenTask(roadmap.wave.repo, $0) },
            onTaskAction: { task, action in
                model.perform(action, task: task)
            },
            onOpenWorktree: model.openWorktree
        )
    }

    @MainActor
    private func refresh() async {
        await model.refresh()
    }
}

extension WorkModel {
    /// A Portfolio row's one contextual action.
    func perform(_ action: RoadmapTaskAction, task: RoadmapTask) {
        switch action {
        case .openPr:
            if let github = task.pr?.publication?.github {
                NSWorkspace.shared.open(github.url)
            }
        case .run:
            startTask(nil, task: task)
        }
    }
}

/// One Wave on the roadmap: a section under its name, with no frame of its own.
struct RoadmapWaveSection: View {
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
                    PortfolioTaskRow(task: task, onOpen: { onOpenTask(task.id) },
                        onAction: { action in onTaskAction(task, action) }, onOpenWorktree: onOpenWorktree)
                }
                if truncated { Text("Older Tasks are truncated.").foregroundStyle(palette.warning) }
            }
            ForEach(roadmap.unavailableTasks, id: \.taskId) { task in
                Text("\(task.taskIdentifier): \(task.reason) · \(task.recovery)").foregroundStyle(palette.warning)
            }
        }
        .accessibilityIdentifier("loopflow-wave-\(roadmap.wave.id)")
    }
}

/// A Task on a Portfolio screen: its row, which opens it, with its actions beside it.
struct PortfolioTaskRow: View {
    let task: RoadmapTask
    /// The Task's Wave, where the list crosses Waves.
    var context: String?
    let onOpen: () -> Void
    let onAction: (RoadmapTaskAction) -> Void
    let onOpenWorktree: (TaskWorktreeSnapshot) -> Void

    var body: some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            TaskRow(task: task, context: context).opening(onOpen)
            TaskActionCluster(task: task, onAction: onAction, onOpenWorktree: onOpenWorktree)
                .padding(.top, Spacing.sm)
        }
        .accessibilityIdentifier("loopflow-task-\(task.id)")
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
            }
            if let workspace = task.reference.workspace {
                Button("Worktree") { onOpenWorktree(workspace) }
            }
            if let github = task.pr?.publication?.github {
                Link("PR #\(github.number)", destination: github.url)
                    .font(Typography.meta)
            }
        }
        .buttonStyle(LoopflowButtonStyle())
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
                    .foregroundStyle(palette.text)
                Text("\(section.rows.count)")
                    .font(Typography.meta.weight(.semibold))
                    .foregroundStyle(palette.textSecondary)
                    .padding(.horizontal, Spacing.xs)
                    .padding(.vertical, Spacing.xxs)
                    .background(palette.surfaceMuted.opacity(0.6))
                    .clipShape(Capsule())
                Spacer()
            }
            ForEach(section.rows) { row in
                PortfolioTaskRow(
                    task: row.task,
                    context: row.wave?.name ?? "No Wave",
                    onOpen: { onSelect(row) },
                    onAction: { action in onTaskAction(row, action) },
                    onOpenWorktree: onOpenWorktree
                )
            }
        }
        .accessibilityIdentifier("loopflow-now-\(section.group.rawValue)")
    }
}
#endif
