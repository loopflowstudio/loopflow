#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// What a brief offers besides its text: the Tasks it names and its headings.
enum TaskBrief {
    /// Task identifiers the brief mentions, in first-mention order, without the Task itself.
    static func related(in description: String, excluding own: String) -> [String] {
        let pattern = #/\b[A-Z][A-Z0-9]{1,9}-[0-9]+\b/#
        var seen: Set<String> = [own]
        var result: [String] = []
        for match in description.matches(of: pattern) {
            let identifier = String(match.output)
            if seen.insert(identifier).inserted { result.append(identifier) }
        }
        return result
    }

    struct Heading: Equatable {
        let block: Int
        let title: String
    }

    /// Headings and a bold `Acceptance:` lead, by block index; fewer than two is no outline.
    static func outline(_ source: String) -> [Heading] {
        let headings = MarkdownBlocks.blocks(source).enumerated().compactMap { index, block -> Heading? in
            let title = String(block.text.characters).trimmingCharacters(in: .whitespaces)
            switch block.kind {
            case .heading: return Heading(block: index, title: title)
            case .paragraph where title.hasPrefix("Acceptance"):
                return Heading(block: index, title: "Acceptance")
            default: return nil
            }
        }
        return headings.count >= 2 ? headings : []
    }
}

/// The Brief page: state first, then the Tasks it names, an outline, the text.
struct TaskBriefView: View {
    @Bindable var store: TaskFilesStore
    let task: RoadmapTask
    @Environment(\.palette) private var palette
    @State private var position = ScrollPosition()
    @State private var updated = false

    private var description: String { task.task.description }

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    status
                    let related = TaskBrief.related(in: description, excluding: task.task.identifier)
                    if !related.isEmpty { relatedTasks(related) }
                    let outline = TaskBrief.outline(description)
                    if !outline.isEmpty { outlineView(outline, proxy: proxy) }
                    if updated {
                        Text("Brief updated since you last read it").font(Typography.meta).foregroundStyle(palette.warning)
                    }
                    Divider()
                    if description.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        Text("No brief recorded · edit with `lf task edit \(task.task.identifier)`")
                            .font(Typography.meta).foregroundStyle(palette.textSecondary)
                    } else {
                        MarkdownBlocks(source: description).equatable()
                            .frame(maxWidth: 680, alignment: .leading)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(14).textSelection(.enabled)
            }
            .scrollPosition($position)
            .onScrollGeometryChange(for: CGPoint.self, of: \.contentOffset) { _, offset in store.briefOffset = offset }
        }
        .accessibilityIdentifier("browser-task-overview")
        .onAppear {
            let digest = description.hashValue
            updated = store.briefShown.map { $0 != digest } ?? false
            store.briefShown = digest
            position.scrollTo(point: store.briefOffset)
        }
    }

    private var status: some View {
        let runtime = task.runtime?.status.label ?? "unstarted"
        let step = task.execution?.step.map { " · \($0)" } ?? ""
        let delivery = TaskDelivery.describe(record: task.pr, taskOpen: !task.isCompleted, reading: store.pullRequest.pullRequest,
            error: store.pullRequest.error, readAt: store.pullRequest.readAt, localHead: store.changes?.headCommit)
        return VStack(alignment: .leading, spacing: 4) {
            Text("\(runtime.capitalized)\(step)").font(Typography.textStrong)
                .accessibilityIdentifier("brief-state")
            Text(task.nextMove.reason).font(Typography.text).foregroundStyle(palette.textSecondary)
                .fixedSize(horizontal: false, vertical: true)
            if task.condition.state != .clear {
                Text("\(task.condition.state.rawValue.capitalized) · \(task.condition.reason)")
                    .font(Typography.meta).foregroundStyle(palette.warning)
            }
            HStack(spacing: 10) {
                Button { store.page = .changes; store.noteInteraction() } label: {
                    Label(delivery.headline, systemImage: "arrow.triangle.pull").lineLimit(1)
                }
                .buttonStyle(WorkPlainButtonStyle()).help("Show Diff")
                if let due = task.task.dueDate { Text("Due \(due)") }
            }
            .font(Typography.meta).foregroundStyle(palette.textSecondary)
        }
    }

    private func relatedTasks(_ identifiers: [String]) -> some View {
        HStack(spacing: 6) {
            Text("Related").font(Typography.meta).foregroundStyle(palette.textSecondary)
            ForEach(identifiers, id: \.self) { identifier in
                Button(identifier) { open(identifier) }
                    .buttonStyle(.plain)
                    .font(Typography.mono)
                    .padding(.horizontal, 6).padding(.vertical, 2)
                    .background(palette.surfaceMuted, in: Capsule())
                    .help("Open \(identifier)")
            }
        }
    }

    private func open(_ identifier: String) {
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "-._~"))
        let repository = URL(fileURLWithPath: task.task.repo).lastPathComponent
            .addingPercentEncoding(withAllowedCharacters: allowed) ?? ""
        if let url = URL(string: "loopflow://\(repository)/task/\(identifier)") { NSWorkspace.shared.open(url) }
    }

    private func outlineView(_ outline: [TaskBrief.Heading], proxy: ScrollViewProxy) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            ForEach(outline, id: \.block) { heading in
                Button(heading.title) {
                    withAnimation(.easeOut(duration: 0.15)) { proxy.scrollTo(MarkdownBlocks.anchor(heading.block), anchor: .top) }
                }
                .buttonStyle(WorkPlainButtonStyle()).font(Typography.meta).lineLimit(1)
            }
        }
    }
}
#endif
