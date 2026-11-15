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
    @State private var controlError: String?
    @State private var activeControlId: String?

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
    private var queryError: String? { model.roadmap.errorMessage }
    private var isRefreshing: Bool { model.isRefreshing }

    private var visibleWaves: [WaveRoadmap] {
        model.visibleRoadmaps
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
                Text("Work")
                    .font(Typography.sectionTitle(20))
                    .foregroundStyle(palette.text)
                Text(repoPath == nil ? "All planned Work" : "Planned Work in this repository")
                    .font(Typography.caption(11))
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
                            activeControlId: activeControlId,
                            onSelect: { row in
                                onOpenTask(row.task.task.repo, row.task.id)
                            },
                            onTaskAction: { row, action in
                                perform(action, task: row.task)
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
            activeControlId: activeControlId,
            onOpenWave: { onOpenWave(roadmap.wave) },
            onOpenTask: { onOpenTask(roadmap.wave.repo, $0) },
            onTaskAction: { task, action in
                perform(action, task: task)
            },
            onOpenWorktree: openWorktree
        )
    }

    private func evidenceBanner(title: String, detail: String) -> some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(palette.warning)
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
        .background(palette.warning.opacity(0.12))
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
            let controlId = "task:\(task.id)"
            activeControlId = controlId
            controlError = nil
            Task {
                do {
                    let repo = task.task.repo
                    let issue = task.task.identifier
                    try await LocalWaveAgentLauncher.runTask(repoPath: repo, issue: issue)
                    try await model.confirmWorkUpdate()
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
    let activeControlId: String?
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
                .onTapGesture(perform: onOpenWave)
                Spacer()
                Text(roadmap.wave.machine.map { $0.route == "local" ? $0.id : $0.route } ?? "Unplaced")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
            }

            if let chapter = roadmap.currentProject { WaveProjectView(project: chapter) }
            switch roadmap.tasks {
            case .unavailable(let reason):
                Label(reason, systemImage: "exclamationmark.triangle").foregroundStyle(palette.warning)
            case .available(let tasks, let truncated):
                if tasks.isEmpty { Text("No Tasks in this Project.").foregroundStyle(palette.textSecondary) }
                ForEach(tasks) { task in
                    RoadmapTaskRow(task: task,
                        activeControlId: activeControlId, onSelect: { onOpenTask(task.id) },
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
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
        .overlay {
            RoundedRectangle(cornerRadius: CornerRadius.lg)
                .stroke(palette.border, lineWidth: 1)
        }
        .accessibilityIdentifier("loopflow-wave-\(roadmap.wave.id)")
    }
}

struct RoadmapTaskRow: View {
    let task: RoadmapTask
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
                .foregroundStyle(task.task.isSuccessful ? palette.success : task.section.color(in: palette))
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
                        .foregroundStyle(task.section.color(in: palette))
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
        .background(palette.background)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
        .contentShape(Rectangle())
        .onTapGesture { onSelect() }
        .accessibilityIdentifier("loopflow-task-\(task.id)")
    }
}

/// The shared Task condition plus its orthogonal planning facts. `runtime == nil`
/// means Work has not started.
struct WorkChannelChips: View {
    let task: RoadmapTask
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.xs) {
            channel("Condition", task.condition.state.rawValue, task.condition.state.color(in: palette))
            channel("PM", task.task.terminalLabel ?? "open",
                    task.task.isSuccessful ? palette.success : palette.textSecondary)
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
        status.isTerminal ? palette.textTertiary : palette.working
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
            if let github = task.pr?.publication?.github {
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
                    .foregroundStyle(section.group.color(in: palette))
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
                    activeControlId: activeControlId,
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
                Text(row.wave?.name ?? "No Wave")
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
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
        .overlay {
            RoundedRectangle(cornerRadius: CornerRadius.sm)
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
        case .backlog, .autonomous, .done, .abandoned: palette.textTertiary
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
