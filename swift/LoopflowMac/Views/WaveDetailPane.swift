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

/// One Wave surface: current Project/Task state beside the durable conversation.
/// `lf status` supplies the work map; the Wave listener streams ordered chat and
/// child activity from its journal.
struct WaveDetailPane: View {
    let wave: WaveViewModel
    let repoPath: String
    let onClose: () -> Void

    @Environment(\.palette) private var palette
    @State private var selection: WaveWorkSelection?
    @State private var prefill: WaveComposerPrefill?
    @State private var workRefresh: UInt64 = 0
    @StateObject private var terminalStore = TaskTerminalStore.shared

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
            Image(systemName: wave.statusIndicator.icon)
                .foregroundStyle(wave.statusIndicator.color)
                .accessibilityLabel(wave.statusText)
            Text(wave.displayName)
                .font(Typography.sectionTitle())
                .foregroundStyle(palette.text)
            Text("WaveChat")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            Spacer()

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
    @State private var workMap: WaveWorkMap?
    @State private var workMapError: String?

    private var identity: String { "\(repoPath)|\(wave.id)" }

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
                WaveTrajectorySection(
                    waveName: wave.name,
                    repoPath: repoPath,
                    homeLikelyLive: wave.status == .running,
                    refreshSignal: refreshSignal
                )
            }
            .padding(Spacing.xl)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(palette.background)
        .task(id: identity) {
            while !Task.isCancelled {
                await refreshWorkMap()
                try? await Task.sleep(for: .seconds(30))
            }
        }
        .task(id: refreshSignal) { await refreshWorkMap() }
    }

    private var objective: some View {
        let text = workMap?.objective ?? plan.objective
        return VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Objective")
                .font(Typography.caption(10))
                .fontWeight(.medium)
                .foregroundStyle(palette.textSecondary)

            Text(text.isEmpty ? "No objective written yet." : text)
                .font(Typography.body(14))
                .foregroundStyle(palette.text)
                .lineSpacing(3)
                .textSelection(.enabled)
        }
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

            if let workMapError {
                Label(workMapError, systemImage: "exclamationmark.triangle")
                    .font(Typography.caption(11))
                    .foregroundStyle(Color.statusError)
                    .textSelection(.enabled)
            } else if let workMap, !workMap.projects.isEmpty {
                LazyVStack(alignment: .leading, spacing: Spacing.md) {
                    ForEach(workMap.projects) { project in
                        WaveProjectWorkView(
                            project: project,
                            selection: $selection
                        )
                    }
                }
            } else if plan.projects.isEmpty {
                Text("No live projects.")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            } else {
                LazyVStack(alignment: .leading, spacing: Spacing.md) {
                    ForEach(plan.projects) { project in
                        WaveProjectView(project: project)
                    }
                }
            }
        }
    }

    private func refreshWorkMap() async {
        guard wave.isRegistered else {
            workMap = nil
            workMapError = nil
            return
        }
        do {
            let snapshot = try await RegistryQueryLocal.shared.status(
                wave: wave.name,
                cwd: repoPath
            )
            workMap = snapshot.workMap
            workMapError = nil
        } catch {
            workMap = nil
            workMapError = "Work map unavailable: \(error.localizedDescription)"
        }
    }
}

private struct WaveProjectWorkView: View {
    let project: WaveProjectWork
    @Binding var selection: WaveWorkSelection?

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                Text(project.project.name)
                    .font(Typography.sectionTitle(17))
                    .foregroundStyle(palette.text)
                Spacer()
                Text(project.runtime?.status.rawValue ?? "unstarted")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
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
        .onTapGesture {
            selection = WaveWorkSelection(kind: .project, id: project.project.slug)
        }
    }

    private var isSelected: Bool {
        selection == WaveWorkSelection(kind: .project, id: project.project.slug)
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
    }
}

private struct WaveTaskWorkView: View {
    let task: WaveTaskWork
    @Binding var selection: WaveWorkSelection?

    @Environment(\.palette) private var palette

    var body: some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            Image(systemName: task.task.completed ? "checkmark.circle.fill" : "circle")
                .font(Typography.caption(11))
                .foregroundStyle(task.task.completed ? palette.accent : palette.textSecondary)
                .frame(width: 14)
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
                    reason: task.nextMove.reason,
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

/// The Wave's curated **trajectory**: its memory facts with the evidence that
/// justifies each, plus a compose field to record a new evidence-backed note.
/// Reads are daemon-less (`lf memory log --json` folds the journal, so the list
/// renders whether or not the Home is live); the write goes through the Wave's
/// live Home (`lf memory add`), so a stopped Home surfaces the Start-on-Home
/// hint instead of failing silently. No Mac-only model — it decodes the shared
/// `MemoryFact`/`Receipt` and renders.
private struct WaveTrajectorySection: View {
    let waveName: String
    let repoPath: String
    /// The Wave's coarse liveness from its status row — a hint for the compose
    /// gate. The authoritative signal is still the write's own error: recording
    /// through a stopped Home throws, and that error is what the field shows.
    let homeLikelyLive: Bool
    let refreshSignal: UInt64

    @Environment(\.palette) private var palette
    @State private var facts: [MemoryFact]?
    @State private var loadError: String?
    @State private var draft: String = ""
    @State private var evidence: String = ""
    @State private var isRecording = false
    @State private var recordError: String?

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            Text("Trajectory")
                .font(Typography.caption(10))
                .fontWeight(.medium)
                .foregroundStyle(palette.textSecondary)

            if let loadError {
                Label(loadError, systemImage: "exclamationmark.triangle")
                    .font(Typography.caption(11))
                    .foregroundStyle(Color.statusWarning)
                    .textSelection(.enabled)
            }

            if let facts {
                if facts.isEmpty {
                    Text("No trajectory recorded yet.")
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                } else {
                    LazyVStack(alignment: .leading, spacing: Spacing.sm) {
                        ForEach(Array(facts.enumerated()), id: \.offset) { _, fact in
                            TrajectoryNoteView(fact: fact)
                        }
                    }
                }
            } else if loadError == nil {
                ProgressView()
                    .controlSize(.small)
            }

            composer
        }
        .task(id: identity) { await load() }
        .task(id: refreshSignal) { await load() }
    }

    private var identity: String { "\(repoPath)|\(waveName)" }

    private var composer: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            TextField("Record a trajectory note…", text: $draft, axis: .vertical)
                .textFieldStyle(.plain)
                .font(Typography.body(12))
                .lineLimit(1...4)
                .padding(Spacing.sm)
                .background(palette.surfaceMuted)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
            TextField("Evidence receipts, e.g. pr:owner/repo#12 (space-separated, optional)", text: $evidence)
                .textFieldStyle(.plain)
                .font(.system(size: 10, design: .monospaced))
                .padding(Spacing.sm)
                .background(palette.surfaceMuted)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
            HStack(spacing: Spacing.sm) {
                if !homeLikelyLive {
                    Text("Recording needs the Wave's Home running.")
                        .font(Typography.caption(9))
                        .foregroundStyle(palette.textSecondary)
                }
                Spacer()
                if isRecording {
                    ProgressView().controlSize(.small)
                }
                Button("Record") { Task { await record() } }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    .disabled(isRecording || draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
            if let recordError {
                Text(recordError)
                    .font(Typography.caption(9))
                    .foregroundStyle(Color.statusError)
                    .textSelection(.enabled)
            }
        }
    }

    @MainActor
    private func load() async {
        do {
            facts = try await RegistryQueryLocal.shared.memoryLog(wave: waveName, cwd: repoPath)
            loadError = nil
        } catch {
            // Keep the last-good list; only the banner changes (RoadmapView idiom).
            loadError = "Trajectory unavailable: \(error.localizedDescription)"
        }
    }

    @MainActor
    private func record() async {
        let fact = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !fact.isEmpty else { return }
        let receipts = evidence
            .split(whereSeparator: \.isWhitespace)
            .map(String.init)
        isRecording = true
        defer { isRecording = false }
        do {
            try await RegistryQueryLocal.shared.memoryAdd(
                wave: waveName,
                fact: fact,
                receipts: receipts,
                cwd: repoPath
            )
            draft = ""
            evidence = ""
            recordError = nil
            await load()
        } catch {
            recordError = recordFailureHint(error)
        }
    }

    /// A stopped Home is the common recording failure — the write has no offline
    /// path — so point at the Home control rather than echoing the raw error.
    private func recordFailureHint(_ error: Error) -> String {
        let raw = error.localizedDescription
        if raw.contains("no live listener") || raw.localizedCaseInsensitiveContains("listener") {
            return "Start the Wave's Home to record trajectory."
        }
        return raw
    }
}

private struct TrajectoryNoteView: View {
    let fact: MemoryFact

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text(fact.fact)
                .font(Typography.body(12))
                .foregroundStyle(palette.text)
                .lineSpacing(2)
                .textSelection(.enabled)
            if !fact.receipts.isEmpty {
                HStack(spacing: Spacing.xs) {
                    ForEach(Array(fact.receipts.enumerated()), id: \.offset) { _, receipt in
                        receiptChip(receipt)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(Spacing.sm)
        .background(palette.surfaceMuted.opacity(0.6))
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
    }

    @ViewBuilder
    private func receiptChip(_ receipt: Receipt) -> some View {
        if let url = receipt.browserURL {
            Link(receipt.token, destination: url)
                .font(.system(size: 9, design: .monospaced))
                .padding(.horizontal, Spacing.xs)
                .padding(.vertical, 1)
                .background(palette.accent.opacity(0.12))
                .clipShape(Capsule())
        } else {
            Text(receipt.token)
                .font(.system(size: 9, design: .monospaced))
                .foregroundStyle(palette.textSecondary)
                .padding(.horizontal, Spacing.xs)
                .padding(.vertical, 1)
                .background(palette.surfaceMuted)
                .clipShape(Capsule())
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
