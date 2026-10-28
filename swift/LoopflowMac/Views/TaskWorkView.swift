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
                if task.runtime?.status == .deferred {
                    Text("Deferred").foregroundStyle(palette.textSecondary)
                } else if task.task.terminalLabel == nil, let state = task.task.state {
                    Text(state.capitalized).foregroundStyle(palette.textSecondary)
                        .accessibilityIdentifier("task-lifecycle-status")
                }
                if let workflow = work.value?.graph {
                    Text(positionLabel(workflow))
                        .lineLimit(1)
                        .foregroundStyle(palette.textSecondary)
                        .accessibilityIdentifier("task-workflow-position")
                    if let action = runAction(workflow) {
                        Button(action.label) { Task { await model.startTask(nil, task: task) } }
                            .buttonStyle(WorkOutlineButtonStyle(emphasized: true))
                            .disabled(unavailable != nil)
                            .help(unavailable ?? action.help)
                            .accessibilityIdentifier("task-workflow-run")
                    }
                    Spacer(minLength: 0)
                    // Start the next run at a step: back, ahead, or waiting at an interactive step.
                    Menu("Start from…") {
                        ForEach([TaskGraph.start] + workflow.interactiveSteps.map(\.name), id: \.self) { step in
                            Button(step) { Task { await model.startTask(nil, from: step, task: task) } }
                                .accessibilityIdentifier("task-workflow-move-\(step)")
                        }
                    }
                    .fixedSize()
                    .disabled(unavailable != nil)
                    .help(unavailable ?? "lf task run \(task.task.identifier) --from <step>")
                    .accessibilityIdentifier("task-workflow-move")

                } else if work.value != nil, work.errorMessage == nil {
                    // The shared resolver reports the first-run default and its owner.
                    Text(task.graphName.map { "\($0) · \(task.graphSource ?? "")" } ?? "No workflow selected")
                        .foregroundStyle(palette.textSecondary)
                    Menu("Choose workflow") {
                        ForEach(model.workflowCatalog.value ?? [], id: \.name) { choice in
                            Button(choice.name) { Task { await model.startTask(choice.name, task: task) } }
                        }
                    }.task { await model.loadGraphCatalog() }
                    Button("Start") { Task { await model.startTask(nil, task: task) } }
                        .buttonStyle(WorkOutlineButtonStyle())
                        .disabled(unavailable != nil)
                        .help(unavailable ?? "lf task run \(task.task.identifier)")
                        .accessibilityIdentifier("task-workflow-start")
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
                    Button(task.runtime?.status == .deferred ? "Resume" : "Defer") {
                        Task { await model.deferTask(task, deferred: task.runtime?.status != .deferred) }
                    }
                    .disabled(acting || readingUnavailable != nil)
                    .accessibilityIdentifier("task-defer-resume")
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

    private var draft: TaskRunDraft? { model.navigation.taskRunDrafts[task.id] }
    private var acting: Bool { draft?.acting == true }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ScrollView(.horizontal) { controls }
                .scrollIndicators(.visible)
            if work.value?.graph != nil, let error = work.errorMessage {
                Text("Workflow refresh failed: \(error)")
                    .foregroundStyle(palette.failure)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("task-workflow-refresh-error")
            }
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
                        choose: { _ in Task { await model.startTask(nil, task: task) } })
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
            Text("Cancels the Task, closes its PRs and deletes its branches. Defer instead to keep its work for later.")
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

    /// What `lf task run` does from here, named for it; `nil` while a run
    /// carries the Task or once it is at `end`.
    static func runAction(_ workflow: TaskGraph, issue: String) -> (label: String, help: String)? {
        let help = "lf task run \(issue)"
        switch workflow.position {
        case .at(TaskGraph.start): return ("Start", help)
        case .at(TaskGraph.end): return nil
        case .at(let step): return ("Continue", "\(help): continue after \(step)")
        case .run(_, _, true): return nil
        case .run(let step, _, false): return ("Run again", "\(help): run again from \(step)")
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
        case .run(let step, _, let running):
            let to = WorkflowDefinition(workflow).runs.first { $0.from == step }?.to
            return "\([step, to].compactMap { $0 }.joined(separator: " → ")) · \(running ? "running" : "stopped")"
        }
    }

    /// Why no run can be started now.
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

/// Auto-Ship at the right end of the Workflow bar, always visible; while
/// shipping the same spot holds Stop Auto-Ship. Rust decides whether shipping
/// can start. Sidebar rows and menus carry no ship control.
struct TaskShipControl: View {
    let model: WorkModel
    let task: RoadmapTask

    @State private var hovered = false
    @State private var confirmingAbandon = false

    private var acting: Bool { model.navigation.taskRunDrafts[task.id]?.acting == true }

    private var readingUnavailable: String? {
        if let error = model.roadmap.errorMessage { return error }
        if model.showsSavedPlanning { return WorkCache.savedReason }
        return nil
    }

    var body: some View {
        Group {
            if task.shipping != nil {
                Button("Stop Auto-Ship") { Task { await model.stopShipping(task) } }
                    .buttonStyle(WorkOutlineButtonStyle())
                    .disabled(acting || readingUnavailable != nil)
                    .help("lf --task \(task.task.identifier) ship --stop: withdraw it and disarm auto-merge")
                    .accessibilityIdentifier("task-ship-stop")
            } else if !(task.task.isTerminal || task.runtime?.status.isTerminal == true) {
                let refusal = readingUnavailable ?? task.shipControl.unavailable
                HStack(spacing: Spacing.sm) {
                    // Hovering Auto-Ship offers the other ways out of this Task.
                    if hovered {
                        let deferred = task.runtime?.status == .deferred
                        Button(deferred ? "Resume" : "Defer") {
                            Task { await model.deferTask(task, deferred: !deferred) }
                        }
                        .buttonStyle(WorkOutlineButtonStyle())
                        .disabled(acting || readingUnavailable != nil)
                        .help(deferred ? "lf task resume: let it run again"
                                     : "lf task defer: hold it, keeping its checkout, PR and interactive step")
                        .accessibilityIdentifier("task-defer")
                        Button("Abandon…", role: .destructive) { confirmingAbandon = true }
                            .buttonStyle(WorkOutlineButtonStyle(destructive: true))
                            .disabled(acting || readingUnavailable != nil)
                            .accessibilityIdentifier("task-ship-abandon")
                    }
                    Button("Auto-Ship") { Task { await model.shipTask(task) } }
                        .buttonStyle(WorkShipButtonStyle())
                        .disabled(acting || refusal != nil)
                        .help(refusal ?? "lf --task \(task.task.identifier) ship: run every remaining step and land it")
                        .accessibilityIdentifier("task-ship")
                }
                .onHover { inside in withAnimation(.easeOut(duration: 0.12)) { hovered = inside } }
            }
        }
        .fixedSize()
        .confirmationDialog("Abandon \(task.task.identifier)?", isPresented: $confirmingAbandon) {
            Button("Abandon Task", role: .destructive) {
                Task { await model.abandonTask(task) }
            }
        } message: {
            Text("Cancels the Task, closes its PRs and deletes its branches. Defer instead to keep its work for later.")
        }
    }
}

/// The selected Task's Workflow under the jump bar: its interactive steps as dots, what
/// is happening now, and Auto-Ship or Stop Auto-Ship. Clicking the dots opens a
/// row with each interactive step, who takes part there and where the Task is. Rust owns the
/// graph, the position and the shipping reason; this only arranges them.
struct TaskWorkflowBar: View {
    let model: WorkModel
    let task: RoadmapTask
    /// The Task part of the workspace stream, as last read.
    let work: WorkReading<TaskOverview>
    /// Per viewer: the bar opens to its step cells.
    @State private var expanded = false
    @Environment(\.palette) private var palette

    /// One dot: an interactive step where a person takes part, or `end`.
    struct Stage: Equatable {
        enum Place: Equatable { case passed, here, going, ahead }
        let name: String
        let place: Place
        /// You, or a stand-in while shipping; `end` has no one.
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
                    .foregroundStyle(task.shipping?.state == .restarting ? palette.warning : palette.textSecondary)
                    .help(live)
                    .accessibilityIdentifier("task-workflow-bar-live")
                Spacer(minLength: 0)
                TaskShipControl(model: model, task: task)
            }
            .padding(.horizontal, 14)
            .frame(height: 34)
            if expanded, work.value?.graph != nil {
                cells(stages)
            }
        }
        .font(Typography.body(12))
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
                    .font(.system(size: 9, weight: .semibold))
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
                    Text(stage.who ?? " ").font(Typography.caption(10)).foregroundStyle(palette.textTertiary)
                    Text(stage.name).fontWeight(.semibold)
                    Text(stage.note).foregroundStyle(palette.textSecondary).lineLimit(2)
                }
                .padding(.horizontal, 8).padding(.vertical, 6)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(RoundedRectangle(cornerRadius: 6).fill(
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
        case .going: task.shipping != nil ? palette.success : palette.working
        case .ahead: .clear
        }
    }

    private var stages: [Stage] {
        work.value?.graph.map { Self.stages($0, shipping: task.shipping != nil) } ?? []
    }

    /// What is happening now, in one line.
    var live: String {
        if let shipping = task.shipping { return shipping.reason }
        guard let workflow = work.value?.graph else {
            if let error = work.errorMessage { return "Workflow could not be read: \(error)" }
            if work.value == nil { return "Reading workflow…" }
            return task.graphName.map { "Takes up \($0) when it starts" } ?? "No workflow selected"
        }
        switch workflow.position {
        case .at(TaskGraph.start): return "Not started"
        case .at(TaskGraph.end): return "At its end"
        case .at(let step): return "At \(step) · yours"
        case .run(let from, let process, let running):
            let to = WorkflowDefinition(workflow).runs.first { $0.from == from }?.to
            guard running else { return to.map { "Stopped before \($0)" } ?? "Stopped from \(from)" }
            let step = model.graphProcesses[process]?.presentation.reason
            return [[from, to].compactMap { $0 }.joined(separator: " → "), step].compactMap { $0 }.joined(separator: " · ")
        }
    }

    /// The Workflow's interactive steps in order, then `end`, each placed
    /// against the Task's position. While shipping, stand-ins take the steps ahead.
    static func stages(_ workflow: TaskGraph, shipping: Bool) -> [Stage] {
        let names = workflow.interactiveSteps.map(\.name) + [TaskGraph.end]
        let (at, going): (String, Bool) = switch workflow.position {
        case .at(let step): (step, false)
        case .run(let from, _, let running):
            (WorkflowDefinition(workflow).runs.first { $0.from == from }?.to ?? TaskGraph.start, running)
        }
        let here = names.firstIndex(of: at) ?? -1
        return names.enumerated().map { index, name in
            let place: Stage.Place = index < here ? .passed : index == here ? (going ? .going : .here) : .ahead
            let who: String? = name == "end" ? nil : (shipping && index >= here ? "stand-in" : "you")
            let note: String = switch place {
            case .here: "here now"
            case .going: "on the way"
            case .passed: "passed"
            case .ahead: shipping ? "agents will run it" : "ahead"
            }
            return Stage(name: name, place: place, who: who, note: note)
        }
    }
}

/// What the last Auto-Ship or Stop Auto-Ship did, shown briefly.
struct ShipNotice: Equatable {
    let id = UUID()
    let taskID: String
    let issue: String
    let shipped: Bool

    init(task: RoadmapTask, shipped: Bool) {
        taskID = task.id
        issue = task.task.identifier
        self.shipped = shipped
    }
}

/// The toast after Auto-Ship; its Undo is Stop Auto-Ship.
struct ShipNoticeToast: View {
    let notice: ShipNotice
    let undo: () -> Void
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.md) {
            Text(notice.shipped
                 ? "\(notice.issue) is shipping"
                 : "\(notice.issue) is back in Working · auto-merge disarmed")
                .accessibilityIdentifier("task-ship-notice")
            if notice.shipped {
                Button("Undo", action: undo)
                    .buttonStyle(.plain)
                    .fontWeight(.semibold)
                    .foregroundStyle(palette.success)
                    .help("Stop Auto-Ship")
                    .accessibilityIdentifier("task-ship-undo")
            }
        }
        .font(Typography.body(12))
        .foregroundStyle(palette.text)
        .padding(.leading, 14)
        .padding(.trailing, 10)
        .padding(.vertical, 8)
        .background(RoundedRectangle(cornerRadius: 9).fill(palette.surfaceElement))
        .overlay(RoundedRectangle(cornerRadius: 9).strokeBorder(palette.border, lineWidth: 1))
        .shadow(color: palette.shadow, radius: 8, y: 2)
    }
}

/// The window's ship toast, shown for six seconds after Auto-Ship or Stop
/// Auto-Ship; its Undo is Stop Auto-Ship.
struct ShipNoticeOverlay: View {
    let model: WorkModel

    var body: some View {
        let navigation = model.navigation
        ZStack {
            if let notice = navigation.shipNotice {
                ShipNoticeToast(notice: notice) {
                    guard let task = model.task(id: notice.taskID)?.task else { return }
                    Task { await model.stopShipping(task) }
                }
                .padding(.bottom, 18)
                .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .zIndex(ZIndex.toast)
        .animation(.easeOut(duration: 0.18), value: navigation.shipNotice)
        .task(id: navigation.shipNotice) {
            guard navigation.shipNotice != nil else { return }
            try? await Task.sleep(for: .seconds(6))
            if !Task.isCancelled { navigation.shipNotice = nil }
        }
    }
}

/// A Task's identity wherever it heads Work: the window trail and Dashboard tiles.
/// Its container sets the primary and secondary text styles.
struct TaskHeading: View {
    let task: RoadmapTask

    /// The Task's lifecycle, the state Working Tasks and Shipping list by;
    /// Linear's planning state never overrides it. What shipping is doing
    /// now reads in the Workflow bar.
    var state: String {
        switch task.runtime?.status {
        case .deferred: return "Deferred"
        case .idle: return "Idle"
        case .active: return "Active"
        case .moving: return "Moving"
        case .shipping: return "Shipping"
        case .done: return "Done"
        case .abandoned: return "Abandoned"
        case .unstarted, nil:
            guard task.task.isTerminal else { return "Unstarted" }
            return task.task.isSuccessful ? "Done" : "Abandoned"
        }
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
            Button("Complete Task") { Task { await model.completeTask(task) } }.disabled(unavailable)
            Button(task.runtime?.status == .deferred ? "Resume Task" : "Defer Task") {
                Task { await model.deferTask(task, deferred: task.runtime?.status != .deferred) }
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
/// Workflow's history: a process `lf task run` started and one started ad hoc
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
                let graphs = try await ownerQuery.taskGraphProcesses(task: subject)
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
