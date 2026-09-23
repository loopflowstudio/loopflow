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
                projectDetail
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

                WaveMetricPortfolioView(
                    portfolio: roadmap.metricPortfolio,
                    projectNames: Dictionary(uniqueKeysWithValues:
                        roadmap.projects.items.map { ($0.project.id, $0.project.name) }
                    )
                )

                let sections = nowSections(from: [roadmap])
                if sections.isEmpty {
                    Text("Nothing in this Wave can move right now.")
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                } else {
                    ForEach(sections) { section in
                        NowSectionView(
                            section: section,
                            selection: model.selection,
                            activeControlId: activeControlId,
                            onSelect: { row in model.select(.task(id: row.task.id)) },
                            onTaskAction: { row, action in
                                perform(action, on: WorkTaskSelection(wave: row.wave, task: row.task))
                            },
                            onOpenWorktree: openWorktree
                        )
                    }
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

    // MARK: - Project detail

    @ViewBuilder
    private var projectDetail: some View {
        if let selection = model.selection, let found = model.project(id: selection.id) {
            scrollingDetail(identifier: "podium-detail-project") {
                VStack(alignment: .leading, spacing: Spacing.xxs) {
                    HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                        surfaceHeader(
                            found.project.project.name,
                            subtitle: "\(found.wave.wave.name) · \(found.project.nextMove.owner.rawValue)"
                        )
                        sectionBadge(found.project.section)
                    }
                    if !found.project.project.definition.isEmpty {
                        Text(found.project.project.definition)
                            .font(Typography.body(13))
                            .foregroundStyle(palette.textSecondary)
                    }
                    Text(found.project.nextMove.reason)
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                }

                projectProof(found.project.project)

                if found.project.tasks.isEmpty {
                    Text("No Tasks filed under this Project yet.")
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                } else {
                    ForEach(found.project.tasks) { task in
                        RoadmapTaskRow(
                            task: task,
                            isSelected: false,
                            activeControlId: activeControlId,
                            onSelect: { model.select(.task(id: task.id)) },
                            onAction: { action in
                                perform(action, on: WorkTaskSelection(wave: found.wave.wave, task: task))
                            },
                            onOpenWorktree: openWorktree
                        )
                    }
                }
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
                    Text("\(found.wave.wave.name) · \(found.project.project.name)")
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
                            .font(Typography.sectionTitle(20))
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
                Text("Task directive").font(Typography.sectionTitle(16))
                Text(task.task.description.isEmpty ? "No directive recorded." : task.task.description)
                    .font(Typography.body(13))
                    .textSelection(.enabled)
                    .accessibilityIdentifier("workspace-task-directive")
                if let workspace = task.reference.workspace {
                    Text(workspace.worktree)
                        .font(Typography.code(11)).textSelection(.enabled)
                }
                Text(found.project.project.name).font(Typography.sectionTitle(16))
                Text(found.project.project.definition)
                    .font(Typography.body(13)).textSelection(.enabled)
                projectProof(found.project.project)
            }
        } else {
            missingSelection
        }
    }

    // MARK: - Shared pieces

    private func projectProof(_ project: ProjectPlanningSnapshot) -> some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Key results").font(Typography.sectionTitle(16))
            if project.krs.isEmpty {
                Text("No key results recorded.")
            }
            ForEach(project.krs) { kr in
                Label(kr.text, systemImage: kr.holds ? "checkmark.circle.fill" : "circle")
                    .textSelection(.enabled)
                    .accessibilityLabel("\(kr.holds ? "Holds" : "Not established"): \(kr.text)")
            }
        }
        .font(Typography.body(12))
        .accessibilityIdentifier("workspace-project-proof")
    }

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
                .font(Typography.sectionTitle(22))
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

    private struct WorkTaskSelection {
        let wave: WaveSnapshot
        let task: RoadmapTask
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
#endif
