import Loopflow
import SwiftUI

/// The one Task row: the title in full, then the ID and the status word (what
/// its program reports, else its lifecycle). A row whose program is blocked
/// adds one line saying what it reported. No
/// stripe, no border, no summary; selection is the host's fill.
struct TaskRow: View {
    let id: String
    let presentation: WorkModel.TaskPresentation
    let task: RoadmapTask?
    let status: SessionProgramStatus
    /// The Task's auto setting is on.
    var auto = false
    var selected = false
    /// The row belongs to another Wave than the one selected.
    var muted = false
    var slotHint: String?
    /// The hint shows while ⌘ is held, or while `hover` reports the row hovered.
    var showsSlotHint = false
    var hover: HoverFlag?
    @Environment(\.palette) private var palette

    private var secondary: Color { selected ? palette.workSelectedSecondaryText : palette.textSecondary }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text(presentation.title)
                .font(selected ? Typography.textStrong : Typography.text)
                .foregroundStyle(selected ? palette.workSelectedText : muted ? palette.textSecondary : palette.text)
                .fixedSize(horizontal: false, vertical: true)
                .allowsHitTesting(false)
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                identifier
                if let label = status.label, let tone = status.tone {
                    Text(verbatim: label)
                        .fontWeight(selected ? .semibold : nil)
                        .foregroundStyle(selected ? palette.workSelectedText : tone.ink(in: palette))
                        .help(Text(verbatim: status.help))
                        .accessibilityIdentifier("work-program-status-\(id)")
                } else if case .unavailable = status {
                    Image(systemName: "info.circle")
                        .help(Text(verbatim: status.help))
                        .accessibilityLabel(Text(verbatim: status.help))
                        .accessibilityIdentifier("work-program-status-unavailable-\(id)")
                } else if let task {
                    Text(verbatim: task.lifecycle.label.capitalized)
                        .accessibilityIdentifier("work-task-lifecycle-\(id)")
                }
                if auto {
                    Text("Auto")
                        .help("Auto is on: the run takes the interactive steps and lands the Task")
                        .accessibilityIdentifier("work-task-auto-\(id)")
                }
                // A row named by its list before planning answers says so.
                if let context = presentation.context ?? (task == nil ? "Loading planning…" : nil) {
                    Text(context).lineLimit(1)
                }
                Spacer(minLength: 0)
                if let slotHint {
                    if let hover, !showsSlotHint {
                        HoverRevealed(flag: hover) { Text(slotHint) }
                    } else {
                        Text(slotHint)
                    }
                }
            }
            .font(Typography.meta)
            .foregroundStyle(secondary)
            .lineLimit(1)
            if let ask = status.ask {
                Text(verbatim: ask)
                    .font(Typography.text)
                    .foregroundStyle(secondary)
                    .lineLimit(2)
                    .allowsHitTesting(false)
                    .accessibilityIdentifier("work-task-ask-\(id)")
            }
        }
        .padding(.vertical, Spacing.sm)
        .padding(.horizontal, Spacing.md)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder private var identifier: some View {
        if let identifier = task?.task.identifier ?? presentation.identifier {
            if let url = task?.reference.issueUrl {
                // Above the row selection target: the issue link never selects Work.
                Link(destination: url) {
                    Text(verbatim: identifier).foregroundStyle(selected ? palette.workSelectedText : palette.linkInk)
                }
                    .buttonStyle(WorkLinkStyle())
                    .help(url.absoluteString)
                    .accessibilityLabel("Open Task \(identifier)")
                    .accessibilityIdentifier("work-task-link-\(id)")
                    .fixedSize()
            } else {
                Text(verbatim: identifier)
                    .accessibilityIdentifier("work-task-id-\(id)")
            }
        }
    }
}
