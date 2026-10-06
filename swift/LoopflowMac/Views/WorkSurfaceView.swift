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
            if let selection = model.selection, selection.kind == .task,
               let error = model.navigation.taskSessionErrors[selection.id] {
                evidenceBanner(title: "New session failed", detail: error)
            }
            content
        }
        .background(palette.background)
        .sheet(item: $model.historyWave) { wave in
            ProjectHistoryView(wave: wave.name, repo: wave.repo, sourceReference: model.historyReference)
        }
        .sheet(item: $editingTask) { selection in
            TaskDirectiveEditor(model: model, task: selection.task, wave: selection.wave)
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

    // MARK: - Wave detail

    @ViewBuilder
    private var waveDetail: some View {
        if let selection = model.selection, let roadmap = model.wave(id: selection.id) {
            scrollingDetail(identifier: "podium-detail-wave") {
                VStack(alignment: .leading, spacing: Spacing.md) {
                    HStack(alignment: .center, spacing: Spacing.md) {
                        Text(roadmap.wave.displayName)
                            .font(Typography.display)
                            .foregroundStyle(palette.text)
                            .accessibilityIdentifier("wave-title")
                        Spacer()
                    }
                    if !roadmap.wave.goal.isEmpty {
                        WaveObjectiveView(goal: roadmap.wave.goal, waveId: roadmap.wave.id)
                    }
                    ForEach(roadmap.unavailableTasks, id: \.taskId) { task in
                        planningNotice(task)
                    }
                }

                section {
                    WorkspaceSectionHeading(title: "Current KRs") {
                        Button("Project history") {
                            model.historyReference = nil
                            model.historyWave = roadmap.wave
                        }
                        .buttonStyle(.plain)
                        .font(Typography.body(12.5))
                        .foregroundStyle(palette.textTertiary)
                        .accessibilityIdentifier("wave-chapter-history")
                    }
                    if let chapter = roadmap.currentProject {
                        WaveProjectView(project: chapter)
                    } else {
                        Text("No current Project plan.").font(Typography.body(13)).foregroundStyle(palette.textSecondary)
                    }
                }

                if let name = roadmap.currentProject?.flow {
                    section {
                        WorkspaceSectionHeading("Flow · \(name)")
                        if let entry = model.flowCatalog.value?.first(where: { $0.name == name }),
                           let graph = entry.graph, let template = entry.template {
                            FlowTemplateView(graph: graph, template: template, navigation: model.navigation)
                        } else {
                            Text(model.flowCatalog.value?.first(where: { $0.name == name })?.unavailable
                                 ?? model.flowCatalog.errorMessage ?? "Flow template unavailable")
                                .font(Typography.caption())
                        }
                    }
                    .task { await model.loadFlowCatalog() }
                }

                switch roadmap.tasks {
                case .unavailable(let reason):
                    section {
                        WorkspaceSectionHeading("Tasks")
                        Text(reason).font(Typography.body(13)).foregroundStyle(Color.statusWarning)
                    }
                case .available(let inventory, let truncated):
                    let filter = model.taskHistoryFilters[roadmap.wave.id] ?? TaskHistoryFilter()
                    let tasks = inventory.filter {
                        filter.includes($0.task, condition: $0.condition, now: model.taskHistoryNow)
                    }
                    section {
                        WorkspaceSectionHeading(title: "Tasks", count: tasks.count) {
                            TaskHistoryControls(filter: Binding(
                                get: { model.taskHistoryFilters[roadmap.wave.id] ?? TaskHistoryFilter() },
                                set: { model.taskHistoryFilters[roadmap.wave.id] = $0; model.taskHistoryNow = Date() }
                            ))
                            .id(roadmap.wave.id)
                        }
                        .accessibilityIdentifier("wave-task-count")
                        if tasks.isEmpty {
                            Text("No current Tasks").font(Typography.body(13)).foregroundStyle(palette.textSecondary)
                        } else {
                            VStack(alignment: .leading, spacing: 0) {
                                ForEach(Array(tasks.sorted { $0.task.rank < $1.task.rank }.enumerated()), id: \.element.id) { index, task in
                                    if index > 0 { Divider().overlay(palette.border.opacity(0.6)) }
                                    planRow(task, rank: index + 1)
                                }
                            }
                            .workspacePanel()
                        }
                        if truncated {
                            Text("Planning is partial; more Tasks exist.").font(Typography.body(12)).foregroundStyle(Color.statusWarning)
                        }
                    }
                }
                WaveMetricPortfolioView(portfolio: roadmap.metricPortfolio)
            }
            .onChange(of: roadmap.wave.id, initial: true) { _, id in
                Perf.endAfterCommit(Perf.taskWorkspaceReady, id: id)
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
            let sessions = model.sessions.value.map { _ in
                model.visibleSessions.filter { record in
                    guard record.primaryScope == nil else { return false }
                    return task.runtime.map {
                        record.taskIds.contains($0.workId) || record.workspace?.taskId == $0.workId
                    } ?? false
                }
            }
            scrollingDetail(identifier: "podium-detail-task") {
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
                            .buttonStyle(WorkspaceOutlineButtonStyle())
                            .disabled(model.navigation.startingTaskSessions.contains(task.id))
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
                        if let workspace = task.reference.workspace {
                            Button("Worktree") { openWorktree(workspace) }
                                .buttonStyle(.plain)
                                .font(Typography.body(12.5))
                        }
                        if let github = task.activePr?.publication?.github {
                            Link("PR #\(github.number)", destination: github.url)
                                .font(Typography.body(12.5))
                        }
                    }
                    .tint(palette.accentInk)
                    .foregroundStyle(palette.accentInk)
                }
                if let unavailable = found.wave.unavailableTasks.first(where: { $0.taskId == task.id }) {
                    evidenceBanner(title: "Retained Task · planning unavailable", detail: unavailable.reason)
                    if case .pinned = task.flow.record {
                        TaskFlowView(model: model, task: task, wave: found.wave.wave, onOpenSession: onOpenSession).id(task.id)
                    }
                } else {
                    TaskFlowView(model: model, task: task, wave: found.wave.wave, onOpenSession: onOpenSession).id(task.id)
                }
                TaskWorkView(model: model, task: task, wave: found.wave.wave)
                TaskHistoryView(model: model, task: task, wave: found.wave.wave)
                    .id(task.id)
                if let sessions, !sessions.isEmpty {
                    section {
                        WorkspaceSectionHeading("Sessions", count: sessions.count)
                        VStack(alignment: .leading, spacing: 0) {
                            ForEach(Array(sessions.enumerated()), id: \.element.id) { index, session in
                                if index > 0 { Divider().overlay(palette.border.opacity(0.6)) }
                                sessionRow(session)
                            }
                        }
                        .workspacePanel()
                    }
                } else if sessions != nil, model.sessions.value != nil, model.sessions.errorMessage == nil {
                    Text("No open Sessions.")
                        .font(Typography.body(12.5))
                        .foregroundStyle(palette.textTertiary)
                        .accessibilityIdentifier("workspace-no-sessions")
                }
                section {
                    WorkspaceSectionHeading(title: "Description") {
                        Button("Edit description") {
                            editingTask = WorkTaskSelection(wave: found.wave.wave, task: task)
                        }
                        .buttonStyle(.plain)
                        .font(Typography.body(12.5))
                        .foregroundStyle(palette.textTertiary)
                        .accessibilityIdentifier("workspace-edit-directive")
                    }
                    Group {
                        if task.task.description.isEmpty {
                            Text("No description recorded.").foregroundStyle(palette.textSecondary)
                        } else {
                            MarkdownBlocks(source: task.task.description).equatable()
                        }
                    }
                    .frame(maxWidth: 680, alignment: .leading)
                    .accessibilityIdentifier("workspace-task-directive")
                }
                TaskCommentsView(model: model, task: task, wave: found.wave.wave)
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

    private func sessionRow(_ session: SessionRecord) -> some View {
        Button { onOpenSession(session) } label: {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                Image(systemName: "bubble.left")
                    .font(.system(size: 12))
                    .foregroundStyle(palette.textTertiary)
                    .frame(width: 16)
                VStack(alignment: .leading, spacing: 2) {
                    Text(session.title)
                        .font(Typography.body(13).weight(.bold))
                        .foregroundStyle(palette.text)
                    HStack(spacing: Spacing.xs) {
                        if let provider = session.provider {
                            Text(provider)
                                .font(Typography.caption(11))
                                .foregroundStyle(palette.textSecondary)
                                .accessibilityIdentifier("task-session-provider-\(session.id)")
                            Text("·").foregroundStyle(palette.textTertiary)
                        }
                        Text(session.flowMembership.label)
                            .font(Typography.code(11))
                            .foregroundStyle(membershipTone(session.flowMembership))
                    }
                    // Only a summary the Session actually recorded; never a
                    // preview derived from its prompt or transcript.
                    if let summary = session.readySummary {
                        Text(summary)
                            .font(Typography.body(12))
                            .foregroundStyle(palette.textSecondary)
                            .lineLimit(2)
                            .help(summary)
                            .accessibilityIdentifier("task-session-summary-\(session.id)")
                    }
                }
                Spacer(minLength: Spacing.sm)
                WorkspaceChip(text: session.state.rawValue.capitalized, tone: sessionTone(session.state))
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("task-session-\(session.id)")
    }

    private func membershipTone(_ membership: SessionFlowMembership) -> Color {
        if case .step = membership { return FlowPalette.loops[1] }
        return palette.textTertiary
    }

    private func sessionTone(_ state: SessionState) -> WorkspaceTone {
        switch state {
        case .unknown: .neutral
        case .active: .running
        case .waiting, .ready: .human
        case .closed, .interrupted: .stopped
        }
    }

    private func planRow(_ task: RoadmapTask, rank: Int) -> some View {
        let state = planState(task)
        return Button {
            if let onOpenTask { onOpenTask(.task(id: task.id)) } else { model.select(.task(id: task.id)) }
        } label: {
            HStack(spacing: 10) {
                Text("\(rank)")
                    .font(Typography.caption(12))
                    .monospacedDigit()
                    .foregroundStyle(palette.textTertiary)
                    .frame(width: 20, alignment: .trailing)
                Circle()
                    .fill(state.tone == .neutral ? Color.clear : state.tone.ink)
                    .overlay(Circle().strokeBorder(state.tone == .neutral ? palette.borderStrong : state.tone.ink, lineWidth: 1.5))
                    .frame(width: 9, height: 9)
                    .frame(width: 14)
                Text(task.task.name)
                    .font(Typography.body(13))
                    .foregroundStyle(task.task.completed ? palette.textTertiary : palette.text)
                    .lineLimit(1)
                    .truncationMode(.tail)
                    .frame(maxWidth: .infinity, alignment: .leading)
                // A chip only where a state needs eyes; the dot carries the rest.
                Group {
                    if let label = state.label { WorkspaceChip(text: label, tone: state.tone) }
                }
                .frame(width: 96, alignment: .leading)
                Text(task.task.identifier)
                    .font(Typography.code(11.5))
                    .foregroundStyle(palette.textTertiary)
                    .frame(width: 64, alignment: .trailing)
            }
            .padding(.horizontal, 10)
            .frame(minHeight: 32)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(task.task.name)
        .accessibilityIdentifier("podium-task-\(task.id)")
    }

    /// The plan row's state, read from the shared Task projection. Only running,
    /// done, human, blocked and stalled earn a chip; stopped and unstarted rows keep the dot.
    private func planState(_ task: RoadmapTask) -> (label: String?, tone: WorkspaceTone) {
        if let label = task.task.historyLabel {
            return (label, task.task.isSuccessful ? .done : .neutral)
        }
        if case .pinned(let pinned) = task.flow.record {
            return pinned.execution.presentation
        }
        return (nil, .neutral)
    }

    /// A planning gap as one quiet line; Details holds the full reason and the
    /// recovery command. Nothing is hidden permanently.
    private func planningNotice(_ task: UnavailableTaskEvidence) -> some View {
        let open = model.navigation.expandedNotices.contains(task.taskId)
        return VStack(alignment: .leading, spacing: Spacing.xs) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                Image(systemName: "exclamationmark.circle")
                    .foregroundStyle(WorkspaceTone.human.ink)
                Text("\(task.taskIdentifier) is outside the current plan.")
                    .foregroundStyle(WorkspaceTone.human.text)
                Spacer(minLength: Spacing.sm)
                Button(open ? "Hide details" : "Details") {
                    if open { model.navigation.expandedNotices.remove(task.taskId) } else { model.navigation.expandedNotices.insert(task.taskId) }
                }
                .buttonStyle(.plain)
                .foregroundStyle(palette.accentInk)
                .accessibilityIdentifier("wave-notice-details-\(task.taskId)")
            }
            if open {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    Text("\(task.taskIdentifier): \(task.reason)").textSelection(.enabled)
                    Text("Recover with").font(Typography.caption(11)).foregroundStyle(palette.textTertiary)
                    Text(task.recovery)
                        .font(Typography.code(11.5))
                        .textSelection(.enabled)
                        .padding(Spacing.sm)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(palette.surface, in: RoundedRectangle(cornerRadius: 6))
                        .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(palette.border))
                }
                .foregroundStyle(palette.text)
                .padding(.leading, 22)
            }
        }
        .font(Typography.body(12.5))
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(WorkspaceTone.human.fill.opacity(0.55), in: RoundedRectangle(cornerRadius: 10))
        .accessibilityIdentifier("wave-notice-\(task.taskId)")
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
                .font(Typography.body(14).weight(.bold))
                .foregroundStyle(palette.text)
            Text(subtitle)
                .font(Typography.caption(11))
                .foregroundStyle(palette.textSecondary)
        }
    }


    private func evidenceBanner(title: String, detail: String) -> some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(Color.statusWarning)
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                Text(title)
                    .font(Typography.caption(11).weight(.bold))
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

/// The Wave objective as a serif lede, clamped to four lines with a "more"
/// disclosure only when the text really overflows. Expansion is per Wave and
/// per view lifetime; the stored objective is never abbreviated.
struct WaveObjectiveView: View {
    let goal: String
    let waveId: String

    @Environment(\.palette) private var palette
    @State private var expanded: Set<String> = []
    @State private var clampedHeight: CGFloat = 0
    @State private var fullHeight: CGFloat = 0

    private static let lineLimit = 4
    private var open: Bool { expanded.contains(waveId) }
    private var overflows: Bool { fullHeight > clampedHeight + 1 }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text(goal)
                .font(Typography.lede)
                .foregroundStyle(palette.textSecondary)
                .lineSpacing(4)
                .lineLimit(open ? nil : Self.lineLimit)
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)
                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { clampedHeight = $0 }
                .background {
                    // The unclamped height decides whether "more" is offered.
                    Text(goal)
                        .font(Typography.lede)
                        .lineSpacing(4)
                        .fixedSize(horizontal: false, vertical: true)
                        .hidden()
                        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { fullHeight = $0 }
                }
                .accessibilityIdentifier("wave-objective")
            if open || overflows {
                Button(open ? "less" : "more") {
                    if expanded.remove(waveId) == nil { expanded.insert(waveId) }
                }
                .buttonStyle(.plain)
                .font(Typography.body(12.5))
                .foregroundStyle(palette.accentInk)
                .accessibilityLabel(open ? "Show less of the objective" : "Show the whole objective")
                .accessibilityIdentifier("wave-objective-more")
            }
        }
        .frame(maxWidth: 640, alignment: .leading)
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
                .font(Typography.body(14).weight(.bold))
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
