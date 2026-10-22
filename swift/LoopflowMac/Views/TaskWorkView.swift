import Loopflow
import SwiftUI

/// Keep current Workflow position/actions compact; disclose its graph in Overview.
/// Rust owns the graph, position and legal actions.
struct TaskWorkflowHeader: View {
    let model: WorkModel
    let task: RoadmapTask
    /// The Task part of the workspace stream, as last read.
    let work: WorkReading<TaskOverview>
    var workspaceActions: AnyView? = nil
    @Environment(\.palette) private var palette

    @State private var confirmingAbandon = false
    @State private var expandedTasks: Set<String> = []

    private var controls: some View {
            HStack(alignment: .center, spacing: Spacing.md) {
                if let label = task.task.terminalLabel {
                    Text(label)
                        .fontWeight(.semibold)
                        .foregroundStyle(task.task.isSuccessful ? palette.success : palette.failure)
                        .accessibilityIdentifier("task-lifecycle-status")
                }
                if task.runtime?.status == .paused {
                    Text("Paused").foregroundStyle(palette.textSecondary)
                } else if task.task.terminalLabel == nil, let state = task.task.state {
                    Text(state.capitalized).foregroundStyle(palette.textSecondary)
                        .accessibilityIdentifier("task-lifecycle-status")
                }
                if let workflow = work.value?.workflow {
                    Text(positionLabel(workflow))
                        .lineLimit(1)
                        .foregroundStyle(palette.textSecondary)
                        .accessibilityIdentifier("task-workflow-position")
                    // Routes into `end` are one Ship it button.
                    ForEach(workflow.outgoing, id: \.self) { index in
                        if workflow.edges.indices.contains(index), workflow.edges[index].to != "end" {
                            let edge = workflow.edges[index]
                            Button(edge.name) { Task { await model.startTask(edge.name, task: task) } }
                                .buttonStyle(WorkOutlineButtonStyle(emphasized: true))
                                .disabled(unavailable != nil)
                                .help(unavailable ?? "lf task run \(task.task.identifier) \(edge.name)")
                                .accessibilityIdentifier("task-workflow-action-\(index)")
                        }
                    }
                    shipControl
                    Spacer(minLength: 0)
                    // Put the Task at a node without running anything.
                    Menu("Move to") {
                        ForEach(["start"] + workflow.nodes.map(\.name) + ["end"], id: \.self) { node in
                            Button(node) { Task { await model.moveTask(to: node, task: task) } }
                                .accessibilityIdentifier("task-workflow-move-\(node)")
                        }
                    }
                    .fixedSize()
                    .disabled(acting || readingUnavailable != nil)
                    .help("lf task workflow move \(task.task.identifier) <node>")
                    .accessibilityIdentifier("task-workflow-move")

                } else if work.value != nil, work.errorMessage == nil {
                    // The shared resolver reports the first-run default and its owner.
                    Text(task.workflowName.map { "\($0) · \(task.workflowSource ?? "")" } ?? "No workflow selected")
                        .foregroundStyle(palette.textSecondary)
                    Menu("Choose workflow") {
                        ForEach(model.workflowCatalog.value ?? [], id: \.name) { choice in
                            Button(choice.name) { Task { await model.startTask(choice.name, task: task) } }
                        }
                    }.task { await model.loadWorkflowCatalog() }
                    Button("Start") { Task { await model.startTask(nil, task: task) } }
                        .buttonStyle(WorkOutlineButtonStyle())
                        .disabled(unavailable != nil)
                        .help(unavailable ?? "lf task run \(task.task.identifier)")
                        .accessibilityIdentifier("task-workflow-start")
                    shipControl
                    Spacer()
                } else {
                    Text(work.errorMessage.map { "Workflow could not be read: \($0)" }
                         ?? "Reading workflow…")
                        .foregroundStyle(palette.textSecondary)
                    Spacer()
                }
                if task.task.isTerminal || task.runtime?.status.isTerminal == true {
                    Button("Reopen") { Task { await model.reopenTask(task) } }
                        .disabled(acting || readingUnavailable != nil)
                        .accessibilityIdentifier("task-reopen")
                } else {
                    Button("Complete") { Task { await model.completeTask(task) } }
                        .disabled(acting || readingUnavailable != nil)
                        .accessibilityIdentifier("task-complete")
                    Button(task.runtime?.status == .paused ? "Resume" : "Pause") {
                        Task { await model.pauseTask(task, paused: task.runtime?.status != .paused) }
                    }
                    .disabled(acting || readingUnavailable != nil)
                    .accessibilityIdentifier("task-pause-resume")
                    Button("Abandon…", role: .destructive) { confirmingAbandon = true }
                        .disabled(acting || readingUnavailable != nil)
                        .accessibilityIdentifier("task-abandon")
                }
                workspaceActions
                if acting {
                    ProgressView().controlSize(.small)
                    Text(draft?.progress ?? "Updating Task…").foregroundStyle(palette.textSecondary)
                }
            }
            .lineLimit(1)
            .frame(minHeight: 28)
    }

    /// Ship it, or what shipping is doing now. Rust decides whether it can start.
    @ViewBuilder private var shipControl: some View {
        if let shipping = task.shipping {
            Text("\(shipping.label) · \(shipping.reason)")
                .foregroundStyle(palette.textSecondary)
                .accessibilityIdentifier("task-shipping")
            Button("Stop shipping") { Task { await model.stopShipping(task) } }
                .buttonStyle(WorkOutlineButtonStyle())
                .disabled(acting || readingUnavailable != nil)
                .help("lf --task \(task.task.identifier) ship --stop: return it to working and disarm auto-merge")
                .accessibilityIdentifier("task-ship-stop")
        } else if !(task.task.isTerminal || task.runtime?.status.isTerminal == true) {
            let refusal = readingUnavailable ?? task.shipControl.unavailable
            Button("Ship it") { Task { await model.shipTask(task) } }
                .buttonStyle(WorkOutlineButtonStyle(emphasized: true))
                .disabled(acting || refusal != nil)
                .help(refusal ?? "lf --task \(task.task.identifier) ship: run every remaining step and land it")
                .accessibilityIdentifier("task-ship")
        }
    }

    private var draft: TaskRunDraft? { model.navigation.taskRunDrafts[task.id] }
    private var acting: Bool { draft?.acting == true }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ScrollView(.horizontal) { controls }
                .scrollIndicators(.visible)
            if work.value?.workflow != nil, let error = work.errorMessage {
                Text("Workflow refresh failed: \(error)")
                    .foregroundStyle(palette.failure)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("task-workflow-refresh-error")
            }
            DisclosureGroup("Overview", isExpanded: Binding(
                get: { expandedTasks.contains(task.id) },
                set: { if $0 { expandedTasks.insert(task.id) } else { expandedTasks.remove(task.id) } }
            )) {
                if let workflow = work.value?.workflow {
                    WorkflowDiagram(
                        nodes: workflow.nodes, edges: workflow.edges, position: workflow.position,
                        // Ship it is the one way into `end`.
                        choices: workflow.outgoing.filter { workflow.edges.indices.contains($0) && workflow.edges[$0].to != "end" },
                        unavailable: unavailable, fits: true,
                        hint: { "lf task run \(task.task.identifier) \($0.name) · to \($0.to)" },
                        choose: { edge in Task { await model.startTask(edge.name, task: task) } })
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                Text(task.task.description)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.vertical, 6)
            }
            .accessibilityIdentifier("task-overview")
            TaskDueDateView(task: task)
            // Completion may fail after the Workflow has already arrived.
            if let conflict = task.runtime?.completionPending {
                HStack(spacing: Spacing.sm) {
                    Text(conflict)
                        .foregroundStyle(palette.failure)
                        .textSelection(.enabled)
                        .accessibilityIdentifier("task-completion-pending")
                    Button("Retry completion") {
                        Task { await model.completeTask(task) }
                    }
                    .buttonStyle(WorkOutlineButtonStyle())
                    .disabled(acting || readingUnavailable != nil)
                    .help("lf task complete \(task.task.identifier)")
                    .accessibilityIdentifier("task-completion-retry")
                }
            }
            if let error = draft?.error ?? model.navigation.taskSessionErrors[task.id] {
                Text(error)
                    .foregroundStyle(palette.failure)
                    .lineLimit(3)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("task-workflow-error")
                Button("Refresh readings") { model.rescanWork() }
                    .accessibilityIdentifier("task-refresh-readings")
            }
        }
        .confirmationDialog("Abandon \(task.task.identifier)?", isPresented: $confirmingAbandon) {
            Button("Abandon Task", role: .destructive) {
                Task { await model.abandonTask(task) }
            }
        } message: {
            Text("Cancels the Task, closes its PRs and deletes its branches. Pause instead to keep its work for later.")
        }
        .font(Typography.body(12))
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background {
            palette.background
            if task.task.terminalLabel != nil {
                (task.task.isSuccessful ? palette.success : palette.failure).opacity(0.14)
            }
        }
        .overlay(alignment: .leading) {
            if task.task.terminalLabel != nil {
                Rectangle()
                    .fill(task.task.isSuccessful ? palette.success : palette.failure)
                    .frame(width: 3)
            }
        }
        .overlay(alignment: .bottom) { Rectangle().fill(palette.border).frame(height: 1) }
        .accessibilityIdentifier("task-workflow")
        .onChange(of: work, initial: true) { _, reading in
            if reading.value != nil { Perf.endAfterCommit(Perf.workflowReady, id: task.id) }
        }
        .onChange(of: task.id) { _, id in
            if work.value != nil { Perf.endAfterCommit(Perf.workflowReady, id: id) }
        }
    }

    private func positionLabel(_ workflow: Workflow) -> String {
        switch workflow.position {
        case .node(let name): return "\(workflow.name) · \(name)"
        case .edge(let index, _, let running):
            guard workflow.edges.indices.contains(index) else { return "Workflow position unavailable" }
            let edge = workflow.edges[index]
            return "\(edge.from) → \(edge.to) · \(running ? "running" : "stopped")"
        }
    }

    /// Why no edge can be started now.
    private var unavailable: String? {
        if acting { return draft?.progress ?? "Updating Task…" }
        if task.task.isTerminal { return "Reopen the Task before starting work" }
        return readingUnavailable ?? task.runControl.unavailable
    }

    private var readingUnavailable: String? {
        if let error = work.errorMessage ?? model.roadmap.errorMessage { return error }
        if model.showsSavedPlanning { return WorkCache.savedReason }
        return nil
    }
}

/// A Task's identity wherever it heads Work: the window trail and Dashboard tiles.
/// Its container sets the primary and secondary text styles.
struct TaskHeading: View {
    let task: RoadmapTask

    /// Started is durable Task evidence, the definition Working Tasks lists by;
    /// Linear's planning state never overrides it.
    var state: String {
        if let label = task.task.terminalLabel { return label }
        if task.runtime?.status == .paused { return "Paused" }
        return task.runtime?.owner != nil ? "Started" : "Unstarted"
    }

    var body: some View {
        HStack(spacing: 6) {
            Text(task.task.identifier).font(.system(.body, design: .monospaced)).fixedSize()
            // The title truncates first; identifier and state keep their width.
            Text(task.task.name).lineLimit(1).truncationMode(.tail)
                .frame(minWidth: 0, alignment: .leading).help(task.task.name)
            Text(state).foregroundStyle(.secondary).fixedSize()
                .accessibilityIdentifier("task-heading-state")
        }
    }
}

extension RoadmapTask {
    /// Not yet started and still open: its Session slot offers Start.
    var awaitsStart: Bool { runtime?.owner == nil && !task.isTerminal }
}

/// An unstarted Task's Session slot. Showing it starts nothing; Start runs the
/// Task's Workflow through the same path as `lf task run`.
struct TaskStartOffer: View {
    let model: WorkModel
    let task: RoadmapTask

    private var unavailable: String? {
        if let draft = model.navigation.taskRunDrafts[task.id], draft.acting { return draft.progress ?? "Updating Task…" }
        if let error = model.roadmap.errorMessage { return error }
        if model.showsSavedPlanning { return WorkCache.savedReason }
        return task.runControl.unavailable
    }

    var body: some View {
        SessionSlotOffer(
            title: "Not started", systemImage: "play.circle",
            action: "Start", actionImage: "play.fill",
            note: model.navigation.taskRunDrafts[task.id]?.error
                ?? "Starts this Task’s Workflow and its first conversation.",
            unavailable: unavailable, help: "lf task run \(task.task.identifier)",
            accessibilityLabel: "Start \(task.task.identifier)",
            actionIdentifier: "task-start-\(task.id)",
            perform: { Task { await model.startTask(nil, task: task) } }
        ) {
            Text(task.task.description.isEmpty ? "No description recorded." : task.task.description)
                .lineLimit(8)
        }
        .accessibilityIdentifier("task-start-offer-\(task.id)")
    }
}

/// Shared Task actions for keyboard access and sidebar context menus.
struct TaskActionItems: View {
    let model: WorkModel
    let task: RoadmapTask
    let wave: WaveSnapshot?
    var newConversation: (() -> Void)?
    var openSession: ((SessionRecord) -> Void)?
    @State private var copied: String?

    private var unavailable: Bool {
        model.navigation.taskRunDrafts[task.id]?.acting == true || model.roadmap.errorMessage != nil || model.showsSavedPlanning
    }

    var body: some View {
        Button("New conversation") { newConversation?() }.disabled(newConversation == nil || unavailable)
        Menu("Move to Wave") {
            Button("No Wave") { Task { await model.moveTask(task, toWave: nil) } }
                .disabled(task.task.waveId == nil)
            ForEach(model.visibleRoadmaps.filter { $0.wave.repo == task.task.repo }, id: \.wave.id) { destination in
                Button(destination.wave.displayName) {
                    Task { await model.moveTask(task, toWave: destination.wave.id) }
                }.disabled(destination.wave.id == task.task.waveId)
            }
        }.disabled(unavailable).accessibilityIdentifier("task-move-wave")
        if task.task.isTerminal || task.runtime?.status.isTerminal == true {
            Button("Reopen Task") { Task { await model.reopenTask(task) } }.disabled(unavailable)
        } else {
            if task.shipping == nil {
                Button("Ship it") { Task { await model.shipTask(task) } }
                    .disabled(unavailable || task.shipControl.unavailable != nil)
            } else {
                Button("Stop shipping") { Task { await model.stopShipping(task) } }.disabled(unavailable)
            }
            Button("Complete Task") { Task { await model.completeTask(task) } }.disabled(unavailable)
            Button(task.runtime?.status == .paused ? "Resume Task" : "Pause Task") {
                Task { await model.pauseTask(task, paused: task.runtime?.status != .paused) }
            }.disabled(unavailable)
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
            if session.work?.kind != .task {
                Button("Bind \(session.title) to Task…") { model.beginSessionBinding(session) }
            }
        }
        if let error = model.navigation.taskRunDrafts[task.id]?.error { Text(error) }
    }

    private func copy(_ value: String, key: String) {
        NSPasteboard.general.clearContents()
        if NSPasteboard.general.setString(value, forType: .string) { copied = key }
    }
}


/// The Task due date stays beside the conversation and in Task details.
struct TaskDueDateView: View {
    let task: RoadmapTask
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xxs) {
            if let due = task.task.dueDate {
                Text("Due \(due)")
                    .font(Typography.body(12.5))
                    .foregroundStyle(palette.textSecondary)
            }
        }
    }
}

/// Every Graph process in the Task's checkout, newest first, whatever started
/// it: one line each, opening to its launched graph and steps. It is not the
/// Workflow's history: a process that carried an edge and one started ad hoc
/// are the same kind of row. A multiplexer pane on the page surface.
struct GraphProcessLog: View {
    let model: WorkModel
    let taskId: String
    let location: WorkspaceIdentity?
    var execution: TaskLocationObservation? = nil
    var query: RegistryQuery = RegistryQueryLocal.shared
    @State private var captured: [GraphProcessDetail]?
    @State private var error: String?
    @State private var refresh = 0
    @Environment(\.palette) private var palette

    var body: some View {
        let task = model.task(id: taskId)?.task
        let reading = task.map { model.taskWork[$0.id] }
        let readsWindow = execution == nil && query.remoteMachine == nil && location != nil
            && location?.machineId == model.observingMachineId && location == task?.reference.workspace?.identity
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if !readsWindow {
                    Button("Refresh") { refresh += 1 }
                    if let error { Text("Graph processes could not be read: \(error)").padding(.vertical, 6) }
                }
                if let graphs = captured?.map(\.entry) ?? (readsWindow ? reading?.value?.graphProcesses : nil) {
                    ForEach(Array(graphs.reversed().enumerated()), id: \.element.id) { index, graph in
                        if index > 0 { Divider().overlay(palette.border.opacity(0.6)) }
                        GraphProcessView(model: model, graph: graph, run: captured?.first { $0.entry.id == graph.id } ?? (readsWindow ? model.graphProcesses[graph.id] : nil))
                    }
                    if graphs.isEmpty {
                        Text("No Graph has run.").padding(.vertical, 6)
                            .accessibilityIdentifier("task-graph-processes-empty")
                    }
                } else if let error = error ?? reading?.errorMessage {
                    Text("Graph processes could not be read: \(error)").padding(.vertical, 6)
                } else {
                    Text(task == nil ? "Task unavailable" : "Reading Graph processes…").padding(.vertical, 6)
                }
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .font(Typography.body(12))
        .foregroundStyle(palette.textSecondary)
        .textSelection(.enabled)
        .background(palette.background)
        .environment(\.colorScheme, .light)
        .accessibilityIdentifier("task-graph-processes")
        .task(id: refresh) {
            guard !readsWindow else { return }
            do {
                let ownerQuery: RegistryQuery
                let subject: String
                let cwd: String?
                if let execution {
                    guard case .recorded(let ownerTask, _) = execution.location else {
                        throw RegistryQueryError("Task log location is unavailable.")
                    }
                    ownerQuery = query.onMachine(execution.machineID, repository: execution.repositoryID).pinTaskLocation(to: execution)
                    subject = ownerTask
                    cwd = nil
                } else {
                    guard let location, location.machineId == model.savedMachineId,
                          query.remoteMachine == nil else {
                        throw RegistryQueryError("Task log location is unavailable; no local fallback was used.")
                    }
                    ownerQuery = query
                    subject = task?.runtime?.workId ?? taskId
                    cwd = location.worktree
                }
                let graphs = try await ownerQuery.taskGraphProcesses(task: subject, cwd: cwd)
                try Task.checkCancellation()
                captured = graphs
                error = nil
            } catch { self.error = error.localizedDescription }
        }
    }
}

/// Diagnostic history is observed only while explicitly expanded.
struct TaskDiagnosticsView: View {
    let model: WorkModel
    let task: RoadmapTask
    @State private var expanded = false

    var body: some View {
        DisclosureGroup("Debug", isExpanded: $expanded) {
            if expanded { TaskWorkView(model: model, task: task) }
        }
        .onChange(of: expanded) { _, visible in model.observeTaskDiagnostics(task, visible: visible) }
        .onDisappear { model.observeTaskDiagnostics(task, visible: false) }
    }
}

/// The Task's conversations and mechanical Processes, including headless work and
/// conversations without turns: raw records, shown under Debug. Its Graph
/// processes are listed by `GraphProcessLog`.
struct TaskWorkView: View {
    let model: WorkModel
    let task: RoadmapTask
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            WorkSectionHeading(title: "Work") {
                Button("Refresh") { model.observeTaskDiagnostics(task, visible: true) }
            }
            if let work = model.taskDiagnostics[task.id].value {
                ForEach(work.sessions) { session in
                    row(session.interactive ? "Session" : "Run", id: session.id, name: session.title,
                        state: session.completedAt == nil ? "open" : "completed")
                }
                ForEach(work.processes) { process in
                    row("Process", id: process.id, name: process.command ?? "Unknown command",
                        state: process.outcome ?? "unknown")
                }
                if work.sessions.isEmpty && work.processes.isEmpty {
                    Text("No recorded work.")
                }
            }
            if let error = model.taskDiagnostics[task.id].errorMessage {
                Text("Work could not be read: \(error)")
                Button("Retry") { model.observeTaskDiagnostics(task, visible: true) }
            }
        }
        .font(Typography.body(12))
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
            Text(id).font(Typography.code(10)).help(id)
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
    private var expanded: Bool { model.navigation.expandedGraphProcesses.contains(graph.id) }
    @State private var inspected: UInt32?
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
                        .font(.system(size: 9, weight: .semibold))
                        .frame(width: 10)
                        .accessibilityHidden(true)
                    Text(graph.name).font(Typography.code(12)).foregroundStyle(palette.text)
                    Text(graph.state == .current ? "running" : graph.state.rawValue)
                        .foregroundStyle(tone?.ink(in: palette) ?? palette.textSecondary)
                    Spacer()
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
        Text(graph.id).font(Typography.code(10)).foregroundStyle(palette.textTertiary)
            .accessibilityIdentifier("graph-process-id-\(graph.id)")
        if let run {
            GraphDiagram(compiled: run.compiled, latest: run, inspected: $inspected)
            Text([run.presentation.reason, graphIterationLabel(run.iterations)]
                .compactMap { $0 }.joined(separator: " · "))
                .accessibilityIdentifier("graph-process-status-\(graph.id)")
            ForEach(run.steps) { step in
                HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                    Text(step.label).font(Typography.code(11))
                    if let iteration = graphIterationLabel(step.iterations) { Text(iteration).monospacedDigit() }
                    Spacer()
                    Text(step.outcome ?? (step.completedAt == nil ? "running" : "no recorded exit"))
                    if let completed = step.completedAt {
                        Text(Self.duration(completed - step.startedAt)).monospacedDigit()
                            .frame(minWidth: 52, alignment: .trailing)
                    }
                }
                .help(step.lfProcessId)
                .accessibilityIdentifier("graph-process-step-\(step.lfProcessId)")
            }
        } else {
            Text("Reading Graph…")
        }
    }
}
