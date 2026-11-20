#if os(macOS)
import SwiftUI
import Loopflow

struct WaveDetailReading {
    private(set) var snapshot: WaveRoadmap?

    func plan(cached: WavePlan) -> WavePlan {
        guard let snapshot else { return cached }
        return WavePlan(objective: snapshot.workMap.objective, projects: snapshot.workMap.projects)
    }

    mutating func update(_ snapshot: WaveRoadmap) {
        self.snapshot = snapshot
    }

    /// The Wave has no live reading; the planning reading's Notice says why.
    mutating func clear() {
        snapshot = nil
    }
}

/// The Wave's Project plan, Tasks and metrics, from the window's planning reading.
struct WaveDetailPane: View {
    let wave: WaveViewModel
    let repoPath: String
    /// This Wave's share of the window's planning reading, once one has arrived.
    let streamed: StreamedWaveDetail?
    let onClose: () -> Void
    let onOpenTask: (String) -> Void

    @Environment(\.palette) private var palette
    @State private var showProjects = false

    var body: some View {
        VStack(spacing: 0) {
            header
            HStack {
                Button("Projects") { showProjects = true }
            }.padding(.bottom, 8)
            Divider()
            HSplitView {
                WavePlanView(
                    plan: wave.plan ?? WavePlan(objective: ""),
                    wave: wave,
                    streamed: streamed,
                    onOpenTask: onOpenTask
                )
                .id("\(repoPath)|\(wave.id)")
                .frame(minWidth: 230, idealWidth: 320, maxWidth: 440, maxHeight: .infinity)
            }
        }
        .sheet(isPresented: $showProjects) {
            WaveProjectsView(wave: wave.name, repo: repoPath, sourceReference: nil)
        }
    }

    private var header: some View {
        HStack(spacing: Spacing.sm) {
            WaveLensView(lens: wave.lens)
            Text(wave.displayName)
                .font(Typography.sectionTitle())
                .foregroundStyle(palette.text)

            Spacer()

            Button {
                onClose()
            } label: {
                Image(systemName: "xmark")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            .buttonStyle(WorkPlainButtonStyle())
            .help("Close wave")
            .accessibilityLabel("Close wave")
        }
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.md)
    }
}

private struct WavePlanView: View {
    let plan: WavePlan
    let wave: WaveViewModel
    let streamed: StreamedWaveDetail?
    let onOpenTask: (String) -> Void

    @Environment(\.palette) private var palette
    @State private var selectedTaskID: String?
    @State private var reading = WaveDetailReading()
    @State private var historyFilter = TaskHistoryFilter()
    @State private var historyNow = Date()
    /// Closed Tasks, paged in only while completed Tasks are shown.
    @State private var closed = ClosedTaskHistory()
    // True until the first live read resolves. It gates the loading affordance,
    // so an empty plan during the pre-snapshot window reads as loading.
    @State private var isAwaitingDetail = true

    private var workMap: WaveWorkMap? { reading.snapshot?.workMap }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.xl) {
                objective
                projectAndTasks
                if let selectedTaskID, let workMap {
                    WaveWorkInspector(
                        selectedTaskID: selectedTaskID,
                        workMap: workMap,
                        onOpenTask: onOpenTask
                    )
                }
            }
            .padding(Spacing.xl)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(palette.background)
        .task(id: historyFilter) { await loadClosed(more: false) }
        .onChange(of: streamed?.sequence, initial: true) { _, _ in
            if AppTestMode.current() == .mockWaves {
                applyMockDetail()
            } else {
                applyStreamed()
            }
        }
    }

    /// Later readings arrive with each planning frame; nothing here reads on
    /// a clock.
    private func applyStreamed() {
        guard let streamed, streamed.wave == wave.id else { return }
        if let snapshot = streamed.snapshot {
            historyNow = Date()
            reading.update(snapshot)
        } else {
            reading.clear()
        }
        isAwaitingDetail = false
    }

    /// Pages closed Tasks while completed ones are shown: back to the range's
    /// start, or one more page on request.
    private func loadClosed(more: Bool) async {
        guard historyFilter.showCompleted, AppTestMode.current() != .mockWaves else {
            closed = ClosedTaskHistory()
            return
        }
        if !more { closed = ClosedTaskHistory() }
        closed.isLoading = true
        defer { closed.isLoading = false }
        let since = historyFilter.days == 0 ? nil : Date().addingTimeInterval(-Double(historyFilter.days) * 86_400)
        do {
            repeat {
                let page = try await RegistryQueryLocal.shared.closedTasks(wave: wave.id, after: closed.next, cwd: wave.repo)
                closed.append(page)
            } while !more && closed.next != nil && since.map { !closed.reaches($0) } ?? false
        } catch {
            closed.error = "Completed Tasks unavailable: \(error.localizedDescription)"
        }
    }

    private var displayedPlan: WavePlan { reading.plan(cached: plan) }
    private var objectiveText: String { displayedPlan.objective }

    /// Lead with one sentence, prominent. The full objective is disclosure, not
    /// clipped prose — and the lead is a deterministic excerpt, never a
    /// generated summary that could disagree with GOAL.md.
    private var objective: some View {
        let full = objectiveText.trimmingCharacters(in: .whitespacesAndNewlines)
        let lead = Self.firstSentence(full)
        return VStack(alignment: .leading, spacing: Spacing.sm) {
            Text(full.isEmpty ? "No objective written yet." : lead)
                .font(Typography.sectionTitle(20))
                .foregroundStyle(palette.text)
                .lineSpacing(3)
                .textSelection(.enabled)
                .accessibilityIdentifier("wave-objective-lead")

            if !full.isEmpty, full != lead {
                DisclosureGroup {
                    Text(full)
                        .font(Typography.body(13))
                        .foregroundStyle(palette.textSecondary)
                        .lineSpacing(3)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.top, Spacing.xs)
                } label: {
                    Text("Full objective")
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                }
                .tint(palette.actionInk)
            }
        }
    }

    /// Deterministic one-sentence excerpt: flatten newlines, then cut at the
    /// first sentence terminator followed by a space or end of text.
    static func firstSentence(_ text: String) -> String {
        let flat = text.split(whereSeparator: \.isNewline)
            .joined(separator: " ")
            .trimmingCharacters(in: .whitespaces)
        guard !flat.isEmpty else { return "" }
        let terminators: Set<Character> = [".", "!", "?"]
        let chars = Array(flat)
        var result = ""
        for (i, c) in chars.enumerated() {
            result.append(c)
            guard terminators.contains(c) else { continue }
            let next = i + 1 < chars.count ? chars[i + 1] : " "
            if next == " " { return result.trimmingCharacters(in: .whitespaces) }
        }
        return result.trimmingCharacters(in: .whitespaces)
    }

    private var projectAndTasks: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            ForEach(displayedPlan.projects.items) { WaveProjectView(project: $0, repo: wave.repo) }
            if isAwaitingDetail {
                WorkSectionHeading("Tasks")
                ProgressView("Loading Tasks…").accessibilityIdentifier("wave-detail-loading")
            } else if let workMap {
                switch workMap.tasks {
                case .unavailable(let reason):
                    WorkSectionHeading("Tasks")
                    Text(reason).foregroundStyle(palette.warning)
                case .available(let inventory, let truncated):
                    let tasks = closed.merged(after: inventory).filter {
                        historyFilter.includes($0, now: historyNow)
                    }
                    WorkSectionHeading(title: "Tasks", count: tasks.count) {
                        TaskHistoryControls(filter: $historyFilter)
                            .onChange(of: historyFilter) { historyNow = Date() }
                    }
                    if truncated { Text("Planning is partial; more Tasks exist.") }
                    if tasks.isEmpty { Text("No current Tasks").foregroundStyle(palette.textSecondary) }
                    ForEach(tasks) { task in RoadmapTaskView(task: task, selectedTaskID: $selectedTaskID) }
                    if historyFilter.showCompleted {
                        if let error = closed.error {
                            Text(error).foregroundStyle(palette.warning)
                        } else if closed.next != nil {
                            Button("Older completed Tasks") { Task { await loadClosed(more: true) } }
                                .disabled(closed.isLoading)
                        }
                    }
                }
            } else {
                WorkSectionHeading("Tasks")
                Text("Task status unavailable.").foregroundStyle(palette.textSecondary)
            }
        }.accessibilityIdentifier("wave-tasks")
    }

    private func applyMockDetail() {
        let outcome = MockWaveFixture.detailReading(state: MockWaveFixture.detailState)
        reading = outcome.reading
        isAwaitingDetail = outcome.awaitingFirstRead
    }
}

private struct RoadmapTaskView: View {
    let task: RoadmapTask
    @Binding var selectedTaskID: String?

    @Environment(\.palette) private var palette

    /// State, owner when owned, and who moves next.
    private var statusLine: String {
        let state = task.historyLabel
        let owner = task.runtime?.owner.map { " · \($0.summary)" } ?? ""
        return "\(state)\(owner) · next: \(task.nextMove.owner.rawValue)"
    }

    var body: some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            WaveLensView(lens: WaveLens.forTask(task.condition), diameter: 9, accessibilityId: "task-lens")
                .frame(width: 14)
                .padding(.top, 2)
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                    Text(task.task.identifier)
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                    Text(task.task.name)
                        .font(Typography.caption(12))
                        .foregroundStyle(palette.text)
                        .lineLimit(2)
                }
                Text(statusLine)
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                if let pr = task.pr {
                    PrLink(pr: pr)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(Spacing.sm)
        .background(palette.background.opacity(0.55))
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
        .overlay {
            RoundedRectangle(cornerRadius: CornerRadius.sm)
                .stroke(isSelected ? palette.selectionEdge : Color.clear, lineWidth: 1)
        }
        .contentShape(Rectangle())
        .accessibilityIdentifier("wave-task")
        .onTapGesture {
            selectedTaskID = task.task.id
        }
    }

    private var isSelected: Bool {
        selectedTaskID == task.task.id
    }
}

private struct WaveWorkInspector: View {
    let selectedTaskID: String
    let workMap: WaveWorkMap
    let onOpenTask: (String) -> Void

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack {
                Text("Selected work")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                Spacer()
            }
            if let task {
                Text("\(task.task.identifier) · \(task.task.name)")
                    .font(Typography.sectionTitle(15))
                    .foregroundStyle(palette.text)
                details(
                    status: task.historyLabel,
                    reason: task.condition.reason,
                    provider: task.runtime?.provider,
                    location: taskLocation,
                    pr: task.pr
                )
                if task.reference.workspace != nil {
                    Button("Open Task workspace") {
                        onOpenTask(task.task.id)
                    }
                        .buttonStyle(.borderedProminent)
                        .controlSize(.small)
                }
            }
        }
        .padding(Spacing.md)
        .background(palette.surfaceMuted)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    }

    private var task: RoadmapTask? {
        return workMap.tasks.items
            .first { $0.task.id == selectedTaskID }
    }

    private var taskLocation: String? {
        guard let workspace = task?.reference.workspace else { return nil }
        guard let branch = workspace.branch else { return workspace.worktree }
        return "\(workspace.worktree)\n\(branch)"
    }

    @ViewBuilder
    private func details(
        status: String,
        reason: String,
        provider: String?,
        location: String?,
        pr: PrSnapshot?
    ) -> some View {
        Text("\(status) · \(reason)")
            .font(Typography.caption(11))
            .foregroundStyle(palette.textSecondary)
        if let provider {
            Text("Provider · \(provider)")
                .font(Typography.caption(10))
                .foregroundStyle(palette.textSecondary)
        }
        if let location {
            Text(location)
                .font(.system(size: 10, design: .monospaced))
                .foregroundStyle(palette.textSecondary)
                .textSelection(.enabled)
        }
        if let pr {
            PrLink(pr: pr)
        }
    }
}

private struct PrLink: View {
    let pr: PrSnapshot

    @Environment(\.palette) private var palette

    var body: some View {
        if let github = pr.publication?.github {
            Link(
                "PR #\(github.number) · \(pr.phase.rawValue)",
                destination: github.url
            )
            .font(Typography.caption(10))
        } else {
            Text("Pull request · \(pr.phase.rawValue) · \(pr.branch)")
                .font(Typography.caption(10))
                .foregroundStyle(palette.textSecondary)
        }
    }
}

struct WaveProjectView: View {
    let project: ProjectPlanningSnapshot
    let repo: String
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline) {
                Text(project.name).font(Typography.sectionTitle(15)).textSelection(.enabled)
                Text(project.status.rawValue.capitalized)
                    .font(Typography.caption()).foregroundStyle(palette.textSecondary)
            }
            TrackerSyncView(sync: project.sync, repo: repo)
        }.accessibilityIdentifier("wave-project")
    }
}

#endif
