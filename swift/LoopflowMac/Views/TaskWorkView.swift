import Loopflow
import SwiftUI

/// Where the Task stands, in its pane and on its details page: its lifecycle
/// word and Rust's sentence. Its actions are the workflow bar's.
struct TaskWorkflowHeader: View {
    let model: WorkModel
    let task: RoadmapTask
    /// The Task part of the workspace stream, as last read.
    let work: Reading<TaskOverview>
    @Environment(\.palette) private var palette

    /// What the Task's Workflow reading says, or why there is none yet.
    private var position: String? {
        if let workflow = work.value?.graph { return workflow.sentence }
        if work.value != nil {
            return model.workflowSelection(task)
        }
        return work.isLoading ? task.setupPosition ?? "Reading workflow…" : nil
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.md) {
                // The word the Task's row and the title bar say.
                let standing = TaskRowStatus(task: task, status: model.programStatus(task: task.id))
                if let word = standing.word {
                    Text(verbatim: word)
                        .fontWeight(task.isCompleted ? .semibold : .regular)
                        .foregroundStyle(standing.tone?.ink(in: palette) ?? palette.textSecondary)
                        .help(Text(verbatim: standing.detail ?? word))
                        .accessibilityIdentifier("task-lifecycle-status")
                }
                if task.isUnassigned {
                    Text("Unassigned").foregroundStyle(palette.textSecondary)
                }
                if let position {
                    Text(position)
                        .foregroundStyle(palette.textSecondary)
                        .accessibilityIdentifier("task-workflow-position")
                }
            }
            TaskPlanningFactsView(task: task)
            // Completion may fail after the Workflow has already arrived.
            if let conflict = task.runtime?.completionPending {
                Text(conflict)
                    .foregroundStyle(palette.failure)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("task-completion-pending")
            }
        }
        .font(Typography.text)
        .padding(.horizontal, Spacing.md)
        .padding(.vertical, Spacing.sm)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background {
            palette.background
            if task.isCompleted {
                (task.lifecycle == .done ? palette.success : palette.failure).opacity(0.14)
            }
        }
        .overlay(alignment: .leading) {
            if task.isCompleted {
                Rectangle()
                    .fill(task.lifecycle == .done ? palette.success : palette.failure)
                    .frame(width: 3)
            }
        }
        .overlay(alignment: .bottom) { Rectangle().fill(palette.border).frame(height: 1) }
        .accessibilityIdentifier("task-workflow")
        .onChange(of: work, initial: true) { _, reading in
            if reading.value != nil { Perf.stateObserved(Perf.workflowReady, id: task.id, owner: model.navigation.performanceOwner) }
        }
        .onChange(of: task.id) { _, id in
            if work.value != nil { Perf.stateObserved(Perf.workflowReady, id: id, owner: model.navigation.performanceOwner) }
        }
    }
}

/// The selected Task's Graph and the controls for its saved position.
struct TaskWorkflowBar: View {
    let model: WorkModel
    let task: RoadmapTask
    /// The Task part of the workspace stream, as last read.
    let work: Reading<TaskOverview>
    @Environment(\.palette) private var palette

    var body: some View {
        let actions = TaskActions(work: work.value, row: task, execution: model.taskExecution[task.id].value,
                                  outline: model.workflowOutline(task.graphName),
                                  autoPending: model.pendingChanges.autoRequest(for: task.task.identifier))
        TaskWorkflowControls(task: task, actions: actions, pending: model.pendingChanges.controlRequest(for: task.task.identifier),
                             run: { model.startTaskFromInput(nil, from: $0, task: task) },
                             pause: { model.setPause($0, task: task) }, interrupt: { model.interruptStep(task) },
                             toggleAuto: { if actions.auto == .on { model.turnAutoOff(task) } else { model.turnAutoOn(task) } },
                             complete: { model.beginComplete(task) }) {
            TaskLifecycleItems(model: model, task: task)
            if let overview = work.value, overview.graph == nil, !task.isCompleted {
                TaskStartGraphMenu(model: model, task: task)
            }
        }
        .disabled(model.changeRefusal != nil)
        .padding(.horizontal, Spacing.md)
        .frame(height: 34)
        .font(Typography.text)
        .background(palette.background)
        .overlay(alignment: .bottom) { Rectangle().fill(palette.border).frame(height: 1) }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("task-workflow-bar")
        // A Task without a Graph draws its workflow from the catalog, and "Start with…" lists the others.
        .task(id: work.value != nil && work.value?.graph == nil) {
            if work.value != nil, work.value?.graph == nil { await model.loadGraphCatalog() }
        }
    }
}

private struct TaskStartGraphMenu: View {
    let model: WorkModel
    let task: RoadmapTask

    var body: some View {
        Menu("Start with…") {
            StartFlowItems(model: model, chosen: task.graphName) { model.startTaskFromInput($0, task: task) }
        }
        .disabled(model.runRefusal(task) != nil)
        .accessibilityIdentifier("task-workflow-choose")
    }
}

/// An unstarted Task's Session slot, in the New Task sheet's words: the
/// workflow Start will run, whose choice it is and where a person takes part.
/// Showing it starts nothing; Start runs the same path as `lf task run`.
struct TaskStartOffer: View {
    let model: WorkModel
    let task: RoadmapTask
    @Environment(\.compactPanes) private var compact

    private var sentence: String? {
        let parts = [model.workflowSelection(task).map { "It will run \($0)." },
                     model.workflowOutline(task.graphName)?.sentence].compactMap { $0 }
        return parts.isEmpty ? nil : parts.joined(separator: " ")
    }

    var body: some View {
        let refusal = model.runRefusal(task)
        // The brief beside the pane and a Dashboard tile's header already name the Task.
        EmptyState(title: "Not started", sentence: sentence) {
            Button("Start") { model.startTaskFromInput(nil, task: task) }
                .buttonStyle(LoopflowButtonStyle(.accent))
                .disabled(refusal != nil)
                .help(refusal ?? "lf task run \(task.task.identifier)")
                .accessibilityLabel("Start \(task.task.identifier)")
                .accessibilityIdentifier("task-start-\(task.id)")
            if !compact {
                Menu("Start with…") {
                    StartFlowItems(model: model, chosen: task.graphName) { model.startTaskFromInput($0, task: task) }
                }
                .menuStyle(.borderlessButton).fixedSize()
                .disabled(refusal != nil)
                .accessibilityIdentifier("task-start-with-\(task.id)")
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("task-start-offer-\(task.id)")
        .task { await model.loadGraphCatalog() }
    }
}

/// The complete action list for sidebar rows, Dashboard tiles and ⌘/.
struct TaskActionItems: View {
    let model: WorkModel
    let task: RoadmapTask
    let wave: WaveSnapshot?
    /// The Task's Workflow reading the menu's host shows, so both agree.
    let work: TaskOverview?
    var newConversation: (() -> Void)?
    var openSession: ((SessionRecord) -> Void)?
    var body: some View {
        let actions = TaskActions(work: work, row: task, execution: model.taskExecution[task.id].value)
        if work?.graph != nil {
            Menu("Start from…") {
                ForEach(actions.steps, id: \.self) { step in
                    Button(step) { model.startTaskFromInput(nil, from: step, task: task) }
                        .accessibilityIdentifier("task-workflow-move-\(step)")
                }
            }
            .disabled(model.runRefusal(task) != nil || actions.control?.action == .pause)
            .accessibilityIdentifier("task-workflow-move")
        } else if work != nil, !task.isCompleted {
            TaskStartGraphMenu(model: model, task: task)
        }
        if let action = actions.pause {
            Button(action == .pause ? "Pause" : "Unpause") {
                model.setPause(action, task: task)
            }
            .disabled(model.changeRefusal != nil || model.pendingChanges.controlRequest(for: task.task.identifier) != nil || task.runControl.pausing)
        }
        TaskLifecycleItems(model: model, task: task)
        if !task.isCompleted {
            Button(model.hasOpenChildren(task) ? "Complete Task & Settle Children…" : "Complete Task") {
                model.beginComplete(task)
            }
            .disabled(model.changeRefusal != nil)
            .accessibilityIdentifier("task-complete")
        }
        TaskMetadataItems(model: model, task: task, wave: wave, newConversation: newConversation, openSession: openSession)
    }
}

private struct TaskMetadataItems: View {
    let model: WorkModel
    let task: RoadmapTask
    let wave: WaveSnapshot?
    var newConversation: (() -> Void)?
    var openSession: ((SessionRecord) -> Void)?
    @State private var copied: String?
    private var unavailable: Bool { model.changeRefusal != nil }

    var body: some View {
        Button("New conversation") { newConversation?() }.disabled(newConversation == nil || unavailable)
        Menu("Move to Wave") {
            Button("No Wave") { model.moveTask(task, toWave: nil) }
                .disabled(task.task.waveId == nil)
            ForEach(model.visibleRoadmaps.filter { $0.wave.repo == task.task.repo }, id: \.wave.id) { destination in
                Button(destination.wave.displayName) {
                    model.moveTask(task, toWave: destination.wave.id)
                }.disabled(destination.wave.id == task.task.waveId)
            }
        }.disabled(unavailable).accessibilityIdentifier("task-move-wave")
        if let wave { Button("Open owning Wave") { model.select(.wave(id: wave.id)) } }
        Divider()
        Button(copied == "id" ? "Copied Task ID" : "Copy Task ID") { copy(task.task.identifier, key: "id") }
        if let url = task.reference.issueUrl {
            Button(copied == "link" ? "Copied Task link" : "Copy Task link") { copy(url.absoluteString, key: "link") }
        }
        if let branch = task.reference.workspace?.branch {
            Button(copied == "branch" ? "Copied branch" : "Copy branch") { copy(branch, key: "branch") }
        }
        if let pr = task.pr?.publication?.github { Link("Open PR #\(pr.number)", destination: pr.url) }
        ForEach(model.taskSessions(task.id)) { session in
            Button("Open \(session.title)") { openSession?(session) }.disabled(openSession == nil)
            if !session.taskPrimary, session.titleSource != .unavailable {
                Button("Rename \(session.title)…") { model.beginSessionRename(session) }
            }
            if session.interactive, session.work?.kind != .task {
                Button("Bind \(session.title) to Task…") { model.beginSessionBinding(session) }
            } else if session.interactive, !session.taskPrimary {
                Button("Make \(session.title) primary…") { model.beginSessionBinding(session) }
            }
        }
    }

    private func copy(_ value: String, key: String) {
        NSPasteboard.general.clearContents()
        if NSPasteboard.general.setString(value, forType: .string) { copied = key }
    }
}

private struct TaskLifecycleItems: View {
    let model: WorkModel
    let task: RoadmapTask
    private var unavailable: Bool { model.changeRefusal != nil }

    var body: some View {
        if task.isCompleted {
            Button("Reopen Task") { model.reopenTask(task) }.disabled(unavailable)
                .accessibilityIdentifier("task-reopen")
        } else {
            Button(task.runtime?.owner == nil ? "Claim Task" : "Unassign Task") {
                model.assignTask(task, claimed: task.runtime?.owner == nil)
            }
            .disabled(unavailable)
            .help(task.runtime?.owner == nil ? "lf task claim: you own it on this Machine"
                : "lf task unassign: nobody owns it, so nothing moves it; its checkout, PR and history stay")
            .accessibilityIdentifier("task-claim-unassign")
            Divider()
            Button("Abandon…", role: .destructive) { model.beginAbandon(task) }
                .disabled(unavailable)
                .accessibilityIdentifier("task-abandon")
        }
    }
}

/// The window's one Abandon… confirmation, whichever menu asked for it.
struct TaskAbandonConfirmation: ViewModifier {
    let model: WorkModel

    private var abandoning: RoadmapTask? {
        model.navigation.abandoning.flatMap { model.task(id: $0)?.task }
    }

    func body(content: Content) -> some View {
        content.confirmationDialog(abandoning.map { "Abandon \($0.task.identifier)?" } ?? "Abandon Task?",
            isPresented: Binding(get: { abandoning != nil }, set: { if !$0 { model.navigation.abandoning = nil } }),
            presenting: abandoning
        ) { task in
            Button("Abandon Task", role: .destructive) { model.abandonTask(task) }
        } message: { _ in
            Text("Cancels the Task, closes its PRs and deletes its branches. Unassign it instead to keep its work for later.")
        }
    }
}

/// The window's one Complete… confirmation when completing a parent will settle open children.
struct TaskCompleteConfirmation: ViewModifier {
    let model: WorkModel

    private var completing: RoadmapTask? {
        model.navigation.completing.flatMap { model.task(id: $0)?.task }
    }

    private func summary(for descendants: [RoadmapTask]) -> (done: [RoadmapTask], abandoned: [RoadmapTask]) {
        let done = descendants.filter { $0.pr?.phase == .merged }
        let abandoned = descendants.filter { $0.pr?.phase != .merged }
        return (done, abandoned)
    }

    private func message(for descendants: [RoadmapTask]) -> String {
        let (done, abandoned) = summary(for: descendants)
        var lines: [String] = []
        if !done.isEmpty {
            let names = done.map(\.task.identifier).joined(separator: ", ")
            lines.append("Marked done (PR merged): \(names).")
        }
        if !abandoned.isEmpty {
            let names = abandoned.map(\.task.identifier).joined(separator: ", ")
            lines.append("Abandoned (PRs closed, branches deleted, cannot be reopened): \(names).")
        }
        return lines.joined(separator: "\n")
    }

    func body(content: Content) -> some View {
        let descendants = completing.map { model.openDescendants(of: $0) } ?? []
        let (_, abandoned) = summary(for: descendants)
        let title: String = if let task = completing {
            descendants.count == 1
                ? "Complete \(task.task.identifier) and settle 1 open child?"
                : "Complete \(task.task.identifier) and settle \(descendants.count) open children?"
        } else {
            "Complete Task?"
        }
        content.confirmationDialog(
            title,
            isPresented: Binding(get: { completing != nil }, set: { if !$0 { model.navigation.completing = nil } }),
            presenting: completing
        ) { task in
            Button(
                abandoned.isEmpty ? "Complete & Finish Children" : "Complete & Settle Children",
                role: abandoned.isEmpty ? nil : .destructive
            ) {
                model.navigation.completing = nil
                model.completeTaskFromInput(task)
            }
        } message: { _ in
            Text(message(for: descendants))
        }
    }
}

/// The Task's due date, beside the conversation and in Task details.
struct TaskPlanningFactsView: View {
    let task: RoadmapTask
    @Environment(\.palette) private var palette

    var body: some View {
        if let due = task.task.dueDate {
            Text("Due \(due)")
                .font(Typography.text)
                .foregroundStyle(palette.textSecondary)
        }
    }
}

/// Every Graph process in the Task's checkout, newest first, whatever started
/// it: one line each, opening to its recorded loops and steps. It is not the
/// Workflow's history: a process `lf task run` started and one started ad hoc
/// are the same kind of row. A multiplexer pane on the page surface.
struct GraphProcessLog: View {
    let model: WorkModel
    let taskId: String
    let location: WorkspaceIdentity?
    var query: RegistryQuery = RegistryQueryLocal.shared
    var openFiles: (() -> Void)? = nil
    /// The Task page prints the sentence in its workflow header above, so its log has no header row.
    var showsPosition = true
    @State private var captured: WorkFrame.TaskPart?
    @State private var error: String?
    @State private var refresh = 0
    @Environment(\.palette) private var palette

    var body: some View {
        let task = model.task(id: taskId)?.task
        let reading = task.map { model.taskWork[$0.id] }
        // This Mac's window already reads its own Tasks; any other is read
        // through the server from wherever it runs.
        let readsWindow = location != nil && location?.machineId == model.observingMachineId
            && location == task?.reference.workspace?.identity
        let work = readsWindow ? reading?.value : captured?.work
        let graphs = readsWindow ? work?.graphProcesses : captured?.graphProcesses.map(\.entry)
        let details = readsWindow ? model.graphProcesses : Dictionary(uniqueKeysWithValues:
            (captured?.graphProcesses ?? []).map { ($0.entry.id, $0) })
        let readError = readsWindow ? reading?.failure : error
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if showsPosition {
                    HStack {
                        if let work {
                            Text(work.graph?.sentence ?? (readsWindow ? task?.execution : captured?.row?.execution)?.reason
                                 ?? task.flatMap(model.workflowSelection) ?? "")
                                .accessibilityIdentifier("task-run-position")
                        } else {
                            Text(task?.setupPosition ?? "Reading workflow…")
                        }
                        Spacer()
                        if let openFiles { Button("Files", action: openFiles).buttonStyle(LoopflowButtonStyle()) }
                    }.padding(.vertical, Spacing.sm)
                }
                if !readsWindow {
                    Button("Refresh") { refresh += 1 }.buttonStyle(LoopflowButtonStyle())
                }
                if let graphs {
                    if let graph = graphs.last {
                        GraphProcessView(model: model, graph: graph,
                            run: details[graph.id],
                            startedBy: readsWindow ? task.flatMap { model.taskExecution[$0.id].value?.startedBy } : nil)
                    }
                    if graphs.count > 1 {
                        DisclosureGroup("Earlier runs (\(graphs.count - 1))") {
                            ForEach(Array(graphs.dropLast().reversed())) { graph in
                                GraphProcessView(model: model, graph: graph,
                                    run: details[graph.id])
                            }
                        }
                        .padding(.vertical, Spacing.sm)
                        .accessibilityIdentifier("task-earlier-runs")
                    }
                    if graphs.isEmpty {
                        Text("No Graph has run.").emptyLine()
                            .accessibilityIdentifier("task-graph-processes-empty")
                            .padding(.vertical, Spacing.xs)
                    }
                } else if readError == nil {
                    Text(task == nil ? "Task unavailable." : "Reading Graph processes…").emptyLine().padding(.vertical, Spacing.xs)
                }
            }
            .padding(.horizontal, Spacing.md)
            .padding(.vertical, Spacing.sm)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .font(Typography.text)
        .foregroundStyle(palette.textSecondary)
        .textSelection(.enabled)
        .background(palette.background)
        .environment(\.colorScheme, .light)
        .accessibilityIdentifier("task-graph-processes")
        .onChange(of: error) { old, new in model.reported("graph-processes", "Graph processes", from: old, to: new) }
        .task(id: refresh) {
            guard !readsWindow else { return }
            do {
                let subject = task?.runtime?.workId ?? taskId
                let subscription = try query.subscribeTask(task: subject)
                do {
                    for try await line in subscription.lines {
                        try Task.checkCancellation()
                        let frame = try WorkFrame.decode(part: .task,
                            scope: WorkScope(repo: nil, headless: false, task: subject, activity: nil, taskListWave: nil), sequence: 0, line: line)
                        guard case .task(let reading?) = frame.content else {
                            self.error = frame.unavailable ?? "Task reading is unavailable"
                            continue
                        }
                        captured = reading
                        error = nil
                    }
                } catch {
                    await subscription.cancel()
                    throw error
                }
                await subscription.cancel()
            } catch is CancellationError {
                return
            } catch { self.error = error.localizedDescription }
        }
    }
}

/// The shown Task's recorded work, from its status subscription, behind a disclosure.
struct TaskDiagnosticsView: View {
    let model: WorkModel
    let task: RoadmapTask
    @State private var expanded = false

    var body: some View {
        DisclosureGroup("Debug", isExpanded: $expanded) {
            if expanded { TaskWorkView(model: model, task: task) }
        }
    }
}

/// The Task's conversations, including headless work and
/// conversations without turns: raw records, shown under Debug. Its Graph
/// processes are listed by `GraphProcessLog`.
struct TaskWorkView: View {
    let model: WorkModel
    let task: RoadmapTask
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            let sessions = model.taskExecution[task.id].value?.work.sessions
            WorkSectionHeading(title: "Work") {
                if sessions?.isEmpty != true { Button("Refresh") { model.rescanWork() }.buttonStyle(LoopflowButtonStyle()) }
            }
            if let sessions {
                ForEach(sessions) { session in
                    row(session.interactive ? "Session" : "Run", id: session.id, name: session.title,
                        state: session.completedAt == nil ? "open" : "completed")
                }
                if sessions.isEmpty {
                    HStack(spacing: Spacing.sm) {
                        Text("No recorded work.")
                        Button("Refresh") { model.rescanWork() }.buttonStyle(LoopflowButtonStyle())
                    }
                }
            }
        }
        .font(Typography.text)
        .foregroundStyle(palette.textSecondary)
        .textSelection(.enabled)
        .accessibilityIdentifier("task-work")
    }

    private func row(_ kind: String, id: String, name: String, state: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
            Text(kind)
            Text(name).lineLimit(2)
            Spacer()
            Text(state)
            Text(id).font(Typography.mono).help(id)
        }
        .accessibilityIdentifier("task-work-\(id)")
    }
}

/// One Graph process of the Task: its Graph, state, when it started and how long
/// it ran, opening to its id, launched graph, where it stands and each step
/// it started. Every process reads the same way, whoever started it.
struct GraphProcessView: View {
    let model: WorkModel
    let graph: GraphProcessInventoryEntry
    let run: GraphProcessDetail?
    var startedBy: Starter? = nil
    private var expanded: Bool { model.navigation.expandedGraphProcesses.contains(graph.id) }
    @Environment(\.palette) private var palette

    private var started: Int64 { run?.steps.first?.startedAt ?? graph.updatedAt }

    /// How long the process ran; a running one has no length yet.
    static func duration(_ seconds: Int64) -> String {
        let seconds = max(0, seconds)
        if seconds < 60 { return "\(seconds)s" }
        if seconds < 3600 { return "\(seconds / 60)m \(seconds % 60)s" }
        return "\(seconds / 3600)h \(seconds % 3600 / 60)m"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Button {
                if expanded { model.navigation.expandedGraphProcesses.remove(graph.id) }
                else { model.navigation.expandedGraphProcesses.insert(graph.id) }
            } label: {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                    Image(systemName: expanded ? "chevron.down" : "chevron.right")
                        .font(Typography.meta).fontWeight(.semibold)
                        .frame(width: 10)
                        .accessibilityHidden(true)
                    Text(graph.name).font(Typography.mono).foregroundStyle(palette.text)
                    Text(graph.state == .current ? "running" : graph.state.rawValue)
                        .foregroundStyle(tone?.ink(in: palette) ?? palette.textSecondary)
                    Spacer()
                    if let startedBy {
                        Text("started by \(startedBy.label)").lineLimit(1).truncationMode(.middle)
                            .help("started by \(startedBy.label)")
                            .accessibilityIdentifier("graph-process-started-by-\(graph.id)")
                    }
                    Text(Date(timeIntervalSince1970: TimeInterval(started))
                        .formatted(date: .abbreviated, time: .shortened))
                    if let ended = graph.endedAt {
                        Text(Self.duration(ended - started)).monospacedDigit()
                            .frame(minWidth: 52, alignment: .trailing)
                    }
                }
                .lineLimit(1)
                .contentShape(Rectangle())
            }
            .buttonStyle(WorkPlainButtonStyle())
            .accessibilityValue(expanded ? "Expanded" : "Collapsed")
            .accessibilityIdentifier("task-work-\(graph.id)")
            if expanded { detail.padding(.leading, Spacing.lg) }
        }
        .padding(.vertical, Spacing.xs)
    }

    private var tone: WorkTone? {
        switch graph.state {
        case .current: .running
        case .stopped: .stopped
        case .completed: nil
        }
    }

    @ViewBuilder
    private var detail: some View {
        Text(graph.id).font(Typography.mono).foregroundStyle(palette.textTertiary)
            .accessibilityIdentifier("graph-process-id-\(graph.id)")
        if let run {
            Text([run.presentation.reason, graphIterationLabel(run.iterations)]
                .compactMap { $0 }.joined(separator: " · "))
                .accessibilityIdentifier("graph-process-status-\(graph.id)")
            GraphRunHistory(steps: run.steps)
        } else {
            Text("Reading Graph…")
        }
    }
}


/// Leaf history stays inspectable without a terminal or window. No summary is
/// invented from an exit: only the recorded step facts appear here.
struct GraphRunHistory: View {
    let steps: [GraphStepProcess]

    var body: some View {
        ForEach(GraphStepGroup.groups(steps)) { group in
            DisclosureGroup(group.label) {
                ForEach(group.steps) { step in
                    GraphRunStep(step: step)
                }
            }
            .accessibilityIdentifier("graph-process-loop-\(group.id)")
        }
    }
}

struct GraphRunStep: View {
    let step: GraphStepProcess

    var body: some View {
        DisclosureGroup {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                Text(step.lfProcessId).font(Typography.mono)
                Text("Started " + Date(timeIntervalSince1970: TimeInterval(step.startedAt))
                    .formatted(date: .abbreviated, time: .standard))
                if let ended = step.completedAt {
                    Text("Ended " + Date(timeIntervalSince1970: TimeInterval(ended))
                        .formatted(date: .abbreviated, time: .standard))
                }
                if let code = step.exitCode { Text("Exit \(code)") }
            }
            .textSelection(.enabled)
        } label: {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                Text(step.label).font(Typography.mono)
                Spacer()
                Text(step.outcome ?? (step.completedAt == nil ? "running" : "no recorded exit"))
                if let completed = step.completedAt {
                    Text(GraphProcessView.duration(completed - step.startedAt)).monospacedDigit()
                }
            }
        }
        .accessibilityIdentifier("graph-process-step-\(step.lfProcessId)")
    }
}
