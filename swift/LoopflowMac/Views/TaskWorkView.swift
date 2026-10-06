import Loopflow
import SwiftUI

/// A Task's workflow: the stages where a person takes part, joined by the
/// Flows that move between them. Rust owns the graph and the position; this
/// draws them, chooses an edge through `lf task run` and sets a stage
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
                WorkflowGraphRow(
                    stages: workflow.stages, edges: workflow.edges,
                    current: { if case .stage(let stage) = workflow.position { stage } else { nil } }(),
                    running: workflow.onEdge.flatMap { $0.running ? $0.index : nil },
                    stopped: workflow.onEdge.flatMap { $0.running ? nil : $0.index })
                Text(position)
                    .font(Typography.body(13))
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("task-workflow-position")
                HStack(spacing: Spacing.sm) {
                    ForEach(workflow.choices, id: \.index) { index, edge in
                        Button(edge.name ?? edge.flow.map { "Run \($0)" } ?? "Finish") {
                            Task { await model.startFlow(edge.launchName, task: task, wave: wave) }
                        }
                        .buttonStyle(WorkspaceOutlineButtonStyle())
                        .disabled(unavailable != nil)
                        .help(unavailable ?? "lf task run \(task.task.identifier) \(edge.launchName) · to \(edge.to)")
                        .accessibilityIdentifier("task-workflow-run-\(index)")
                    }
                    // Put the Task at a stage without running anything.
                    Menu("Move to") {
                        ForEach(["start"] + workflow.stages.map(\.name) + ["end"], id: \.self) { stage in
                            Button(stage) { Task { await model.moveTask(to: stage, task: task, wave: wave) } }
                                .accessibilityIdentifier("task-workflow-move-\(stage)")
                        }
                    }
                    .fixedSize()
                    .disabled(draft?.acting == true)
                    .help("lf task move \(task.task.identifier) <stage>")
                    .accessibilityIdentifier("task-workflow-move")
                    if draft?.acting == true { ProgressView().controlSize(.small) }
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
        case .stage(let stage):
            if stage == "end" { return "Ended" }
            if let skill = workflow.stages.first(where: { $0.name == stage })?.skill {
                return "Waiting on you at \(stage) · \(skill)"
            }
            return "Not started"
        case .edge:
            guard let (_, edge, running) = workflow.onEdge else { return "On an edge" }
            return "\(running ? "Running" : "Stopped on") \(edge.launchName) · \(edge.from) to \(edge.to)"
        }
    }
}

/// A workflow's stages in authored order, each followed by the edges leaving it.
struct WorkflowGraphRow: View {
    let stages: [Workflow.Stage]
    let edges: [Workflow.Edge]
    /// The stage a Task waits at, and the edge it is running, when a Task has taken the workflow up.
    var current: String?
    var running: Int?
    var stopped: Int?
    @Environment(\.palette) private var palette

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: Spacing.sm) {
                ForEach(["start"] + stages.map(\.name) + ["end"], id: \.self) { stage in
                    WorkspaceChip(text: stage, tone: current == stage ? .human : .neutral)
                        .accessibilityIdentifier("task-workflow-stage-\(stage)")
                        .accessibilityValue(current == stage ? "Current" : "")
                    ForEach(edges.leaving(stage), id: \.index) { index, edge in
                        Text("→ \(edge.name ?? edge.flow ?? "no Flow") → \(edge.to)")
                            .font(Typography.code(11))
                            .foregroundStyle(
                                running == index ? WorkspaceTone.running.ink
                                    : stopped == index ? WorkspaceTone.blocked.ink : palette.textTertiary)
                            .accessibilityValue(running == index ? "Running" : stopped == index ? "Stopped" : "")
                            .accessibilityIdentifier("task-workflow-edge-\(index)")
                    }
                }
            }
        }
    }
}

/// Every retained owner, including headless work and conversations without turns.
struct TaskWorkView: View {
    let model: PodiumModel
    let task: RoadmapTask
    let wave: WaveSnapshot
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            WorkspaceSectionHeading(title: "Work") {
                Button("Refresh") { Task { await model.refreshShownTaskWork() } }
                    .disabled(model.taskWork.inFlight.contains(task.id))
            }
            if let work = model.taskWork[task.id].value {
                ForEach(work.sessions) { session in
                    row(session.interactive ? "Session" : "Run", id: session.id, name: session.title,
                        state: session.completedAt == nil ? "open" : "completed")
                }
                ForEach(work.flows) { flow in
                    FlowRunView(model: model, flow: flow, wave: wave)
                }
                ForEach(work.execs) { exec in
                    row("Exec", id: exec.id, name: exec.command ?? "Unknown command",
                        state: exec.outcome ?? "unknown")
                }
                if work.sessions.isEmpty && work.flows.isEmpty && work.execs.isEmpty {
                    Text("No recorded work.")
                }
            }
            if let error = model.taskWork[task.id].errorMessage {
                Text("Work could not be read: \(error)")
                Button("Retry") { Task { await model.loadTaskWork(task: task, wave: wave) } }
            }
        }
        .font(Typography.body(12))
        .foregroundStyle(palette.textSecondary)
        .textSelection(.enabled)
        .task(id: task.id) { await model.loadTaskWork(task: task, wave: wave) }
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

/// One Flow run of the Task, opened to its launched graph, where it stands
/// and each step it started. Every run reads the same way, whoever started it.
struct FlowRunView: View {
    let model: PodiumModel
    let flow: TaskFlowMember
    let wave: WaveSnapshot
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
                    Task { await model.loadFlowRun(flow.id, wave: wave) }
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
        let reading = model.flowRuns[flow.id]
        if let run = reading.value {
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
        }
        if let error = reading.errorMessage {
            Text("Flow could not be read: \(error)")
        } else if reading.value == nil {
            Text("Reading Flow…")
        }
    }
}
