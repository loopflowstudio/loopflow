import Loopflow
import SwiftUI

/// Keep current Workflow position/actions compact; disclose its graph in Overview.
/// Rust owns the graph, position and legal actions.
struct TaskWorkflowHeader: View {
    let model: WorkModel
    let task: RoadmapTask
    /// The Task part of the workspace stream, as last read.
    let work: Reading<TaskOverview>
    var workspaceActions: AnyView? = nil
    @Environment(\.palette) private var palette

    @State private var confirmingAbandon = false
    @State private var expandedTasks: Set<String> = []

    private var lifecycleLabel: String {
        switch task.lifecycle {
        case .done: "Completed"
        case .abandoned: "Canceled"
        default: task.lifecycle.label.capitalized
        }
    }

    private var controls: some View {
            HStack(alignment: .center, spacing: Spacing.md) {
                Text(lifecycleLabel)
                    .fontWeight(task.isCompleted ? .semibold : .regular)
                    .foregroundStyle(task.lifecycle == .done ? palette.success
                        : task.lifecycle == .abandoned ? palette.failure : palette.textSecondary)
                    .accessibilityIdentifier("task-lifecycle-status")
                if task.isUnassigned {
                    Text("Unassigned").foregroundStyle(palette.textSecondary)
                }
                if let workflow = work.value?.graph {
                    Text(positionLabel(workflow))
                        .lineLimit(1)
                        .foregroundStyle(palette.textSecondary)
                        .accessibilityIdentifier("task-workflow-position")
                    if let action = runAction(workflow) {
                        Button(action.label) { model.startTaskFromInput(nil, task: task) }
                            .buttonStyle(WorkOutlineButtonStyle(emphasized: true))
                            .disabled(unavailable != nil)
                            .help(unavailable ?? action.help)
                            .accessibilityIdentifier("task-workflow-run")
                    }
                    Spacer(minLength: 0)
                    // Start the next run at a step: back, ahead, or waiting at an interactive step.
                    Menu("Start from…") {
                        ForEach([TaskGraph.start] + workflow.interactiveSteps.map(\.name), id: \.self) { step in
                            Button(step) { model.startTaskFromInput(nil, from: step, task: task) }
                                .accessibilityIdentifier("task-workflow-move-\(step)")
                        }
                    }
                    .fixedSize()
                    .disabled(unavailable != nil)
                    .help(unavailable ?? "lf task run \(task.task.identifier) --from <step>")
                    .accessibilityIdentifier("task-workflow-move")

                } else if work.value != nil {
                    // The shared resolver reports the first-run default and its owner.
                    Text(task.graphName.map { "\($0) · \(task.graphSource ?? "")" } ?? "No workflow selected")
                        .foregroundStyle(palette.textSecondary)
                    Menu("Choose workflow") {
                        StartFlowItems(model: model, chosen: task.graphName) { model.startTaskFromInput($0, task: task) }
                    }.task { await model.loadGraphCatalog() }
                    Button("Start") { model.startTaskFromInput(nil, task: task) }
                        .buttonStyle(WorkOutlineButtonStyle())
                        .disabled(unavailable != nil)
                        .help(unavailable ?? "lf task run \(task.task.identifier)")
                        .accessibilityIdentifier("task-workflow-start")
                    Spacer()
                } else {
                    if work.isLoading {
                        Text(task.setupPosition ?? "Reading workflow…").foregroundStyle(palette.textSecondary)
                    }
                    Spacer()
                }
                if task.isCompleted {
                    Button("Reopen") { model.reopenTask(task) }
                        .disabled(readingUnavailable != nil)
                        .accessibilityIdentifier("task-reopen")
                } else {
                    Button("Complete") { model.completeTaskFromInput(task) }
                        .disabled(readingUnavailable != nil)
                        .accessibilityIdentifier("task-complete")
                    Button(task.runtime?.owner == nil ? "Claim" : "Unassign") {
                        model.assignTask(task, claimed: task.runtime?.owner == nil)
                    }
                    .disabled(readingUnavailable != nil)
                    .accessibilityIdentifier("task-claim-unassign")
                    Button("Abandon…", role: .destructive) { confirmingAbandon = true }
                        .disabled(readingUnavailable != nil)
                        .accessibilityIdentifier("task-abandon")
                }
                workspaceActions
            }
            .lineLimit(1)
            .frame(minHeight: 28)
    }


    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ScrollView(.horizontal) { controls }
                .scrollIndicators(.visible)
            DisclosureGroup("Overview", isExpanded: Binding(
                get: { expandedTasks.contains(task.id) },
                set: { if $0 { expandedTasks.insert(task.id) } else { expandedTasks.remove(task.id) } }
            )) {
                if let workflow = work.value?.graph {
                    let shape = WorkflowDefinition(workflow)
                    WorkflowDiagram(
                        nodes: shape.steps, edges: shape.runs, position: workflow.position,
                        choices: choices(workflow, shape),
                        unavailable: unavailable, fits: true,
                        hint: { "lf task run \(task.task.identifier) · to \($0.to)" },
                        choose: { _ in model.startTaskFromInput(nil, task: task) })
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                Text(task.task.description)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.vertical, 6)
            }
            .accessibilityIdentifier("task-overview")
            TaskPlanningFactsView(task: task)
            // Completion may fail after the Workflow has already arrived.
            if let conflict = task.runtime?.completionPending {
                Text(conflict)
                    .foregroundStyle(palette.failure)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("task-completion-pending")
            }
        }
        .confirmationDialog("Abandon \(task.task.identifier)?", isPresented: $confirmingAbandon) {
            Button("Abandon Task", role: .destructive) {
                model.abandonTask(task)
            }
        } message: {
            Text("Cancels the Task, closes its PRs and deletes its branches. Unassign it instead to keep its work for later.")
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

    /// What `lf task run` does from here, named for it; `nil` while a run
    /// carries the Task or once it is at `end`.
    static func runAction(_ workflow: TaskGraph, issue: String) -> (label: String, help: String)? {
        let help = "lf task run \(issue)"
        switch workflow.position {
        case .at(TaskGraph.start): return ("Start", help)
        case .at(TaskGraph.end): return nil
        case .at(let step): return ("Continue", "\(help): continue after \(step)")
        case .run(_, _, _, true): return nil
        case .run(_, let at, _, false): return ("Run again", "\(help): run again at \(at)")
        }
    }

    private func runAction(_ workflow: TaskGraph) -> (label: String, help: String)? {
        Self.runAction(workflow, issue: task.task.identifier)
    }

    /// The run `lf task run` would start, by its place among the diagram's runs.
    private func choices(_ workflow: TaskGraph, _ shape: WorkflowDefinition) -> [Int] {
        guard runAction(workflow) != nil,
              let index = shape.runs.firstIndex(where: { $0.from == workflow.position.step }) else { return [] }
        return [index]
    }

    private func positionLabel(_ workflow: TaskGraph) -> String {
        switch workflow.position {
        case .at(let step): return "\(workflow.name) · \(step)"
        case .run(let step, _, _, let running):
            let to = WorkflowDefinition(workflow).runs.first { $0.from == step }?.to
            return "\([step, to].compactMap { $0 }.joined(separator: " → ")) · \(running ? "running" : "stopped")"
        }
    }

    /// Why no run can be started now.
    private var unavailable: String? {
        readingUnavailable ?? task.runControl.unavailable
    }

    private var readingUnavailable: String? {
        if !model.pendingChanges.acceptsChanges { return PendingChanges.awayText }
        if model.showsSavedPlanning { return WorkCache.savedReason }
        return nil
    }
}

/// The Auto toggle at the right end of the Workflow bar. On: the Task's run
/// takes its interactive steps and lands it. Turning it on is `lf task auto on`, which
/// also runs the Task; Rust decides whether that can start. Sidebar rows and
/// menus carry no auto control.
struct TaskAutoControl: View {
    let model: WorkModel
    let task: RoadmapTask

    @State private var hovered = false
    @State private var confirmingAbandon = false

    private var readingUnavailable: String? {
        if !model.pendingChanges.acceptsChanges { return PendingChanges.awayText }
        if model.showsSavedPlanning { return WorkCache.savedReason }
        return nil
    }

    /// The Task reading's own mark; until it arrives, the state its row carries.
    private var on: Bool {
        guard let execution = model.taskExecution[task.id].value else { return task.selectionStates.contains(.auto) }
        return execution.auto?.state == .on
    }

    var body: some View {
        Group {
            if on {
                toggle(refusal: readingUnavailable)
            } else if !task.isCompleted {
                HStack(spacing: Spacing.sm) {
                    // Hovering Auto offers the other ways out of this Task.
                    if hovered {
                        let owned = task.runtime?.owner != nil
                        Button(owned ? "Unassign" : "Claim") {
                            model.assignTask(task, claimed: !owned)
                        }
                        .buttonStyle(WorkOutlineButtonStyle())
                        .disabled(readingUnavailable != nil)
                        .help(owned ? "lf task unassign: nobody owns it, so nothing moves it; its checkout, PR and history stay"
                                    : "lf task claim: you own it on this Machine")
                        .accessibilityIdentifier("task-unassign")
                        Button("Abandon…", role: .destructive) { confirmingAbandon = true }
                            .buttonStyle(WorkOutlineButtonStyle(destructive: true))
                            .disabled(readingUnavailable != nil)
                            .accessibilityIdentifier("task-abandon")
                    }
                    toggle(refusal: readingUnavailable ?? task.runControl.unavailable)
                }
                .onHover { inside in withAnimation(.easeOut(duration: 0.12)) { hovered = inside } }
            }
        }
        .fixedSize()
        .confirmationDialog("Abandon \(task.task.identifier)?", isPresented: $confirmingAbandon) {
            Button("Abandon Task", role: .destructive) {
                model.abandonTask(task)
            }
        } message: {
            Text("Cancels the Task, closes its PRs and deletes its branches. Unassign it instead to keep its work for later.")
        }
    }

    private func toggle(refusal: String?) -> some View {
        let issue = task.task.identifier
        return Toggle("Auto", isOn: Binding(get: { on }, set: { turnOn in
            if turnOn { model.turnAutoOn(task) } else { model.turnAutoOff(task) }
        }))
        .toggleStyle(.switch).controlSize(.small)
        .font(Typography.text)
        .disabled(refusal != nil)
        .help(refusal ?? (on
            ? "lf task auto off \(issue): the run waits at its next interactive step for you, and auto-merge is disarmed"
            : "lf task auto on \(issue): run every remaining step and land it"))
        .accessibilityIdentifier("task-auto")
    }
}

/// The selected Task's Workflow under the jump bar: its interactive steps as dots, what
/// is happening now, and the Auto toggle. Clicking the dots opens a
/// row with each interactive step, who takes part there and where the Task is. Rust owns the
/// graph, the position and whether auto can start; this only arranges them.
struct TaskWorkflowBar: View {
    let model: WorkModel
    let task: RoadmapTask
    /// The Task part of the workspace stream, as last read.
    let work: Reading<TaskOverview>
    /// Per viewer: the bar opens to its step cells.
    @State private var expanded = false
    @Environment(\.palette) private var palette

    /// One dot: an interactive step where a person takes part, or `end`.
    struct Stage: Equatable {
        enum Place: Equatable { case passed, here, going, ahead }
        let name: String
        let place: Place
        /// You, or a stand-in while auto is on; `end` has no one.
        let who: String?
        let note: String
        /// Where the Task stands now or is headed.
        var current: Bool { place == .here || place == .going }
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: Spacing.md) {
                if let workflow = work.value?.graph {
                    Text(workflow.name).fontWeight(.semibold).fixedSize()
                        .accessibilityIdentifier("task-workflow-bar-name")
                    dots(stages)
                }
                Text(live)
                    .lineLimit(1).truncationMode(.tail)
                    .foregroundStyle(palette.textSecondary)
                    .help(live)
                    .accessibilityIdentifier("task-workflow-bar-live")
                Spacer(minLength: 0)
                TaskAutoControl(model: model, task: task)
            }
            .padding(.horizontal, 14)
            .frame(height: 34)
            if expanded, work.value?.graph != nil {
                cells(stages)
            }
        }
        .font(Typography.text)
        .background(palette.background)
        .overlay(alignment: .bottom) { Rectangle().fill(palette.border).frame(height: 1) }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("task-workflow-bar")
    }

    private func dots(_ stages: [Stage]) -> some View {
        Button { expanded.toggle() } label: {
            HStack(spacing: 5) {
                ForEach(stages, id: \.name) { stage in
                    Circle()
                        .fill(fill(stage.place))
                        .overlay(Circle().strokeBorder(stage.place == .ahead ? palette.borderStrong : .clear))
                        .frame(width: stage.current ? 9 : 7, height: stage.current ? 9 : 7)
                        .help(stage.who.map { "\(stage.name) (\($0))" } ?? stage.name)
                }
                Image(systemName: expanded ? "chevron.down" : "chevron.right")
                    .font(Typography.meta).fontWeight(.semibold)
                    .foregroundStyle(palette.textTertiary)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .fixedSize()
        .help(expanded ? "Hide interactive steps" : "Show interactive steps")
        .accessibilityLabel("Interactive steps")
        .accessibilityIdentifier("task-workflow-bar-dots")
    }

    private func cells(_ stages: [Stage]) -> some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            ForEach(stages, id: \.name) { stage in
                VStack(alignment: .leading, spacing: 2) {
                    Text(stage.who ?? " ").font(Typography.meta).foregroundStyle(palette.textTertiary)
                    Text(stage.name).fontWeight(.semibold)
                    Text(stage.note).foregroundStyle(palette.textSecondary).lineLimit(2)
                }
                .padding(.horizontal, 8).padding(.vertical, 6)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(RoundedRectangle(cornerRadius: CornerRadius.row).fill(
                    stage.current ? palette.surfaceElement : palette.surface))
                .accessibilityIdentifier("task-workflow-bar-stage-\(stage.name)")
            }
        }
        .padding(.horizontal, 14).padding(.bottom, 8)
    }

    private func fill(_ place: Stage.Place) -> Color {
        switch place {
        case .passed: palette.textTertiary
        case .here: palette.text
        case .going: auto?.state == .on ? palette.success : palette.working
        case .ahead: .clear
        }
    }

    private var stages: [Stage] {
        work.value?.graph.map { Self.stages($0, auto: auto?.state == .on) } ?? []
    }

    private var auto: Auto? { model.taskExecution[task.id].value?.auto }

    /// What is happening now, in one line: where the Task stands, and its
    /// auto setting when someone set one.
    var live: String {
        guard let auto else { return position }
        return "\(position) · \(Self.autoLine(auto))"
    }

    static func autoLine(_ mark: Auto) -> String {
        switch mark.state {
        case .on: return "auto on, by \(mark.by.label)"
        case .off:
            let when = Date(timeIntervalSince1970: TimeInterval(mark.at)).formatted(date: .abbreviated, time: .shortened)
            return "auto off by \(mark.by.label) \(when)"
        }
    }

    private var position: String {
        guard let workflow = work.value?.graph else {
            if work.value == nil { return work.isLoading ? task.setupPosition ?? "Reading workflow…" : "" }
            return task.graphName.map { "Takes up \($0) when it starts" } ?? "No workflow selected"
        }
        switch workflow.position {
        // A Task that took up its Graph and is waiting for setup to start it.
        case .at(TaskGraph.start): return task.setupPosition ?? "Not started"
        case .at(TaskGraph.end): return "At its end"
        case .at(let step): return "At \(step) · yours"
        case .run(let from, let at, let process, let running):
            let to = WorkflowDefinition(workflow).runs.first { $0.from == from }?.to
            guard running else { return "Stopped at \(at)" + (to.map { " before \($0)" } ?? "") }
            let step = model.graphProcesses[process]?.presentation.reason
            return [[from, to].compactMap { $0 }.joined(separator: " → "), step].compactMap { $0 }.joined(separator: " · ")
        }
    }

    /// The Workflow's interactive steps in order, then `end`, each placed
    /// against the Task's position. While auto is on, stand-ins take the steps ahead.
    static func stages(_ workflow: TaskGraph, auto: Bool) -> [Stage] {
        let names = workflow.interactiveSteps.map(\.name) + [TaskGraph.end]
        let (at, going): (String, Bool) = switch workflow.position {
        case .at(let step): (step, false)
        case .run(let from, _, _, let running):
            (WorkflowDefinition(workflow).runs.first { $0.from == from }?.to ?? TaskGraph.start, running)
        }
        let here = names.firstIndex(of: at) ?? -1
        return names.enumerated().map { index, name in
            let place: Stage.Place = index < here ? .passed : index == here ? (going ? .going : .here) : .ahead
            let who: String? = name == "end" ? nil : (auto && index >= here ? "stand-in" : "you")
            let note: String = switch place {
            case .here: "here now"
            case .going: "on the way"
            case .passed: "passed"
            case .ahead: auto ? "agents will run it" : "ahead"
            }
            return Stage(name: name, place: place, who: who, note: note)
        }
    }
}

/// A Task's identity wherever it heads Work: the window trail and Dashboard tiles.
/// Its container sets the primary and secondary text styles.
struct TaskHeading: View {
    let task: RoadmapTask

    /// The Task's lifecycle, the state Working Tasks list by; Linear's planning
    /// state never overrides it. What its run is doing now reads in the Workflow bar.
    var state: String { task.lifecycle.label.capitalized }

    var body: some View {
        HStack(spacing: 6) {
            Text(task.task.identifier).font(Typography.mono).fixedSize()
            // The title truncates first; identifier and state keep their width.
            Text(task.task.name).lineLimit(1).truncationMode(.tail)
                .frame(minWidth: 0, alignment: .leading).help(task.task.name)
            Text(state).foregroundStyle(.secondary).fixedSize()
                .accessibilityIdentifier("task-heading-state")
        }
    }
}

/// An unstarted Task's Session slot. Showing it starts nothing; Start runs the
/// Task's Workflow through the same path as `lf task run`.
struct TaskStartOffer: View {
    let model: WorkModel
    let task: RoadmapTask

    private var unavailable: String? {
        if !model.pendingChanges.acceptsChanges { return PendingChanges.awayText }
        if model.showsSavedPlanning { return WorkCache.savedReason }
        return task.runControl.unavailable
    }

    var body: some View {
        SessionSlotOffer(
            title: "Not started", systemImage: "play.circle",
            action: "Start", actionImage: "play.fill",
            note: "Starts this Task’s Workflow and its first conversation.",
            unavailable: unavailable, help: "lf task run \(task.task.identifier)",
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

/// Shared Task actions for keyboard access and sidebar context menus.
struct TaskActionItems: View {
    let model: WorkModel
    let task: RoadmapTask
    let wave: WaveSnapshot?
    var newConversation: (() -> Void)?
    var openSession: ((SessionRecord) -> Void)?
    @State private var copied: String?

    private var unavailable: Bool {
        model.showsSavedPlanning || !model.pendingChanges.acceptsChanges
    }

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
        if task.isCompleted {
            Button("Reopen Task") { model.reopenTask(task) }.disabled(unavailable)
        } else {
            Button("Complete Task") { model.completeTaskFromInput(task) }.disabled(unavailable)
            Button(task.runtime?.owner == nil ? "Claim Task" : "Unassign Task") {
                model.assignTask(task, claimed: task.runtime?.owner == nil)
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


/// The Task's due date and regression mark stay beside the conversation and in Task details.
struct TaskPlanningFactsView: View {
    let task: RoadmapTask
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xxs) {
            if let due = task.task.dueDate {
                Text("Due \(due)")
                    .font(Typography.text)
                    .foregroundStyle(palette.textSecondary)
            }
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
                HStack {
                    if let work {
                        TaskRunPosition(workflow: work.graph,
                                        execution: readsWindow ? task?.execution : captured?.row?.execution)
                    } else {
                        Text(task?.setupPosition ?? "Reading workflow…")
                    }
                    Spacer()
                    if let openFiles { Button("Files", action: openFiles) }
                }.padding(.vertical, 8)
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


struct TaskRunPosition: View {
    let workflow: TaskGraph?
    let execution: TaskExecutionSnapshot?

    var label: String {
        guard let workflow else { return execution?.reason ?? "No workflow selected" }
        switch workflow.position {
        case .at(TaskGraph.start): return "\(workflow.name) · not started"
        case .at(TaskGraph.end): return "\(workflow.name) · at end"
        case .at(let step): return "\(step) · available for review"
        case .run(_, let at, _, let running):
            if let execution, execution.state != .idle { return execution.reason }
            return "\(running ? "Running" : "Stopped") at \(at)"
        }
    }

    var body: some View {
        Text(label).textSelection(.enabled)
            .accessibilityIdentifier("task-run-position")
    }
}
