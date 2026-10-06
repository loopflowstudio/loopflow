import Loopflow
import SwiftUI

/// A Task's workflow: the nodes where a person takes part, joined by the
/// Flows that move between them. Rust owns the graph and the position; this
/// draws them, chooses an edge through `lf task run` and sets a node
/// through `lf task move`.
struct WorkflowView: View {
    let model: PodiumModel
    let task: RoadmapTask
    let wave: WaveSnapshot
    let workflow: Workflow
    @Environment(\.palette) private var palette

    private var draft: TaskFlowDraft? { model.navigation.flowDrafts[task.id] }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            WorkspaceSectionHeading(title: "Workflow") {
                Text(workflow.name).font(Typography.code(12)).foregroundStyle(palette.textSecondary)
            }
            VStack(alignment: .leading, spacing: Spacing.sm) {
                WorkflowGraph(
                    nodes: workflow.nodes, edges: workflow.edges, position: workflow.position,
                    choices: workflow.outgoing, unavailable: unavailable,
                    hint: { "lf task run \(task.task.identifier) \($0.launchName) · to \($0.to)" },
                    choose: { edge in Task { await model.startFlow(edge.launchName, task: task, wave: wave) } })
                Text(position)
                    .font(Typography.body(13))
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("task-workflow-position")
                HStack(spacing: Spacing.sm) {
                    // Put the Task at a node without running anything.
                    Menu("Move to") {
                        ForEach(["start"] + workflow.nodes.map(\.name) + ["end"], id: \.self) { node in
                            Button(node) { Task { await model.moveTask(to: node, task: task, wave: wave) } }
                                .accessibilityIdentifier("task-workflow-move-\(node)")
                        }
                    }
                    .fixedSize()
                    .disabled(draft?.acting == true)
                    .help("lf task move \(task.task.identifier) <node>")
                    .accessibilityIdentifier("task-workflow-move")
                    if draft?.acting == true { ProgressView().controlSize(.small) }
                }
                // Linear called the Task complete while it is active here.
                if let conflict = task.runtime?.planningConflict {
                    HStack(spacing: Spacing.sm) {
                        Text(conflict)
                            .font(Typography.body(12))
                            .foregroundStyle(WorkspaceTone.blocked.ink)
                            .textSelection(.enabled)
                            .accessibilityIdentifier("task-workflow-conflict")
                        Button("Complete anyway") {
                            Task { await model.moveTask(to: "end", force: true, task: task, wave: wave) }
                        }
                        .buttonStyle(WorkspaceOutlineButtonStyle())
                        .disabled(draft?.acting == true)
                        .help("lf task move \(task.task.identifier) end --force")
                        .accessibilityIdentifier("task-workflow-force-end")
                    }
                }
                if let error = draft?.error {
                    Text(error)
                        .font(Typography.body(12))
                        .foregroundStyle(WorkspaceTone.blocked.ink)
                        .textSelection(.enabled)
                        .accessibilityIdentifier("task-workflow-error")
                }
            }
            .workspacePanel(padding: 13)
        }
        .accessibilityIdentifier("task-workflow")
    }

    /// Why no edge can be started now.
    private var unavailable: String? {
        if draft?.acting == true { return "Starting" }
        return task.flow.control(.start)?.unavailable
    }

    private var position: String {
        switch workflow.position {
        case .node(let node):
            if node == "end" { return "Ended" }
            if let skill = workflow.nodes.first(where: { $0.name == node })?.skill {
                return "Waiting on you at \(node) · \(skill)"
            }
            return "Not started"
        case .edge:
            guard let (_, edge, running) = workflow.onEdge else { return "On an edge" }
            return "\(running ? "Running" : "Stopped on") \(edge.launchName) · \(edge.from) to \(edge.to)"
        }
    }
}

/// Every Flow run in the Task's checkout, newest first, whatever started it.
/// It is not the Workflow's history: a run that carried an edge and one
/// started ad hoc are the same kind of row.
struct FlowRunLog: View {
    let model: PodiumModel
    let task: RoadmapTask
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            WorkspaceSectionHeading("Flow runs")
            VStack(alignment: .leading, spacing: Spacing.sm) {
                if let flows = model.taskWork[task.id].value?.flows {
                    ForEach(flows.reversed()) { flow in
                        FlowRunView(model: model, flow: flow)
                    }
                    if flows.isEmpty {
                        Text("No Flow has run.").accessibilityIdentifier("task-flow-runs-empty")
                    }
                } else if model.taskWork[task.id].errorMessage == nil {
                    Text("Reading Flow runs…")
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .workspacePanel(padding: 13)
        }
        .font(Typography.body(12))
        .foregroundStyle(palette.textSecondary)
        .textSelection(.enabled)
        .accessibilityIdentifier("task-flow-runs")
    }
}

/// The Task's conversations and mechanical Execs, including headless work and
/// conversations without turns. Its Flow runs are listed by `FlowRunLog`.
struct TaskWorkView: View {
    let model: PodiumModel
    let task: RoadmapTask
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            WorkspaceSectionHeading(title: "Work") {
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

/// One Flow exec of the Task, opened to its launched graph, where it stands
/// and each step it started. Every run reads the same way, whoever started it.
struct FlowRunView: View {
    let model: PodiumModel
    let flow: TaskFlowMember
    @State private var inspected: UInt32?
    @Environment(\.palette) private var palette

    private var expanded: Bool { model.navigation.expandedFlowRuns.contains(flow.id) }

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
                    Text("Flow")
                    Text(flow.name).font(Typography.code(12))
                    Spacer()
                    Text(flow.state.rawValue)
                    Text(flow.id).font(Typography.code(10)).help(flow.id)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityValue(expanded ? "Expanded" : "Collapsed")
            .accessibilityIdentifier("task-work-\(flow.id)")
            if expanded { detail.padding(.leading, 18) }
        }
    }

    @ViewBuilder
    private var detail: some View {
        if let run = model.flowRuns[flow.id] {
            let progress = run.progress
            FlowDiagram(graph: run.graph, latest: progress, inspected: $inspected)
            Text([progress.reason, flowIterationLabel(run.iterations).map { "iteration \($0)" }]
                .compactMap { $0 }.joined(separator: " · "))
                .accessibilityIdentifier("flow-run-status-\(flow.id)")
            ForEach(run.steps) { step in
                HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                    Text(step.label).font(Typography.code(11))
                    if let iteration = flowIterationLabel(step.iterations) { Text(iteration).monospacedDigit() }
                    Spacer()
                    Text(step.outcome ?? (step.completedAt == nil ? "running" : "no recorded exit"))
                    Text(step.execId).font(Typography.code(10)).help(step.execId)
                }
                .accessibilityIdentifier("flow-run-step-\(step.execId)")
            }
        } else {
            Text("Reading Flow…")
        }
    }
}
