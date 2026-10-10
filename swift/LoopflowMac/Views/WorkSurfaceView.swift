// Work inspectors consume the same planning reading as the compact navigator.

#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

struct WorkSurfaceView: View {
    @Bindable var model: WorkModel
    var onOpenSession: (SessionRecord) -> Void = { _ in }
    /// Opens a Task the way the sidebar does: its one Session, else its overview.
    var onOpenTask: ((WorkReference) -> Void)?
    /// Starts an independent Task-context conversation in the Task's checkout.
    var onNewSession: ((String) async throws -> Void)?

    @Environment(\.palette) private var palette
    @State private var controlError: String?
    @State private var editingTask: TaskSelection?

    private var snapshot: RoadmapSnapshot? { model.roadmap.value }
    private var queryError: String? { model.roadmap.errorMessage }
    private var visibleWaves: [WaveRoadmap] { model.visibleRoadmaps }
    private var openedWave: WaveSnapshot? {
        guard let selection = model.selection, selection.kind == .wave else { return nil }
        return model.wave(id: selection.id)?.wave
    }

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
        .onChange(of: openedWave?.id, initial: true) { _, _ in
            guard let wave = openedWave else { return }
            model.activateProject(id: wave.id, name: wave.name, repo: wave.repo)
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
                .accessibilityIdentifier("loopflow-work-loading")
        } else if snapshot == nil {
            ContentUnavailableView(
                "Work unavailable",
                systemImage: "exclamationmark.triangle",
                description: Text("Couldn't read the latest work. Refresh to try again.")
            )
            .accessibilityIdentifier("loopflow-work-unavailable")
        } else if visibleWaves.isEmpty, model.visibleWaves.isEmpty {
            ContentUnavailableView(
                model.repoPath == nil ? "No planned Work yet" : "No planned Work in this repository",
                systemImage: "map",
                description: Text("Author a Wave to put work on the surface.")
            )
            .accessibilityIdentifier("loopflow-work-empty")
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
            scrollingDetail(identifier: "loopflow-detail-wave") {
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

                ProjectReadinessView(readiness: roadmap.projectReadiness,
                                     isPending: model.isProjectActivationPending(id: roadmap.wave.id),
                                     transportError: model.projectCommandErrors[roadmap.wave.id]) {
                    model.activateProject(id: roadmap.wave.id, name: roadmap.wave.name, repo: roadmap.wave.repo)
                }
                section {
                    WorkSectionHeading(title: "Current KRs") {
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

                if let project = roadmap.currentProject {
                    let name = project.workflow.trimmingCharacters(in: .whitespacesAndNewlines)
                    section { WaveWorkflowView(model: model, wave: roadmap.wave, name: name) }
                        .task { await model.loadWorkflowCatalog() }
                }

                switch roadmap.tasks {
                case .unavailable(let reason):
                    section {
                        WorkSectionHeading("Tasks")
                        Text(reason).font(Typography.body(13)).foregroundStyle(Color.statusWarning)
                    }
                case .available(let inventory, let truncated):
                    let filter = model.taskHistoryFilters[roadmap.wave.id] ?? TaskHistoryFilter()
                    let tasks = inventory.filter {
                        filter.includes($0.task, condition: $0.condition, now: model.taskHistoryNow)
                    }
                    section {
                        WorkSectionHeading(title: "Tasks", count: tasks.count) {
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
                            .workPanel()
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
            scrollingDetail(identifier: "loopflow-detail-wave") {
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
            scrollingDetail(identifier: "loopflow-detail-task") {
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
                            .buttonStyle(WorkOutlineButtonStyle())
                            .disabled(model.navigation.startingTaskSessions.contains(task.id))
                            .help("Open an independent conversation with this Task's context in its worktree. No Flow is started.")
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
                        if let github = task.pr?.publication?.github {
                            Link("PR #\(github.number)", destination: github.url)
                                .font(Typography.body(12.5))
                        }
                    }
                    .tint(palette.accentInk)
                    .foregroundStyle(palette.accentInk)
                    TaskDeliveryView(task: task)
                    if let sync = task.task.sync {
                        PlanningSyncView(sync: sync)
                    }
                }
                if let unavailable = found.wave.unavailableTasks.first(where: { $0.taskId == task.id }) {
                    evidenceBanner(title: "Retained Task · planning unavailable", detail: unavailable.reason)
                }
                TaskHistoryView(model: model, task: task, wave: found.wave.wave)
                    .id(task.id)
                // Raw records, for when the Workflow and the Flow process log disagree with them.
                DisclosureGroup("Debug") { TaskWorkView(model: model, task: task) }
                    .font(Typography.body(12))
                    .foregroundStyle(palette.textSecondary)
                    .accessibilityIdentifier("task-debug")
                if let sessions, !sessions.isEmpty {
                    let groups = TaskSessionGroups(sessions)
                    if !groups.waiting.isEmpty {
                        section {
                            WorkSectionHeading("Waiting", count: groups.waiting.count)
                            VStack(alignment: .leading, spacing: 0) {
                                ForEach(Array(groups.waiting.enumerated()), id: \.element.id) { index, session in
                                    if index > 0 { Divider().overlay(palette.border.opacity(0.6)) }
                                    sessionRow(session)
                                }
                            }
                            .workPanel()
                        }
                        .accessibilityIdentifier("task-sessions-waiting")
                    }
                    if !groups.working.isEmpty {
                        section {
                            WorkSectionHeading("Sessions", count: groups.working.count)
                            VStack(alignment: .leading, spacing: 0) {
                                ForEach(groups.working) { session in compactSessionRow(session) }
                            }
                        }
                        .accessibilityIdentifier("task-sessions-working")
                    }
                } else if sessions != nil, model.sessions.value != nil, model.sessions.errorMessage == nil {
                    Text("No open Sessions.")
                        .font(Typography.body(12.5))
                        .foregroundStyle(palette.textTertiary)
                        .accessibilityIdentifier("work-no-sessions")
                }
                section {
                    WorkSectionHeading(title: "Description") {
                        Button("Edit description") {
                            editingTask = TaskSelection(wave: found.wave.wave, task: task)
                        }
                        .buttonStyle(.plain)
                        .font(Typography.body(12.5))
                        .foregroundStyle(palette.textTertiary)
                        .accessibilityIdentifier("work-edit-directive")
                    }
                    Group {
                        if task.task.description.isEmpty {
                            Text("No description recorded.").foregroundStyle(palette.textSecondary)
                        } else {
                            MarkdownBlocks(source: task.task.description).equatable()
                        }
                    }
                    .frame(maxWidth: 680, alignment: .leading)
                    .accessibilityIdentifier("work-task-directive")
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
                    HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                        Text(session.title)
                            .font(Typography.strong(13))
                            .foregroundStyle(palette.text)
                        primaryTag(session)
                    }
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
                WorkChip(text: session.statusLabel, tone: session.attention == nil ? sessionTone(session.state) : .human)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("task-session-\(session.id)")
    }

    @ViewBuilder
    private func primaryTag(_ session: SessionRecord) -> some View {
        if session.taskPrimary {
            Text("primary").font(Typography.caption(10.5)).foregroundStyle(palette.textTertiary)
        }
    }

    /// One line for a conversation that is not waiting on anyone.
    private func compactSessionRow(_ session: SessionRecord) -> some View {
        Button { onOpenSession(session) } label: {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                Text(session.title)
                    .font(Typography.body(12.5))
                    .foregroundStyle(palette.text)
                    .lineLimit(1)
                primaryTag(session)
                Spacer(minLength: Spacing.sm)
                Text(session.statusLabel)
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textTertiary)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 3)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("task-session-\(session.id)")
    }

    private func membershipTone(_ membership: SessionFlowMembership) -> Color {
        if case .step = membership { return FlowPalette.loops[1] }
        return palette.textTertiary
    }

    private func sessionTone(_ state: SessionState) -> WorkTone {
        switch state {
        case .unknown: .neutral
        case .active: .running
        case .closed: .stopped
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
                    if let label = state.label { WorkChip(text: label, tone: state.tone) }
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
        .accessibilityIdentifier("loopflow-task-\(task.id)")
    }

    /// The plan row's state, read from the shared Task projection. Only running,
    /// done, blocked and stalled earn a chip; stopped and unstarted rows keep the dot.
    private func planState(_ task: RoadmapTask) -> (label: String?, tone: WorkTone) {
        if let label = task.task.historyLabel {
            return (label, task.task.isSuccessful ? .done : .neutral)
        }
        if let execution = task.execution {
            return execution.state.presentation
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
                    .foregroundStyle(WorkTone.human.ink)
                Text("\(task.taskIdentifier) is outside the current plan.")
                    .foregroundStyle(WorkTone.human.text)
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
        .background(WorkTone.human.fill.opacity(0.55), in: RoundedRectangle(cornerRadius: 10))
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
                .font(Typography.strong(14))
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
                    .font(Typography.strong(11))
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

    private struct TaskSelection: Identifiable {
        let wave: WaveSnapshot
        let task: RoadmapTask

        var id: String { "\(wave.id):\(task.id)" }
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
    let model: WorkModel
    let task: RoadmapTask
    let wave: WaveSnapshot

    @Environment(\.dismiss) private var dismiss
    @Environment(\.palette) private var palette
    @State private var draft: String
    @State private var isSaving = false
    @State private var saveError: String?

    init(model: WorkModel, task: RoadmapTask, wave: WaveSnapshot) {
        self.model = model
        self.task = task
        self.wave = wave
        _draft = State(initialValue: task.task.description)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            Text("Edit description · \(task.task.identifier)")
                .font(Typography.strong(14))
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
/// The Project's workflow, taken up by a Task's first `lf task run`: what it
/// draws and its authored definition.
struct WaveWorkflowView: View {
    let model: WorkModel
    let wave: WaveSnapshot
    let name: String
    @State private var editingWorkflow = false
    @Environment(\.palette) private var palette

    private var catalog: [WorkflowCatalogEntry] { model.workflowCatalog.value ?? [] }
    private var entry: WorkflowCatalogEntry? { catalog.first { $0.name == name } }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            WorkSectionHeading(title: "Workflow") {
                // A Project may name no workflow; the menu is where one is chosen.
                Menu(name.isEmpty ? "None" : name) {
                    ForEach(catalog) { choice in
                        Button(choice.unavailable == nil ? choice.name : "\(choice.name) (invalid)") {
                            Task { await model.setWorkflow(choice.name, wave: wave) }
                        }
                        .disabled(choice.unavailable != nil || choice.id == entry?.id)
                        .accessibilityIdentifier("wave-workflow-option-\(choice.id)")
                    }
                }
                .font(Typography.code(12))
                .fixedSize()
                .accessibilityIdentifier("wave-workflow-menu")
                if let entry {
                    // A builtin is copied into the selected definition owner before editing.
                    Button(entry.source == nil ? "Customize" : "Edit") {
                        editingWorkflow = true
                    }
                    .buttonStyle(WorkOutlineButtonStyle())
                    .help("Edit this Wave’s stored workflow")
                    .accessibilityIdentifier("wave-workflow-edit")
                }
            }
            Text(entry.map { $0.source == "stored" ? "Stored in this Wave" : ($0.source ?? "builtin workflow") } ?? "")
                .font(Typography.code(11)).foregroundStyle(palette.textTertiary)
                .accessibilityIdentifier("wave-workflow-source")
            if let workflow = entry?.workflow {
                WorkflowGraph(nodes: workflow.nodes, edges: workflow.edges)
            } else if !name.isEmpty {
                Text(entry?.unavailable.map { "\(name) is invalid: \($0)" }
                     ?? model.workflowCatalog.errorMessage
                     ?? (model.workflowCatalog.isLoading ? "Reading workflows…" : "\(name) names no workflow here"))
                    .font(Typography.caption())
                    .foregroundStyle(Color.statusWarning)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("wave-workflow-invalid")
            }
            if let error = model.workflowCatalog.errorMessage {
                Text(error).foregroundStyle(Color.statusWarning)
                Button("Retry") { Task { await model.loadWorkflowCatalog(force: true) } }
            }
            if let error = model.workflowErrors[wave.id] {
                Text(error)
                    .font(Typography.body(12))
                    .foregroundStyle(WorkTone.blocked.ink)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("wave-workflow-error")
            }
        }
        .accessibilityIdentifier("wave-workflow")
        .sheet(isPresented: $editingWorkflow) { WorkflowEditor(model: model, wave: wave, name: name) }
    }
}

struct WorkflowEditor: View {
    let model: WorkModel
    let wave: WaveSnapshot
    let name: String
    @Environment(\.dismiss) private var dismiss
    @State private var content = ""
    @State private var ready = false
    @State private var saving = false
    @State private var error: String?

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            Text("Edit \(name)").font(Typography.sectionTitle(22))
            TextEditor(text: $content).font(.system(.body, design: .monospaced))
                .disabled(!ready || saving)
                .accessibilityIdentifier("workflow-content")
            if let error { Text(error).foregroundStyle(Color.statusWarning) }
            HStack {
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
                Spacer()
                Button("Save") { Task {
                    saving = true
                    defer { saving = false }
                    do {
                        try await model.saveWorkflow(name, content: content, wave: wave)
                        dismiss()
                    } catch { self.error = error.localizedDescription }
                } }.disabled(!ready || saving).keyboardShortcut(.defaultAction)
            }
        }
        .padding(24).frame(width: 640, height: 480)
        .interactiveDismissDisabled(saving)
        .task {
            do { content = try await model.workflowSource(name, wave: wave); ready = true }
            catch { self.error = error.localizedDescription }
        }
    }
}
#endif
