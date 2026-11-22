import Loopflow
import SwiftUI

/// The one Task row: the title in full, then the ID and the status word, and
/// one line when the Task needs someone. `TaskStanding` decides both. No
/// stripe, no border, no summary; selection is the host's fill.
struct TaskRow: View {
    let id: String
    let presentation: WorkModel.TaskPresentation
    let task: RoadmapTask?
    let standing: TaskStanding
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

    init(id: String, presentation: WorkModel.TaskPresentation, task: RoadmapTask?, status: SessionProgramStatus,
         auto: Bool = false, selected: Bool = false, muted: Bool = false,
         slotHint: String? = nil, showsSlotHint: Bool = false, hover: HoverFlag? = nil) {
        self.id = id
        self.presentation = presentation
        self.task = task
        standing = TaskStanding(task: task, status: status)
        self.auto = auto
        self.selected = selected
        self.muted = muted
        self.slotHint = slotHint
        self.showsSlotHint = showsSlotHint
        self.hover = hover
    }

    /// A row for a host that holds the Task and no Session readings: the Portfolio.
    init(task: RoadmapTask, context: String? = nil) {
        self.init(id: task.id,
                  presentation: .init(title: task.task.name, identifier: task.task.identifier, context: context),
                  task: task, status: .unknown)
    }

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
                if let word = standing.word {
                    Text(verbatim: word)
                        .fontWeight(selected && standing.tone != nil ? .semibold : nil)
                        .foregroundStyle(standing.tone.map { selected ? palette.workSelectedText : $0.ink(in: palette) } ?? secondary)
                        .help(Text(verbatim: standing.help ?? word))
                        .accessibilityIdentifier(wordIdentifier)
                } else if let help = standing.help {
                    Image(systemName: "info.circle")
                        .help(Text(verbatim: help))
                        .accessibilityLabel(Text(verbatim: help))
                        .accessibilityIdentifier("work-program-status-unavailable-\(id)")
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
            if let line = standing.line {
                Text(verbatim: line)
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

    /// The whole row opens the Task; the ID's link stays above the target.
    func opening(_ action: @escaping () -> Void) -> some View {
        background {
            Button(action: action) { Color.clear.contentShape(Rectangle()) }
                .buttonStyle(.plain)
                .accessibilityLabel(presentation.title)
                .accessibilityIdentifier("task-row-open-\(id)")
        }
    }

    private var wordIdentifier: String {
        switch standing.source {
        case .program, .unavailable, .none: "work-program-status-\(id)"
        case .condition: "work-task-condition-\(id)"
        case .lifecycle: "work-task-lifecycle-\(id)"
        }
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
