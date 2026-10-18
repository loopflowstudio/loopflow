import Loopflow
import SwiftUI

extension TaskWorktreeSnapshot {
    func sidebarMachine(localMachineId: String?) -> String? {
        guard let machineId, let localMachineId else { return "Machine unknown" }
        guard machineId != localMachineId else { return nil }
        guard let machineLabel, !machineLabel.isEmpty else { return "Remote · name unavailable" }
        return ProgramStatusReport.displaySafe(machineLabel)
    }
}

struct TaskRowMetadata: View {
    let task: RoadmapTask
    let status: SessionProgramStatus
    let localMachineId: String?
    var selected = false
    var slotHint: String?
    var sourceAge: String?
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: 5) {
            if let url = task.reference.issueUrl {
                destination(task.task.identifier, url: url, kind: "task")
            } else {
                Text(verbatim: task.task.identifier)
                    .accessibilityIdentifier("work-task-id-\(task.id)")
            }
            if let label = status.label, let tone = status.tone {
                Text(verbatim: label)
                    .foregroundStyle(tone.ink(in: palette))
                    .help(Text(verbatim: status.help))
                    .accessibilityIdentifier("work-program-status-\(task.id)")
            } else if case .unavailable = status {
                Image(systemName: "info.circle")
                    .help(Text(verbatim: status.help))
                    .accessibilityLabel(Text(verbatim: status.help))
                    .accessibilityIdentifier("work-program-status-unavailable-\(task.id)")
            }
            if let pr = task.pr?.publication?.github {
                destination("#\(pr.number)", url: pr.url, kind: "pr")
            }
            // Only recorded checkout ownership is shown; assignment is not execution.
            if let workspace = task.reference.workspace {
                if let machine = workspace.sidebarMachine(localMachineId: localMachineId) {
                    Text(verbatim: machine).help(Text(verbatim: "Recorded checkout Machine · \(machine)"))
                        .accessibilityIdentifier("work-machine-\(task.id)")
                }
            } else if task.runtime?.started == true {
                Text("Machine unknown").accessibilityIdentifier("work-machine-\(task.id)")
            }
            Spacer(minLength: 0)
            Text(slotHint ?? sourceAge ?? "").font(Typography.code(10)).frame(width: 28, alignment: .trailing)
        }
        .font(Typography.code(10))
        .foregroundStyle(selected ? palette.workSelectedSecondaryText : palette.textSecondary)
        .lineLimit(1)
    }

    private func destination(_ title: String, url: URL, kind: String) -> some View {
        // Above the row selection target: external links never select Work.
        Link(destination: url) { Text(verbatim: title).foregroundStyle(selected ? palette.workSelectedText : palette.linkInk) }
            .buttonStyle(WorkLinkStyle())
            .help(url.absoluteString)
            .accessibilityLabel("Open \(kind == "pr" ? "PR" : "Task") \(title)")
            .accessibilityIdentifier("work-\(kind)-link-\(task.id)")
            .fixedSize()
    }
}
