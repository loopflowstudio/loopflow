import Loopflow
import SwiftUI

/// Every retained owner, including headless work and conversations without turns.
struct TaskWorkView: View {
    let model: PodiumModel
    let task: RoadmapTask
    let wave: WaveSnapshot
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            WorkspaceSectionHeading(title: "Work") {
                Button("Refresh") { Task { await model.loadTaskWork(task: task, wave: wave) } }
                    .disabled(model.taskWork.inFlight.contains(task.id))
            }
            if let work = model.taskWork[task.id].value {
                ForEach(work.sessions) { session in
                    row("Session", id: session.id, name: session.title,
                        state: session.completedAt == nil ? "open" : "completed")
                }
                ForEach(work.flows) { flow in
                    row("Flow", id: flow.id, name: flow.name, state: flow.state.rawValue)
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
