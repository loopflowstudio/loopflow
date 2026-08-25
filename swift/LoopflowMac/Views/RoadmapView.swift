#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

enum RoadmapTaskAction: Equatable {
    case run
    case resume
    case openPr

    var label: String {
        switch self {
        case .run: "Start"
        case .resume: "Resume"
        case .openPr: "Open PR"
        }
    }
}

/// Pick the one contextual Task action from Rust's legal-action model. The
/// server decides what is legal and recommends one move; this maps that
/// recommendation onto the affordance the app can offer, and never re-derives
/// it from status.
func roadmapTaskAction(_ task: RoadmapTask) -> RoadmapTaskAction? {
    guard task.runtime != nil else {
        return task.attention.pmCompleted ? nil : .run
    }
    switch task.attention.actions.recommended {
    case .resume: return .resume
    case .openPr:
        if task.activePr?.publication?.github != nil { return .openPr }
    case .startNextPr, .complete, .noAction, .none:
        break
    }
    return nil
}

private struct RoadmapTaskSelection: Identifiable {
    let wave: WaveSnapshot
    let task: RoadmapTask

    var id: String { "\(wave.id):\(task.id)" }
}

/// One query, two shapes. Both read the single `lf roadmap` snapshot: NOW
/// re-shapes it into a flat, cross-wave, attention-grouped list; ROADMAP keeps
/// the Wave › Project › Task tree.
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

/// The Podium's shared Work surface: one machine-wide `lf roadmap --json`
/// read, rendered without re-querying each Wave or inventing another work model.
struct RoadmapView: View {
    let onOpenWave: (WaveSnapshot) -> Void

    @Environment(\.palette) private var palette
    // Externally-owned singleton: observe it, don't @StateObject-own it (see
    // WaveDetailPane) — the create-and-own lifecycle fires the publisher during
    // the first body pass and logs an AttributeGraph cycle at cold launch.
    @ObservedObject private var terminalStore = TaskTerminalStore.shared
    @State private var model: PodiumModel
    @Binding private var selection: WorkReference?
    @State private var lens: WorkLens = .now
    @State private var controlError: String?
    @State private var activeControlId: String?
    @State private var workspaceSelection: RoadmapTaskSelection?

    init(
        repoPath: String?,
        onOpenWave: @escaping (WaveSnapshot) -> Void
    ) {
        _model = State(initialValue: PodiumModel(
            query: RegistryQueryLocal.shared,
            repoPath: repoPath
        ))
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

    private var selectedProject: (wave: WaveRoadmap, project: RoadmapProject)? {
        guard let selection else { return nil }
        switch selection.kind {
        case .project:
            return model.project(id: selection.id)
        case .task:
            guard let task = model.task(id: selection.id) else { return nil }
            return (task.wave, task.project)
        case .wave:
            return nil
        }
    }

    private var selectedTask: (
        wave: WaveRoadmap,
        project: RoadmapProject,
        task: RoadmapTask
    )? {
        guard let selection, selection.kind == .task else { return nil }
        return model.task(id: selection.id)
    }

    var body: some View {
        VStack(spacing: 0) {
            header
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
        .task(id: repoPath) {
            await refresh()
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(15))
                if Task.isCancelled { return }
                await refresh()
            }
        }
        .sheet(item: $workspaceSelection) { selection in
            TaskWorkspaceView(
                task: selection.task.task,
                reference: selection.task.reference,
                runtime: selection.task.runtime,
                attention: selection.task.attention,
                repoPath: selection.wave.repo,
                terminalStore: terminalStore,
                initialSection: .changes
            )
        }
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
            if let project = selectedProject?.project {
                breadcrumbSeparator
                Button(project.project.name) { selection = .project(id: project.id) }
                    .buttonStyle(.plain)
                    .disabled(selection == .project(id: project.id))
            }
            if let task = selectedTask?.task {
                breadcrumbSeparator
                Text(task.task.identifier)
            }
        }
        .font(Typography.caption(9).weight(.semibold))
        .foregroundStyle(palette.textSecondary)
        .accessibilityIdentifier("podium-work-breadcrumb")
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
                selectedProject?.project.project.name ?? "Project"
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
                selectedProject?.project.project.definition ?? "Project unavailable"
            case .task:
                selectedTask?.task.attention.reason ?? "Task unavailable"
            }
        }
    }

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
        } else if visibleWaves.isEmpty {
            ContentUnavailableView(
                repoPath == nil ? "No planned Work yet" : "No planned Work in this repository",
                systemImage: "map",
                description: Text("Waves without readable Projects remain in the Waves sidebar.")
            )
            .accessibilityIdentifier("podium-work-empty")
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
            if let selectedProject {
                focusedScroll {
                    projectCard(selectedProject.project, wave: selectedProject.wave.wave)
                }
            } else {
                missingFocus("Project unavailable")
            }
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
                "Nothing needs attention",
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
                                perform(action, on: RoadmapTaskSelection(wave: row.wave, task: row.task))
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
            onSetPaused: { paused in
                try await model.setWavePaused(
                    waveId: roadmap.wave.id,
                    paused: paused
                )
            },
            onError: { controlError = $0 },
            onTaskAction: { task, action in
                perform(action, on: RoadmapTaskSelection(wave: roadmap.wave, task: task))
            },
            onOpenWorktree: openWorktree
        )
    }

    private func projectCard(_ project: RoadmapProject, wave: WaveSnapshot) -> some View {
        RoadmapProjectCard(
            project: project,
            selection: selection,
            activeControlId: activeControlId,
            onSelect: { selected in selection = selected },
            onTaskAction: { task, action in
                perform(action, on: RoadmapTaskSelection(wave: wave, task: task))
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
                perform(action, on: RoadmapTaskSelection(wave: wave, task: task))
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

    /// Navigate to the Wave. Starting a stopped Wave remains the Home control's
    /// job; this only opens the detail.
    private func openWave(_ wave: WaveSnapshot) {
        onOpenWave(wave)
    }

    private enum TaskControl {
        case run
        case resume
    }

    private func perform(_ action: RoadmapTaskAction, on selection: RoadmapTaskSelection) {
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

    private func perform(_ control: TaskControl, on selection: RoadmapTaskSelection) {
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
                await refresh()
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

/// A Wave's placed Home on its row: stable identity and current route, plus the
/// probed liveness and the *one* contextual action the shared `HomeRuntimeDto`
/// dictates. The app never does SSH — `lf home probe` and `lf start` route by
/// placement, including to remote Homes. Probed once per
/// Wave card on appear (local reads are instant; a remote Home costs one routed
/// probe), never once per row and never on the 15s roadmap poll.
struct HomeControl: View {
    let wave: WaveSnapshot
    let onOpen: () -> Void
    let onRefresh: () async -> Void
    let onSetPaused: (Bool) async throws -> Void
    let onError: (String) -> Void

    @Environment(\.palette) private var palette
    @State private var runtime: HomeRuntime?
    @State private var probeError: String?
    @State private var isProbing = false
    @State private var isActing = false

    var body: some View {
        VStack(alignment: .trailing, spacing: Spacing.xxs) {
            HStack(spacing: Spacing.xs) {
                Image(systemName: "house")
                    .font(Typography.caption(9))
                    .foregroundStyle(palette.textSecondary)
                Text(wave.home.route == "local" ? wave.home.id : wave.home.route)
                    .font(Typography.caption(10).weight(.medium))
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
                if isProbing {
                    ProgressView().controlSize(.small)
                } else if let runtime {
                    stateChip(runtime.state)
                }
            }
            HStack(spacing: Spacing.xs) {
                turnAction
                homeAction
            }
        }
        .task(id: wave.id) { await probe() }
    }

    @ViewBuilder
    private var turnAction: some View {
        Group {
            if isActing {
                ProgressView().controlSize(.small)
            } else {
                Button(wave.paused ? "Resume" : "Pause") {
                    Task { await setPaused(!wave.paused) }
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help(
                    wave.paused
                        ? "Enable new turns for this Wave"
                        : "Refuse new turns while the listener keeps serving"
                )
                .accessibilityIdentifier("wave-turn-control-\(wave.id)")
            }
        }
    }

    @ViewBuilder
    private var homeAction: some View {
        if !isActing, let runtime {
            switch runtime.action {
            case .attach:
                Button("Open") { onOpen() }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    .help(runtime.endpoint.map { "Attach to \($0)" } ?? "Open the Wave")
            case .start(let homeId):
                Button("Start on \(homeId)") { Task { await start() } }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
            case .reason(let message):
                Text(message)
                    .font(Typography.caption(9))
                    .foregroundStyle(Color.statusWarning)
                    .lineLimit(2)
                    .textSelection(.enabled)
            }
        } else if let probeError {
            Text(probeError)
                .font(Typography.caption(9))
                .foregroundStyle(Color.statusError)
                .lineLimit(2)
        }
    }

    private func stateChip(_ state: HomeState) -> some View {
        let (label, color): (String, Color) = switch state {
        case .running: ("running", .statusSuccess)
        case .stopped: ("stopped", .statusNeutral)
        case .unreachable: ("unreachable", .statusError)
        case .unknown: ("unknown", .statusWarning)
        }
        return Text(label)
            .font(Typography.caption(9).weight(.semibold))
            .foregroundStyle(color)
            .padding(.horizontal, Spacing.xs)
            .padding(.vertical, 1)
            .background(color.opacity(0.12))
            .clipShape(Capsule())
    }

    @MainActor
    private func probe() async {
        isProbing = true
        defer { isProbing = false }
        do {
            runtime = try await RegistryQueryLocal.shared.homeProbe(wave: wave.name, cwd: wave.repo)
            probeError = nil
        } catch {
            probeError = error.localizedDescription
        }
    }

    @MainActor
    private func start() async {
        isActing = true
        defer { isActing = false }
        do {
            _ = try await RegistryQueryLocal.shared.start(wave: wave.name, cwd: wave.repo)
            runtime = try await RegistryQueryLocal.shared.homeProbe(wave: wave.name, cwd: wave.repo)
            await onRefresh()
            onOpen()
        } catch {
            onError(error.localizedDescription)
        }
    }

    @MainActor
    private func setPaused(_ paused: Bool) async {
        isActing = true
        defer { isActing = false }
        do {
            try await onSetPaused(paused)
        } catch {
            onError(error.localizedDescription)
        }
    }
}

private struct RoadmapWaveCard: View {
    let roadmap: WaveRoadmap
    let selection: WorkReference?
    let activeControlId: String?
    let onSelect: (WorkReference) -> Void
    let onOpen: () -> Void
    let onRefresh: () async -> Void
    let onSetPaused: (Bool) async throws -> Void
    let onError: (String) -> Void
    let onTaskAction: (RoadmapTask, RoadmapTaskAction) -> Void
    let onOpenWorktree: (TaskWorkspaceSnapshot) -> Void

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            HStack(alignment: .top, spacing: Spacing.md) {
                VStack(alignment: .leading, spacing: Spacing.xxs) {
                    HStack(spacing: Spacing.sm) {
                        Circle()
                            .fill(
                                roadmap.wave.paused
                                    ? WaveLensColor.blue.glow
                                    : roadmap.wave.live ? Color.statusSuccess : Color.statusNeutral
                            )
                            .frame(width: 7, height: 7)
                        Text(roadmap.wave.name)
                            .font(Typography.sectionTitle(18))
                            .foregroundStyle(palette.text)
                        Text(roadmap.wave.status.label)
                            .font(Typography.caption(10))
                            .foregroundStyle(palette.textSecondary)
                        if roadmap.wave.paused {
                            Text("paused")
                                .font(Typography.caption(9).weight(.semibold))
                                .foregroundStyle(WaveLensColor.blue.glow)
                                .padding(.horizontal, Spacing.xs)
                                .padding(.vertical, 1)
                                .background(WaveLensColor.blue.glow.opacity(0.12))
                                .clipShape(Capsule())
                                .accessibilityIdentifier("wave-paused-\(roadmap.wave.id)")
                        }
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
                HomeControl(
                    wave: roadmap.wave,
                    onOpen: onOpen,
                    onRefresh: onRefresh,
                    onSetPaused: onSetPaused,
                    onError: onError
                )
            }

            switch roadmap.projects {
            case .unavailable(let reason):
                Label(reason, systemImage: "exclamationmark.triangle")
                    .font(Typography.caption(11))
                    .foregroundStyle(Color.statusWarning)
                    .textSelection(.enabled)
            case .available(let projects, let truncated):
                if projects.isEmpty {
                    Text("No Project or Task rows in the local plan snapshot.")
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                } else {
                    ForEach(projects) { project in
                        RoadmapProjectCard(
                            project: project,
                            selection: selection,
                            activeControlId: activeControlId,
                            onSelect: onSelect,
                            onTaskAction: onTaskAction,
                            onOpenWorktree: onOpenWorktree
                        )
                    }
                }
                if truncated {
                    Text("Older plan rows are truncated.")
                        .font(Typography.caption(10))
                        .foregroundStyle(Color.statusWarning)
                }
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
        .accessibilityIdentifier("podium-wave-\(roadmap.wave.id)")
    }
}

private struct RoadmapProjectCard: View {
    let project: RoadmapProject
    let selection: WorkReference?
    let activeControlId: String?
    let onSelect: (WorkReference) -> Void
    let onTaskAction: (RoadmapTask, RoadmapTaskAction) -> Void
    let onOpenWorktree: (TaskWorkspaceSnapshot) -> Void

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                Text(project.project.name)
                    .font(Typography.sectionTitle(15))
                    .foregroundStyle(palette.text)
                sectionBadge(project.section)
                Spacer()
                Text(project.nextMove.owner.rawValue)
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
            }
            .contentShape(Rectangle())
            .onTapGesture {
                onSelect(.project(id: project.id))
            }
            if !project.project.definition.isEmpty {
                Text(project.project.definition)
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(2)
            }
            if project.tasks.isEmpty {
                Text(project.nextMove.reason)
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
            } else {
                ForEach(project.tasks) { task in
                    RoadmapTaskRow(
                        task: task,
                        isSelected: selection == .task(id: task.id),
                        activeControlId: activeControlId,
                        onSelect: {
                            onSelect(.task(id: task.id))
                        },
                        onAction: { action in onTaskAction(task, action) },
                        onOpenWorktree: onOpenWorktree
                    )
                }
            }
        }
        .padding(Spacing.md)
        .background(
            selection == .project(id: project.id)
                ? Color.loopflowBurgundy.opacity(0.08)
                : palette.surfaceMuted.opacity(0.6)
        )
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        .accessibilityAddTraits(
            selection == .project(id: project.id) ? [.isSelected] : []
        )
        .accessibilityIdentifier("podium-project-\(project.id)")
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
}

struct RoadmapTaskRow: View {
    let task: RoadmapTask
    let isSelected: Bool
    let activeControlId: String?
    let onSelect: () -> Void
    let onAction: (RoadmapTaskAction) -> Void
    let onOpenWorktree: (TaskWorkspaceSnapshot) -> Void

    @Environment(\.palette) private var palette

    private var isActing: Bool { activeControlId == "task:\(task.id)" }

    var body: some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            Image(systemName: task.task.completed ? "checkmark.circle.fill" : "circle")
                .font(Typography.caption(11))
                .foregroundStyle(task.task.completed ? Color.statusSuccess : task.section.color)
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
                Text(task.attention.reason)
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(2)
                    .accessibilityLabel(taskAttentionAccessibilityLabel(task))
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
        .accessibilityIdentifier("podium-task-\(task.id)")
        .opacity(roadmapTaskIsActionable(task) ? 1 : 0.55)
    }
}

/// The shared attention fold plus its orthogonal planning facts. `runtime == nil`
/// means Work has not started.
struct WorkChannelChips: View {
    let task: RoadmapTask
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.xs) {
            channel("Attention", task.attention.level.rawValue, task.attention.level.color)
            channel("PM", task.task.completed ? "done" : "open",
                    task.task.completed ? Color.statusSuccess : palette.textSecondary)
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

    private func statusColor(_ status: WorkStatus) -> Color {
        switch status {
        case .done, .abandoned: .statusNeutral
        case .ready: .statusInfo
        }
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
    let onOpenWorktree: (TaskWorkspaceSnapshot) -> Void

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
    let onOpenWorktree: (TaskWorkspaceSnapshot) -> Void

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
        .accessibilityIdentifier("podium-now-\(section.group.rawValue)")
    }

    private func nowColor(_ group: NowGroup) -> Color {
        switch group {
        case .readyForReview: .statusInfo
        case .needsInput: .statusWarning
        case .stopped: .statusNeutral
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
    let onOpenWorktree: (TaskWorkspaceSnapshot) -> Void

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
                Text("\(row.wave.name) · \(row.projectName)")
                    .font(Typography.caption(9))
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(1)
            }
            Text(task.attention.reason)
                .font(Typography.caption(10))
                .foregroundStyle(palette.textSecondary)
                .lineLimit(2)
                .accessibilityLabel(taskAttentionAccessibilityLabel(task))
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
        .accessibilityIdentifier("podium-task-\(task.id)")
        .opacity(roadmapTaskIsActionable(task) ? 1 : 0.55)
    }
}

extension RoadmapSection {
    var label: String {
        switch self {
        case .now: "Now"
        case .needsAttention: "Needs attention"
        case .available: "Available"
        case .later: "Later"
        }
    }

    var color: Color {
        switch self {
        case .now: .statusSuccess
        case .needsAttention: .statusWarning
        case .available: .statusInfo
        case .later: .statusNeutral
        }
    }
}

private extension TaskAttentionLevel {
    var color: Color {
        switch self {
        case .red: .statusError
        case .blue: WaveLensColor.blue.glow
        case .black: .statusNeutral
        case .unknown: .statusWarning
        }
    }
}
#endif
