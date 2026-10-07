import Loopflow
import SwiftUI

/// The Task workspace's header: the Task's Workflow as a graph scaled to the
/// window, the node it waits at or the edge it is on marked. The edges that
/// can be chosen now are buttons (`lf task run`) and **Move to** sets a node
/// (`lf task move`). Rust owns the graph and the position.
struct TaskWorkflowHeader: View {
    let model: WorkModel
    let task: RoadmapTask
    let wave: WaveSnapshot
    /// The Task part of the workspace stream, as last read.
    let work: WorkReading<TaskWork>
    @Environment(\.palette) private var palette

    private var draft: TaskFlowDraft? { model.navigation.flowDrafts[task.id] }
    private var acting: Bool { draft?.acting == true }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .center, spacing: Spacing.md) {
                if let workflow = work.value?.workflow {
                    WorkflowGraph(
                        nodes: workflow.nodes, edges: workflow.edges, position: workflow.position,
                        choices: workflow.outgoing, unavailable: unavailable, fits: true,
                        hint: { "lf task run \(task.task.identifier) \($0.launchName) · to \($0.to)" },
                        choose: { edge in Task { await model.startFlow(edge.launchName, task: task, wave: wave) } })
                        .frame(maxWidth: .infinity, alignment: .leading)
                    // Put the Task at a node without running anything.
                    Menu("Move to") {
                        ForEach(["start"] + workflow.nodes.map(\.name) + ["end"], id: \.self) { node in
                            Button(node) { Task { await model.moveTask(to: node, task: task, wave: wave) } }
                                .accessibilityIdentifier("task-workflow-move-\(node)")
                        }
                    }
                    .fixedSize()
                    .disabled(acting)
                    .help("lf task move \(task.task.identifier) <node>")
                    .accessibilityIdentifier("task-workflow-move")
                } else if work.value != nil {
                    // A Task takes up its Project's workflow on its first run.
                    Text("No workflow yet").foregroundStyle(palette.textSecondary)
                    Button("Start") { Task { await model.startFlow(nil, task: task, wave: wave) } }
                        .buttonStyle(WorkOutlineButtonStyle())
                        .disabled(unavailable != nil)
                        .help(unavailable ?? "lf task run \(task.task.identifier)")
                        .accessibilityIdentifier("task-workflow-start")
                    Button("Complete") { Task { await model.moveTask(to: "end", task: task, wave: wave) } }
                        .disabled(acting)
                        .accessibilityIdentifier("task-workflow-end")
                    Spacer()
                } else {
                    Text(work.errorMessage.map { "Workflow could not be read: \($0)" }
                         ?? "Reading workflow…")
                        .foregroundStyle(palette.textSecondary)
                    Spacer()
                }
                if acting { ProgressView().controlSize(.small) }
            }
            if task.actions.reason.contains("Remaining work:") {
                Text(task.actions.reason)
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("task-remaining-work")
            }
            // Linear called the Task complete while it is active here.
            if let conflict = task.runtime?.planningConflict {
                HStack(spacing: Spacing.sm) {
                    Text(conflict)
                        .foregroundStyle(WorkTone.blocked.ink)
                        .textSelection(.enabled)
                        .accessibilityIdentifier("task-workflow-conflict")
                    Button("Complete anyway") {
                        Task { await model.moveTask(to: "end", force: true, task: task, wave: wave) }
                    }
                    .buttonStyle(WorkOutlineButtonStyle())
                    .disabled(acting)
                    .help("lf task move \(task.task.identifier) end --force")
                    .accessibilityIdentifier("task-workflow-force-end")
                }
            }
            if let error = draft?.error ?? model.navigation.taskSessionErrors[task.id] {
                Text(error)
                    .foregroundStyle(WorkTone.blocked.ink)
                    .lineLimit(3)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("task-workflow-error")
            }
        }
        .font(Typography.body(12))
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(palette.background)
        .overlay(alignment: .bottom) { Rectangle().fill(palette.border).frame(height: 1) }
        .accessibilityIdentifier("task-workflow")
    }

    /// Why no edge can be started now.
    private var unavailable: String? {
        if acting { return "Starting" }
        return task.flow.control(.start)?.unavailable
    }
}

/// Every Flow exec in the Task's checkout, newest first, whatever started
/// it: one line each, opening to its launched graph and steps. It is not the
/// Workflow's history: an exec that carried an edge and one started ad hoc
/// are the same kind of row. A multiplexer pane on the page surface.
struct FlowExecLog: View {
    let model: WorkModel
    let taskId: String
    @Environment(\.palette) private var palette

    var body: some View {
        let task = model.task(id: taskId)?.task
        let reading = task.map { model.taskWork[$0.id] }
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if let flows = reading?.value?.flows {
                    ForEach(Array(flows.reversed().enumerated()), id: \.element.id) { index, flow in
                        if index > 0 { Divider().overlay(palette.border.opacity(0.6)) }
                        FlowRunView(model: model, flow: flow)
                    }
                    if flows.isEmpty {
                        Text("No Flow has run.").padding(.vertical, 6)
                            .accessibilityIdentifier("task-flow-runs-empty")
                    }
                } else if let error = reading?.errorMessage {
                    Text("Flow execs could not be read: \(error)").padding(.vertical, 6)
                } else {
                    Text(task == nil ? "Task unavailable" : "Reading Flow execs…").padding(.vertical, 6)
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
        .accessibilityIdentifier("task-flow-runs")
    }
}

/// The Task's conversations and mechanical Execs, including headless work and
/// conversations without turns: raw records, shown under Debug. Its Flow
/// execs are listed by `FlowExecLog`.
struct TaskWorkView: View {
    let model: WorkModel
    let task: RoadmapTask
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            WorkSectionHeading(title: "Work") {
                Button("Refresh") { Task { await model.refresh() } }
            }
            if let work = model.taskWork[task.id].value {
                ForEach(work.sessions) { session in
                    row(session.interactive ? "Session" : "Run", id: session.id, name: session.title,
                        state: session.completedAt == nil ? "open" : "completed")
                }
                ForEach(work.execs) { exec in
                    row("Exec", id: exec.id, name: exec.command ?? "Unknown command",
                        state: exec.outcome ?? "unknown")
                }
                if work.sessions.isEmpty && work.execs.isEmpty {
                    Text("No recorded work.")
                }
            }
            if let error = model.taskWork[task.id].errorMessage {
                Text("Work could not be read: \(error)")
                Button("Retry") { Task { await model.refresh() } }
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

/// One Flow exec of the Task: its Flow, state, when it started and how long
/// it ran, opening to its id, launched graph, where it stands and each step
/// it started. Every exec reads the same way, whoever started it.
struct FlowRunView: View {
    let model: WorkModel
    let flow: TaskFlowMember
    @State private var inspected: UInt32?
    @Environment(\.palette) private var palette

    private var expanded: Bool { model.navigation.expandedFlowRuns.contains(flow.id) }
    private var run: FlowDetail? { model.flowRuns[flow.id] }
    private var started: Int64 { run?.steps.first?.startedAt ?? flow.updatedAt }

    /// How long the exec ran; a running one has no length yet.
    static func duration(_ seconds: Int64) -> String {
        let seconds = max(0, seconds)
        if seconds < 60 { return "\(seconds)s" }
        if seconds < 3600 { return "\(seconds / 60)m \(seconds % 60)s" }
        return "\(seconds / 3600)h \(seconds % 3600 / 60)m"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Button {
                if expanded {
                    model.navigation.expandedFlowRuns.remove(flow.id)
                } else {
                    model.navigation.expandedFlowRuns.insert(flow.id)
                }
            } label: {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                    Image(systemName: expanded ? "chevron.down" : "chevron.right")
                        .font(.system(size: 9, weight: .semibold))
                        .frame(width: 10)
                        .accessibilityHidden(true)
                    Text(flow.name).font(Typography.code(12)).foregroundStyle(palette.text)
                    Text(flow.state == .current ? "running" : flow.state.rawValue)
                        .foregroundStyle(tone?.ink ?? palette.textSecondary)
                    Spacer()
                    Text(Date(timeIntervalSince1970: TimeInterval(started))
                        .formatted(date: .abbreviated, time: .shortened))
                    if let ended = flow.endedAt {
                        Text(Self.duration(ended - started)).monospacedDigit()
                            .frame(minWidth: 52, alignment: .trailing)
                    }
                }
                .lineLimit(1)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityValue(expanded ? "Expanded" : "Collapsed")
            .accessibilityIdentifier("task-work-\(flow.id)")
            if expanded { detail.padding(.leading, 18) }
        }
        .padding(.vertical, 6)
    }

    private var tone: WorkTone? {
        switch flow.state {
        case .current: .running
        case .stopped: .blocked
        case .completed: nil
        }
    }

    @ViewBuilder
    private var detail: some View {
        Text(flow.id).font(Typography.code(10)).foregroundStyle(palette.textTertiary)
            .accessibilityIdentifier("flow-run-id-\(flow.id)")
        if let run {
            let progress = run.progress
            FlowDiagram(graph: run.graph, latest: progress, inspected: $inspected)
            Text([progress.reason, flowIterationLabel(run.iterations)]
                .compactMap { $0 }.joined(separator: " · "))
                .accessibilityIdentifier("flow-run-status-\(flow.id)")
            ForEach(run.steps) { step in
                HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                    Text(step.label).font(Typography.code(11))
                    if let iteration = flowIterationLabel(step.iterations) { Text(iteration).monospacedDigit() }
                    Spacer()
                    Text(step.outcome ?? (step.completedAt == nil ? "running" : "no recorded exit"))
                    if let completed = step.completedAt {
                        Text(Self.duration(completed - step.startedAt)).monospacedDigit()
                            .frame(minWidth: 52, alignment: .trailing)
                    }
                }
                .help(step.execId)
                .accessibilityIdentifier("flow-run-step-\(step.execId)")
            }
        } else {
            Text("Reading Flow…")
        }
    }
}
