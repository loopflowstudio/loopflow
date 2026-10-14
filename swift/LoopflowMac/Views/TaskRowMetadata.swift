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
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: 5) {
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
            if let url = task.reference.issueUrl {
                destination(task.task.identifier, url: url, kind: "task")
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
        }
        .font(Typography.body(10))
        .foregroundStyle(palette.textSecondary)
        .lineLimit(1)
    }

    private func destination(_ title: String, url: URL, kind: String) -> some View {
        // A sibling control to the title button: external links never select Work.
        Link(destination: url) { Text(verbatim: title) }
            .buttonStyle(WorkLinkStyle())
            .help(url.absoluteString)
            .accessibilityLabel("Open \(kind == "pr" ? "PR" : "Task") \(title)")
            .accessibilityIdentifier("work-\(kind)-link-\(task.id)")
            .fixedSize()
    }
}
