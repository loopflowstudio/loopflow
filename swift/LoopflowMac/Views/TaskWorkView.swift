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

    private var lifecycleLabel: String {
        switch task.lifecycle {
        case .done: "Completed"
        case .abandoned: "Canceled"
        default: task.lifecycle.label.capitalized
        }
    }

    /// What the Task's Workflow reading says, or why there is none yet.
    private var position: String? {
        if let workflow = work.value?.graph { return workflow.sentence }
        if work.value != nil {
            // The shared resolver reports the first-run default and its owner.
            return task.graphName.map { "\($0) · \(task.graphSource ?? "")" } ?? "No workflow selected"
        }
        return work.isLoading ? task.setupPosition ?? "Reading workflow…" : nil
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.md) {
                Text(lifecycleLabel)
                    .fontWeight(task.isCompleted ? .semibold : .regular)
                    .foregroundStyle(task.lifecycle == .done ? palette.success
                        : task.lifecycle == .abandoned ? palette.failure : palette.textSecondary)
                    .accessibilityIdentifier("task-lifecycle-status")
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
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
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

/// Auto-Ship at the right of the Workflow bar: an ordinary button. Off, it is
/// `lf task auto on`, which also runs the Task: its run takes the interactive
/// steps and lands it. On, it is `lf task auto off`. Rust decides whether auto
/// can start. Sidebar rows and menus carry no auto control.
struct TaskAutoControl: View {
    let model: WorkModel
    let task: RoadmapTask

    /// The Task reading's own mark; until it arrives, the state its row carries.
    var on: Bool {
        guard let execution = model.taskExecution[task.id].value else { return task.selectionStates.contains(.auto) }
        return execution.auto?.state == .on
    }

    var body: some View {
        if on || !task.isCompleted {
            let issue = task.task.identifier
            // Rust's refusal to turn it on again never disables turning it off.
            let refusal = on ? model.changeRefusal : model.runRefusal(task)
            Button(on ? "Stop Auto-Ship" : "Auto-Ship") {
                if on { model.turnAutoOff(task) } else { model.turnAutoOn(task) }
            }
            .buttonStyle(LoopflowButtonStyle())
            .fixedSize()
            .disabled(refusal != nil)
            .help(refusal ?? (on
                ? "lf task auto off \(issue): the run waits at its next interactive step for you, and auto-merge is disarmed"
                : "lf task auto on \(issue): run every remaining step and land it"))
            .accessibilityIdentifier("task-auto")
        }
    }
}

/// The selected Task's Workflow under the jump bar: the workflow line, the one
/// step that moves it from here, Auto-Ship, then "…" with every other Task
/// action. Rust owns the graph, the position, the sentence and whether a run
/// can start; this only arranges them.
struct TaskWorkflowBar: View {
    let model: WorkModel
    let task: RoadmapTask
    /// The Task part of the workspace stream, as last read.
    let work: Reading<TaskOverview>
    var newConversation: (() -> Void)?
    var openSession: ((SessionRecord) -> Void)?
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.md) {
            if let workflow = work.value?.graph {
                WorkflowLine(graph: workflow, auto: model.taskExecution[task.id].value?.auto?.state == .on,
                             setup: task.setupPosition)
            } else {
                Text(placeholder)
                    .lineLimit(1).truncationMode(.tail)
                    .foregroundStyle(palette.textSecondary)
                    .accessibilityIdentifier("task-workflow-bar-live")
            }
            Spacer(minLength: 0)
            if let next {
                Button(next.label) { model.startTaskFromInput(nil, task: task) }
                    .buttonStyle(LoopflowButtonStyle())
                    .fixedSize()
                    .disabled(model.runRefusal(task) != nil)
                    .help(model.runRefusal(task) ?? next.help)
                    .accessibilityIdentifier(next.identifier)
            }
            TaskAutoControl(model: model, task: task)
            Menu {
                TaskActionItems(model: model, task: task, wave: model.task(id: task.id)?.wave?.wave,
                                work: work.value, newConversation: newConversation, openSession: openSession)
            } label: {
                Image(systemName: "ellipsis").font(Typography.text)
            }
            .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden)
            .fixedSize()
            .help("Task actions")
            .accessibilityLabel("Task actions")
            .accessibilityIdentifier("task-actions")
        }
        .padding(.horizontal, 14)
        .frame(height: 34)
        .font(Typography.text)
        .background(palette.background)
        .overlay(alignment: .bottom) { Rectangle().fill(palette.border).frame(height: 1) }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("task-workflow-bar")
        // "Start with…" in "…" lists the workflows a Task without one can take up.
        .task(id: work.value != nil && work.value?.graph == nil) {
            if work.value != nil, work.value?.graph == nil { await model.loadGraphCatalog() }
        }
    }

    /// What the bar says before the Task has a Graph.
    private var placeholder: String {
        if work.value == nil { return work.isLoading ? task.setupPosition ?? "Reading workflow…" : "" }
        return task.graphName.map { "Takes up \($0) when it starts" } ?? "No workflow selected"
    }

    /// The step that moves the Task from here: Start before it has a Graph,
    /// else what `lf task run` does from where it stands.
    private var next: (label: String, help: String, identifier: String)? {
        let issue = task.task.identifier
        if let workflow = work.value?.graph {
            return Self.runAction(workflow, issue: issue).map { ($0.label, $0.help, "task-workflow-run") }
        }
        // Only a successful reading with no Workflow offers Start; Rust says when it can't.
        guard work.value != nil else { return nil }
        return ("Start", "lf task run \(issue)", "task-workflow-start")
    }

    /// What `lf task run` does from here, named for it; `nil` while a run
    /// carries the Task or once it is at `end`.
    static func runAction(_ workflow: TaskGraph, issue: String) -> (label: String, help: String)? {
        let help = "lf task run \(issue)"
        switch workflow.position {
        case .at(TaskGraph.start): return ("Start", help)
        case .at(TaskGraph.end): return nil
        case .at(let step): return ("Run", "\(help): continue after \(step)")
        case .run(_, _, _, true): return nil
        case .run(_, let at, _, false): return ("Run", "\(help): run again at \(at)")
        }
    }

}

/// An unstarted Task's Session slot. Showing it starts nothing; Start runs the
/// Task's Workflow through the same path as `lf task run`.
struct TaskStartOffer: View {
    let model: WorkModel
    let task: RoadmapTask

    var body: some View {
        SessionSlotOffer(
            title: "Not started", systemImage: "play.circle",
            action: "Start", actionImage: "play.fill",
            note: "Starts this Task’s Workflow and its first conversation.",
            unavailable: model.runRefusal(task), help: "lf task run \(task.task.identifier)",
            accessibilityLabel: "Start \(task.task.identifier)",
            actionIdentifier: "task-start-\(task.id)",
            perform: { model.startTaskFromInput(nil, task: task) }
        ) {
            Text(task.task.description.isEmpty ? "No description recorded." : task.task.description)
                .lineLimit(8)
        }
        .accessibilityIdentifier("task-start-offer-\(task.id)")
    }
}

/// Every Task action: the workflow bar's "…", sidebar context menus,
/// Dashboard tiles and ⌘/ list the same items.
struct TaskActionItems: View {
    let model: WorkModel
    let task: RoadmapTask
    let wave: WaveSnapshot?
    /// The Task's Workflow reading the menu's host shows, so both agree.
    let work: TaskOverview?
    var newConversation: (() -> Void)?
    var openSession: ((SessionRecord) -> Void)?
    @State private var copied: String?

    private var unavailable: Bool { model.changeRefusal != nil }

    var body: some View {
        if let workflow = work?.graph {
            // Start the next run at a step: back, ahead, or waiting at an interactive step.
            Menu("Start from…") {
                ForEach([TaskGraph.start] + workflow.interactiveSteps.map(\.name), id: \.self) { step in
                    Button(step) { model.startTaskFromInput(nil, from: step, task: task) }
                        .accessibilityIdentifier("task-workflow-move-\(step)")
                }
            }
            .disabled(model.runRefusal(task) != nil)
            .help(model.runRefusal(task) ?? "lf task run \(task.task.identifier) --from <step>")
            .accessibilityIdentifier("task-workflow-move")
        } else if work != nil, !task.isCompleted {
            Menu("Start with…") {
                StartFlowItems(model: model, chosen: task.graphName) { model.startTaskFromInput($0, task: task) }
            }
            .disabled(model.runRefusal(task) != nil)
            .accessibilityIdentifier("task-workflow-choose")
        }
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
        if task.isCompleted {
            Button("Reopen Task") { model.reopenTask(task) }.disabled(unavailable)
                .accessibilityIdentifier("task-reopen")
        } else {
            Button("Complete Task") { model.completeTaskFromInput(task) }.disabled(unavailable)
                .accessibilityIdentifier("task-complete")
            Button(task.runtime?.owner == nil ? "Claim Task" : "Unassign Task") {
                model.assignTask(task, claimed: task.runtime?.owner == nil)
            }
            .disabled(unavailable)
            .help(task.runtime?.owner == nil ? "lf task claim: you own it on this Machine"
                : "lf task unassign: nobody owns it, so nothing moves it; its checkout, PR and history stay")
            .accessibilityIdentifier("task-claim-unassign")
        }
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
        if !task.isCompleted {
            Divider()
            Button("Abandon…", role: .destructive) { model.beginAbandon(task) }
                .disabled(unavailable)
                .accessibilityIdentifier("task-abandon")
        }
    }

    private func copy(_ value: String, key: String) {
        NSPasteboard.general.clearContents()
        if NSPasteboard.general.setString(value, forType: .string) { copied = key }
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
    var execution: TaskLocationObservation? = nil
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
        let readsWindow = (execution == nil || execution?.machineID == model.observingMachineId) && query.remoteMachine == nil && location != nil
            && location?.machineId == model.observingMachineId && location == task?.reference.workspace?.identity
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
                                 ?? "No workflow selected")
                                .accessibilityIdentifier("task-run-position")
                        } else {
                            Text(task?.setupPosition ?? "Reading workflow…")
                        }
                        Spacer()
                        if let openFiles { Button("Files", action: openFiles) }
                    }.padding(.vertical, 8)
                }
                if !readsWindow {
                    Button("Refresh") { refresh += 1 }
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
                        .padding(.vertical, 8)
                        .accessibilityIdentifier("task-earlier-runs")
                    }
                    if graphs.isEmpty {
                        Text("No Graph has run.").padding(.vertical, 6)
                            .accessibilityIdentifier("task-graph-processes-empty")
                    }
                } else if readError == nil {
                    Text(task == nil ? "Task unavailable" : "Reading Graph processes…").padding(.vertical, 6)
                }
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
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
                let ownerQuery: RegistryQuery
                let subject: String
                if let execution {
                    guard case .recorded(let ownerTask, _) = execution.location else {
                        throw RegistryQueryError("Task log location is unavailable.")
                    }
                    ownerQuery = query.onMachine(execution.machineID, repository: execution.repositoryID).pinTaskLocation(to: execution)
                    subject = ownerTask
                } else {
                    guard let location, location.machineId == model.savedMachineId,
                          query.remoteMachine == nil else {
                        throw RegistryQueryError("Task log location is unavailable; no local fallback was used.")
                    }
                    ownerQuery = query
                    subject = task?.runtime?.workId ?? taskId
                }
                let subscription = try await ownerQuery.subscribeTask(task: subject)
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
            WorkSectionHeading(title: "Work") {
                Button("Refresh") { model.rescanWork() }
            }
            if let work = model.taskExecution[task.id].value?.work {
                ForEach(work.sessions) { session in
                    row(session.interactive ? "Session" : "Run", id: session.id, name: session.title,
                        state: session.completedAt == nil ? "open" : "completed")
                }
                if work.sessions.isEmpty {
                    Text("No recorded work.")
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
            if expanded { detail.padding(.leading, 18) }
        }
        .padding(.vertical, 6)
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
