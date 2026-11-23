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

/// The Wave's Project plan and Tasks, from the window's planning reading.
struct WaveDetailPane: View {
    let wave: WaveViewModel
    let repoPath: String
    /// This Wave's share of the window's planning reading, once one has arrived.
    let streamed: StreamedWaveDetail?
    let onClose: () -> Void
    let onOpenTask: (String) -> Void
    let onTaskAction: (RoadmapTask, RoadmapTaskAction) -> Void
    let onOpenWorktree: (TaskWorktreeSnapshot) -> Void

    @Environment(\.palette) private var palette
    @State private var showProjects = false

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            HSplitView {
                WavePlanView(
                    plan: wave.plan ?? WavePlan(objective: ""),
                    wave: wave,
                    streamed: streamed,
                    onOpenTask: onOpenTask,
                    onTaskAction: onTaskAction,
                    onOpenWorktree: onOpenWorktree
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
            StatusMark(lens: wave.lens)
            Text(wave.displayName)
                .serif(.display)
                .foregroundStyle(palette.emphasisInk)
                .lineLimit(1)
                .accessibilityIdentifier("wave-page-title")

            Spacer()

            Button("Projects") { showProjects = true }
                .buttonStyle(LoopflowButtonStyle())

            Button {
                onClose()
            } label: {
                Image(systemName: "xmark")
                    .font(Typography.meta)
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
    let onTaskAction: (RoadmapTask, RoadmapTaskAction) -> Void
    let onOpenWorktree: (TaskWorktreeSnapshot) -> Void

    @Environment(\.palette) private var palette
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
            closed.error = error.localizedDescription
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
                .font(Typography.lede)
                .foregroundStyle(palette.text)
                .lineSpacing(3)
                .textSelection(.enabled)
                .accessibilityIdentifier("wave-objective-lead")

            if !full.isEmpty, full != lead {
                DisclosureGroup {
                    Text(full)
                        .font(Typography.text)
                        .foregroundStyle(palette.textSecondary)
                        .lineSpacing(3)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.top, Spacing.xs)
                } label: {
                    Text("Full objective")
                        .font(Typography.meta)
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
                    FailureLine("Tasks did not load.", machine: reason)
                case .available(let inventory, let truncated):
                    let tasks = closed.merged(after: inventory).filter {
                        historyFilter.includes($0, now: historyNow)
                    }
                    WorkSectionHeading(title: "Tasks", count: tasks.count) {
                        TaskHistoryControls(filter: $historyFilter)
                            .onChange(of: historyFilter) { historyNow = Date() }
                    }
                    if truncated { Text("Planning is partial; more Tasks exist.") }
                    if tasks.isEmpty { Text("No current Tasks.").emptyLine() }
                    ForEach(tasks) { task in
                        PortfolioTaskRow(task: task, onOpen: { onOpenTask(task.id) },
                            onAction: { action in onTaskAction(task, action) }, onOpenWorktree: onOpenWorktree)
                    }
                    if historyFilter.showCompleted {
                        if let error = closed.error {
                            FailureLine("Completed Tasks did not load.", machine: error)
                        } else if closed.next != nil {
                            Button("Older completed Tasks") { Task { await loadClosed(more: true) } }
                                .disabled(closed.isLoading)
                        }
                    }
                }
            } else {
                WorkSectionHeading("Tasks")
                Text("Task status unavailable.").emptyLine()
            }
        }.accessibilityIdentifier("wave-tasks")
    }

    private func applyMockDetail() {
        let outcome = MockWaveFixture.detailReading(state: MockWaveFixture.detailState)
        reading = outcome.reading
        isAwaitingDetail = outcome.awaitingFirstRead
    }
}

struct WaveProjectView: View {
    let project: ProjectPlanningSnapshot
    let repo: String
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            HStack(alignment: .firstTextBaseline) {
                Text(project.name).font(Typography.lede).textSelection(.enabled)
                Text(project.status.rawValue.capitalized)
                    .font(Typography.meta).foregroundStyle(palette.textSecondary)
            }
            TrackerSyncView(sync: project.sync, repo: repo)
        }.accessibilityIdentifier("wave-project")
    }
}

#endif
