import Loopflow
import SwiftUI

struct TaskRowMetadata: View {
    let task: RoadmapTask
    let status: SessionProgramStatus
    var selected = false
    var slotHint: String?
    /// The hint shows while ⌘ is held, or while `hover` reports the row hovered.
    var showsSlotHint = false
    var hover: HoverFlag?
    var sourceAge: String?
    @Environment(\.palette) private var palette

    /// Hint and age both stay rendered; hover only changes which one is opaque.
    @ViewBuilder private var trailing: some View {
        if let slotHint, showsSlotHint || hover == nil {
            Text(slotHint)
        } else if let slotHint, let hover {
            ZStack(alignment: .trailing) {
                HoverRevealed(flag: hover) { Text(slotHint) }
                HoverRevealed(flag: hover, whileHovered: false) { Text(sourceAge ?? "") }
            }
        } else {
            Text(sourceAge ?? "")
        }
    }

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
            Text(verbatim: ProgramStatusReport.displaySafe(task.runtime?.owner?.summary ?? "Unowned"))
                .accessibilityIdentifier("work-owner-\(task.id)")
            Spacer(minLength: 0)
            // Reserve the shortcut glyph's height even when only an age (or nothing) is shown.
            Text("⌘9").hidden().frame(width: 28, alignment: .trailing)
                .overlay(alignment: .trailing) { trailing }
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
