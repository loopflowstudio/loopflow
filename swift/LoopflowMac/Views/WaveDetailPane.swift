#if os(macOS)
import SwiftUI
import Loopflow

struct WaveComposerPrefill: Equatable {
    let id: UUID
    let text: String
}

struct WaveWorkSelection: Equatable {
    let kind: ChildActivitySubject
    let id: String
}

struct WaveDetailReading {
    private(set) var snapshot: WaveDetailSnapshot?
    private(set) var errorMessage: String?

    mutating func update(_ snapshot: WaveDetailSnapshot) {
        self.snapshot = snapshot
        errorMessage = nil
    }

    mutating func recordFailure(_ error: Error) {
        errorMessage = "Wave status unavailable: \(error.localizedDescription)"
    }

    mutating func clear() {
        snapshot = nil
        errorMessage = nil
    }
}

/// One Wave surface: current Project/Task state beside the durable conversation.
/// `lf status` supplies the work map; the Wave listener streams ordered chat and
/// child activity from its journal.
struct WaveDetailPane: View {
    let wave: WaveViewModel
    let repoPath: String
    let onClose: () -> Void

    @Environment(\.palette) private var palette
    @Environment(\.openWindow) private var openWindow
    @State private var selection: WaveWorkSelection?
    @State private var prefill: WaveComposerPrefill?
    @State private var workRefresh: UInt64 = 0
    @State private var showsControl = false
    // A shared singleton is externally owned, so it observes as an @ObservedObject.
    // Wrapping it in @StateObject installs StateObject's create-and-own lifecycle
    // during the first body pass, which fires the singleton's publisher mid-eval —
    // an AttributeGraph dependency cycle at cold launch and sheet presentation.
    @ObservedObject private var terminalStore = TaskTerminalStore.shared

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            HSplitView {
                WavePlanView(
                    plan: wave.plan ?? WavePlan(objective: ""),
                    wave: wave,
                    repoPath: repoPath,
                    selection: $selection,
                    refreshSignal: workRefresh,
                    onTellWave: tellWave,
                    terminalStore: terminalStore
                )
                .frame(minWidth: 230, idealWidth: 320, maxWidth: 440, maxHeight: .infinity)

                WaveChatView(
                    repoPath: repoPath,
                    waveName: wave.name,
                    prefill: prefill,
                    onSelectChild: { selection = $0 },
                    onChildActivity: { workRefresh &+= 1 }
                )
                    .frame(minWidth: 340, maxWidth: .infinity, maxHeight: .infinity)
            }
        }
    }

    private func tellWave(_ selection: WaveWorkSelection) {
        self.selection = selection
        let noun = selection.kind == .project ? "Project" : "Task"
        prefill = WaveComposerPrefill(
            id: UUID(),
            text: "Regarding \(noun) \(selection.id): "
        )
    }

    private var header: some View {
        HStack(spacing: Spacing.sm) {
            WaveLensView(lens: wave.lens)
            Text(wave.displayName)
                .font(Typography.sectionTitle())
                .foregroundStyle(palette.text)

            Spacer()

            Button {
                openWindow(
                    id: "context-lab",
                    value: ContextLabRoute.wave(repoPath: repoPath, wave: wave.name)
                )
            } label: {
                Label("Context Lab", systemImage: "text.magnifyingglass")
                    .font(Typography.caption())
            }
            .buttonStyle(.borderless)
            .help("Study the instructions seen by this Wave's agent sessions")
            .accessibilityIdentifier("wave-context-lab")

            Button {
                showsControl = true
            } label: {
                Image(systemName: "slider.horizontal.3")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            .buttonStyle(.plain)
            .help("Open Control")
            .accessibilityLabel("Open Control")
            .accessibilityIdentifier("wave-control-button")

            Button {
                onClose()
            } label: {
                Image(systemName: "xmark")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            .buttonStyle(.plain)
            .help("Close wave")
            .accessibilityLabel("Close wave")
        }
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.md)
        .sheet(isPresented: $showsControl) {
            ControlView { showsControl = false }
        }
    }
}

private struct WavePlanView: View {
    let plan: WavePlan
    let wave: WaveViewModel
    let repoPath: String
    @Binding var selection: WaveWorkSelection?
    let refreshSignal: UInt64
    let onTellWave: (WaveWorkSelection) -> Void
    @ObservedObject var terminalStore: TaskTerminalStore

    @Environment(\.palette) private var palette
    @State private var reading = WaveDetailReading()
    // True until the first live read resolves. It gates the loading affordance,
    // so an empty projects area during the pre-snapshot window reads as
    // "loading" rather than "no projects".
    @State private var isAwaitingDetail = true

    private var identity: String { "\(repoPath)|\(wave.id)" }
    private var refreshIdentity: String { "\(identity)|\(refreshSignal)" }
    private var workMap: WaveWorkMap? { reading.snapshot?.workMap }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.xl) {
                objective
                projects
                if let selection, let workMap {
                    WaveWorkInspector(
                        selection: selection,
                        workMap: workMap,
                        repoPath: repoPath,
                        onTellWave: onTellWave,
                        terminalStore: terminalStore
                    )
                }
                liveStatusFooter
            }
            .padding(Spacing.xl)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(palette.background)
        .task(id: refreshIdentity) {
            while !Task.isCancelled {
                await refreshDetail()
                try? await Task.sleep(for: .seconds(30))
            }
        }
    }

    private var objectiveText: String { workMap?.objective ?? plan.objective }

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
                .tint(palette.accent)
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

    private var projects: some View {
        let projectCount = workMap?.projects.count ?? plan.projects.count
        return VStack(alignment: .leading, spacing: Spacing.md) {
            HStack(spacing: Spacing.sm) {
                Text("Projects")
                    .font(Typography.caption(10))
                    .fontWeight(.medium)
                    .foregroundStyle(palette.textSecondary)

                Text("\(projectCount)")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .padding(.horizontal, Spacing.sm)
                    .padding(.vertical, Spacing.xxs)
                    .background(palette.surfaceMuted)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
            }

            if let workMap, !workMap.projects.isEmpty {
                LazyVStack(alignment: .leading, spacing: Spacing.md) {
                    ForEach(workMap.projects) { project in
                        WaveProjectWorkView(
                            project: project,
                            selection: $selection
                        )
                    }
                }
            } else if plan.projects.isEmpty {
                if isAwaitingDetail {
                    HStack(spacing: Spacing.sm) {
                        ProgressView().controlSize(.small)
                        Text("Loading live detail…")
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                    }
                    .accessibilityIdentifier("wave-detail-loading")
                } else {
                    Text("No projects yet.")
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                }
            } else {
                LazyVStack(alignment: .leading, spacing: Spacing.md) {
                    ForEach(plan.projects) { project in
                        WaveProjectView(project: project)
                    }
                }
            }
        }
        .accessibilityIdentifier("wave-projects")
    }

    /// Live-status failures are operational detail, not primary hierarchy: a
    /// quiet footer says the plan is showing cached and hides the raw reason
    /// behind disclosure. The plan above still renders from the cached `WavePlan`.
    @ViewBuilder
    private var liveStatusFooter: some View {
        if let errorMessage = reading.errorMessage {
            DisclosureGroup {
                Text(errorMessage)
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.top, Spacing.xxs)
            } label: {
                Label("Showing cached plan · live status unavailable", systemImage: "arrow.triangle.2.circlepath")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
            }
            .tint(palette.textSecondary)
            .accessibilityIdentifier("wave-live-status-footer")
        }
    }

    private func refreshDetail() async {
        if AppTestMode.current() == .mockWaves {
            applyMockDetail()
            return
        }
        guard wave.isRegistered else {
            reading.clear()
            isAwaitingDetail = false
            return
        }
        do {
            let snapshot = try await RegistryQueryLocal.shared.status(
                wave: wave.name,
                cwd: repoPath
            )
            guard !Task.isCancelled else { return }
            reading.update(snapshot)
        } catch {
            guard !Task.isCancelled else { return }
            reading.recordFailure(error)
        }
        isAwaitingDetail = false
    }

    /// The `mock-waves` detail rendering: the fixture owns the state→reading
    /// decision (see `MockWaveFixture.detailReading`); the view just applies it.
    private func applyMockDetail() {
        let outcome = MockWaveFixture.detailReading(
            waveName: wave.name,
            state: MockWaveFixture.detailState
        )
        reading = outcome.reading
        isAwaitingDetail = outcome.awaitingFirstRead
    }
}

private struct WaveProjectWorkView: View {
    let project: WaveProjectWork
    @Binding var selection: WaveWorkSelection?

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                WaveLensView(lens: projectLens, diameter: 10, accessibilityId: "project-lens")
                    .alignmentGuide(.firstTextBaseline) { $0[.bottom] - 2 }

                Text(project.project.name)
                    .font(Typography.sectionTitle(17))
                    .foregroundStyle(palette.text)

                Text(openTaskLabel)
                    .font(Typography.caption(10))
                    .fontWeight(.medium)
                    .foregroundStyle(palette.textSecondary)
                    .padding(.horizontal, Spacing.sm)
                    .padding(.vertical, Spacing.xxs)
                    .background(palette.surfaceMuted)
                    .clipShape(Capsule())
                    .accessibilityIdentifier("project-open-tasks")

                Spacer()

                if let status = project.runtime?.status.rawValue {
                    Text(status)
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                }
            }

            if !project.project.definition.isEmpty {
                Text(project.project.definition)
                    .font(Typography.body(13))
                    .foregroundStyle(palette.textSecondary)
                    .lineSpacing(2)
                    .textSelection(.enabled)
            }

            if !project.project.krs.isEmpty {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    ForEach(project.project.krs) { kr in
                        proofRow(text: kr.text, holds: kr.holds)
                    }
                }
            }

            VStack(alignment: .leading, spacing: Spacing.xs) {
                ForEach(project.tasks) { task in
                    WaveTaskWorkView(
                        task: task,
                        selection: $selection
                    )
                }
            }

            Text("Next: \(project.nextMove.owner.rawValue) · \(project.nextMove.reason)")
                .font(Typography.caption(10))
                .foregroundStyle(palette.textSecondary)
            if let directive = project.directive {
                directiveStatus(directive)
            }
        }
        .padding(Spacing.md)
        .background(palette.surfaceMuted.opacity(0.65))
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        .overlay {
            RoundedRectangle(cornerRadius: CornerRadius.md)
                .stroke(isSelected ? palette.accent : Color.clear, lineWidth: 1)
        }
        .contentShape(Rectangle())
        .accessibilityIdentifier("wave-project")
        .onTapGesture {
            selection = WaveWorkSelection(kind: .project, id: project.project.slug)
        }
    }

    private var isSelected: Bool {
        selection == WaveWorkSelection(kind: .project, id: project.project.slug)
    }

    /// The Project's lens, derived from its shared runtime and its Tasks'
    /// attention evidence — the same grammar the Wave and Task rows use.
    private var projectLens: WaveLens {
        WaveLens.forProject(runtime: project.runtime, tasks: project.tasks)
    }

    private var openTaskLabel: String {
        let open = project.tasks.filter { !$0.task.completed }.count
        return open == 1 ? "1 open task" : "\(open) open tasks"
    }

    private func directiveStatus(_ directive: WorkDirectiveSnapshot) -> some View {
        Text("Direction v\(directive.version) · \(directive.incorporatedAt == nil ? "pending incorporation" : "incorporated")")
            .font(Typography.caption(10))
            .foregroundStyle(directive.incorporatedAt == nil ? palette.textSecondary : palette.accent)
    }

    private func proofRow(text: String, holds: Bool) -> some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            Image(systemName: holds ? "checkmark.circle.fill" : "circle")
                .font(Typography.caption(11))
                .foregroundStyle(holds ? palette.accent : palette.textSecondary)
                .frame(width: 14)
                .accessibilityHidden(true)
            Text(text)
                .font(Typography.caption(12))
                .foregroundStyle(palette.text)
                .lineSpacing(2)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(text)
        .accessibilityValue(holds ? "Holds" : "Open")
        .accessibilityIdentifier("project-key-result")
    }
}

private struct WaveTaskWorkView: View {
    let task: WaveTaskWork
    @Binding var selection: WaveWorkSelection?

    @Environment(\.palette) private var palette

    var body: some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            WaveLensView(lens: WaveLens.forTask(task.attention), diameter: 9, accessibilityId: "task-lens")
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
                Text("\(task.runtime?.status.rawValue ?? "unstarted") · next: \(task.nextMove.owner.rawValue)")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                if let directive = task.directive {
                    Text("direction v\(directive.version) · \(directive.incorporatedAt == nil ? "pending" : "incorporated")")
                        .font(Typography.caption(10))
                        .foregroundStyle(directive.incorporatedAt == nil ? palette.textSecondary : palette.accent)
                }
                ForEach(task.prs) { pr in
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
                .stroke(isSelected ? palette.accent : Color.clear, lineWidth: 1)
        }
        .contentShape(Rectangle())
        .accessibilityIdentifier("wave-task")
        .onTapGesture {
            selection = WaveWorkSelection(kind: .task, id: task.task.identifier)
        }
    }

    private var isSelected: Bool {
        selection == WaveWorkSelection(kind: .task, id: task.task.identifier)
    }
}

private struct WaveWorkInspector: View {
    let selection: WaveWorkSelection
    let workMap: WaveWorkMap
    let repoPath: String
    let onTellWave: (WaveWorkSelection) -> Void
    @ObservedObject var terminalStore: TaskTerminalStore

    @Environment(\.palette) private var palette
    @State private var showsTaskWorkspace = false

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack {
                Text("Selected work")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                Spacer()
                Button("Tell Wave about this") { onTellWave(selection) }
                    .buttonStyle(.borderless)
                    .font(Typography.caption(10))
            }
            if let project {
                Text(project.project.name)
                    .font(Typography.sectionTitle(15))
                    .foregroundStyle(palette.text)
                details(
                    directive: project.directive,
                    status: project.runtime?.status.rawValue ?? "unstarted",
                    reason: project.nextMove.reason,
                    provider: project.runtime?.provider,
                    location: nil,
                    prs: []
                )
            } else if let task {
                Text("\(task.task.identifier) · \(task.task.name)")
                    .font(Typography.sectionTitle(15))
                    .foregroundStyle(palette.text)
                details(
                    directive: task.directive,
                    status: task.runtime?.status.rawValue ?? "unstarted",
                    reason: task.attention.reason,
                    provider: task.runtime?.provider,
                    location: taskLocation,
                    prs: task.prs
                )
                if task.reference.workspace != nil {
                    Button("Open Task workspace") { showsTaskWorkspace = true }
                        .buttonStyle(.borderedProminent)
                        .controlSize(.small)
                }
            }
        }
        .padding(Spacing.md)
        .background(palette.surfaceMuted)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        .sheet(isPresented: $showsTaskWorkspace) {
            if let task {
                TaskWorkspaceView(
                    task: task.task,
                    reference: task.reference,
                    runtime: task.runtime,
                    attention: task.attention,
                    repoPath: repoPath,
                    terminalStore: terminalStore
                )
            }
        }
    }

    private var project: WaveProjectWork? {
        guard selection.kind == .project else { return nil }
        return workMap.projects.first { $0.project.slug == selection.id || $0.project.id == selection.id }
    }

    private var task: WaveTaskWork? {
        guard selection.kind == .task else { return nil }
        return workMap.projects
            .flatMap(\.tasks)
            .first { $0.task.identifier == selection.id || $0.task.id == selection.id }
    }

    private var taskLocation: String? {
        guard let workspace = task?.reference.workspace else { return nil }
        guard let branch = workspace.branch else { return workspace.worktree }
        return "\(workspace.worktree)\n\(branch)"
    }

    @ViewBuilder
    private func details(
        directive: WorkDirectiveSnapshot?,
        status: String,
        reason: String,
        provider: String?,
        location: String?,
        prs: [PrSnapshot]
    ) -> some View {
        Text("\(status) · \(reason)")
            .font(Typography.caption(11))
            .foregroundStyle(palette.textSecondary)
        if let directive {
            Text("Direction v\(directive.version)")
                .font(Typography.caption(10))
                .foregroundStyle(palette.textSecondary)
            Text(directive.text)
                .font(Typography.body(12))
                .foregroundStyle(palette.text)
                .textSelection(.enabled)
            Text(directive.incorporatedAt == nil ? "Awaiting incorporation" : "Incorporated")
                .font(Typography.caption(10))
                .foregroundStyle(directive.incorporatedAt == nil ? palette.textSecondary : palette.accent)
        }
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
        ForEach(prs) { pr in
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
                "PR #\(github.number) · \(pr.phase.rawValue)\(pr.publication?.afterMerge == .completeTask ? " · completes Task" : "")",
                destination: github.url
            )
            .font(Typography.caption(10))
        } else {
            Text("PR \(pr.sequence) · \(pr.phase.rawValue)\(pr.publication?.afterMerge == .completeTask ? " · completes Task" : "") · \(pr.branch)")
                .font(Typography.caption(10))
                .foregroundStyle(palette.textSecondary)
        }
    }
}

private struct WaveProjectView: View {
    let project: WaveProject

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text(project.title)
                .font(Typography.sectionTitle(17))
                .foregroundStyle(palette.text)

            if let definition = project.definition {
                Text(definition)
                    .font(Typography.body(13))
                    .foregroundStyle(palette.textSecondary)
                    .lineSpacing(2)
                    .textSelection(.enabled)
            }

            if !project.krs.isEmpty {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    ForEach(project.krs) { kr in
                        HStack(alignment: .top, spacing: Spacing.sm) {
                            Image(systemName: kr.proof == .holds ? "checkmark.circle.fill" : "circle")
                                .font(Typography.caption(11))
                                .foregroundStyle(kr.proof == .holds ? palette.accent : palette.textSecondary)
                                .frame(width: 14)
                                .accessibilityHidden(true)

                            Text(kr.text)
                                .font(Typography.caption(12))
                                .foregroundStyle(palette.text)
                                .lineSpacing(2)
                                .textSelection(.enabled)
                        }
                        .accessibilityElement(children: .combine)
                        .accessibilityLabel(kr.text)
                        .accessibilityValue(kr.proof == .holds ? "Holds" : "Open")
                    }
                }
                .padding(.top, Spacing.xs)
            }
        }
        .padding(Spacing.md)
        .background(palette.surfaceMuted.opacity(0.65))
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    }
}

#endif
