#if os(macOS)
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

    /// "Waiting, at demo." then why: the word the Task's row says with no
    /// program reporting, the top-level step the workflow bar makes strong, the
    /// next move's reason in one sentence.
    static func state(_ task: RoadmapTask, graph: TaskGraph?) -> (lead: String, reason: String) {
        let word = TaskRowStatus(task: task).word ?? ""
        let step = graph.flatMap { graph in graph.currentStep.map { graph.steps[$0].name } }
        let lead = step.map { "\(word), at \($0)." } ?? "\(word)."
        let why = task.nextMove.reason.trimmingCharacters(in: .whitespacesAndNewlines)
        return (lead, why.isEmpty || why.hasSuffix(".") ? why : why + ".")
    }

    /// What the Task's row says it needs, under the word the row would show
    /// with no program reporting: "Blocked: reason". Nil when the state line
    /// above already ends with that reason.
    static func need(_ task: RoadmapTask) -> String? {
        let standing = TaskRowStatus(task: task)
        func bare(_ text: String) -> String {
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.hasSuffix(".") ? String(trimmed.dropLast()) : trimmed
        }
        guard let line = standing.line, bare(line) != bare(task.nextMove.reason) else { return nil }
        return standing.source == .condition ? "\(standing.word ?? ""): \(line)" : line
    }

    /// Names separated by commas, each a link, so a narrow pane wraps the list.
    static func links(_ items: [(title: String, url: URL)], mono: Bool) -> AttributedString {
        var result = AttributedString()
        for (index, item) in items.enumerated() {
            if index > 0 { result += AttributedString(", ") }
            var link = AttributedString(item.title)
            link.link = item.url
            if mono { link.font = Typography.mono }
            result += link
        }
        return result
    }

    /// The scheme an outline link carries, so the page scrolls rather than opens.
    static let outlineScheme = "loopflow-brief"
}

/// The Brief page: the Task's name, its state in a sentence, a small table
/// of its PR, the Tasks it names and its outline, then the text.
struct TaskBriefView: View {
    @Bindable var store: TaskFilesStore
    let task: RoadmapTask
    let graph: TaskGraph?
    @Environment(\.palette) private var palette
    @State private var position = ScrollPosition()
    @State private var updated = false

    private var description: String { task.task.description }

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: Spacing.md) {
                    Text(task.task.name).serif(.title).foregroundStyle(palette.text)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityAddTraits(.isHeader)
                        .accessibilityIdentifier("brief-title")
                    status
                    facts
                    if updated {
                        Text("The brief changed since you last read it.").font(Typography.text).foregroundStyle(palette.warning)
                            .accessibilityIdentifier("brief-changed")
                    }
                    Divider()
                    if description.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        Text("No brief recorded. Edit it with `lf task edit \(task.task.identifier)`.")
                            .font(Typography.text).foregroundStyle(palette.textSecondary)
                    } else {
                        MarkdownBlocks(source: description).equatable()
                            .frame(maxWidth: 680, alignment: .leading)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(Spacing.md).textSelection(.enabled)
            }
            .scrollPosition($position)
            .onScrollGeometryChange(for: CGPoint.self, of: \.contentOffset) { _, offset in store.briefOffset = offset }
            .environment(\.openURL, OpenURLAction { url in
                guard url.scheme == TaskBrief.outlineScheme, let block = url.host.flatMap(Int.init) else { return .systemAction }
                withAnimation(.easeOut(duration: 0.15)) { proxy.scrollTo(MarkdownBlocks.anchor(block), anchor: .top) }
                return .handled
            })
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
        let state = TaskBrief.state(task, graph: graph)
        return VStack(alignment: .leading, spacing: Spacing.xs) {
            let lead = Text(state.lead).font(Typography.textStrong).foregroundStyle(palette.text)
            let reason = Text(state.reason.isEmpty ? "" : " " + state.reason).font(Typography.text).foregroundStyle(palette.textSecondary)
            Text("\(lead)\(reason)")
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityIdentifier("brief-state")
            if let need = TaskBrief.need(task) {
                Text(verbatim: need)
                    .font(Typography.text).foregroundStyle(palette.warning)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    /// Label beside value, every value at `text`, every row wrapping.
    private var facts: some View {
        let delivery = TaskDelivery.describe(record: task.pr, taskOpen: !task.isCompleted, reading: store.pullRequest.pullRequest,
            error: store.pullRequest.error, readAt: store.pullRequest.readAt, localHead: store.changes?.headCommit)
        let related = TaskBrief.related(in: description, excluding: task.task.identifier)
        let outline = TaskBrief.outline(description)
        return Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 12, verticalSpacing: 6) {
            GridRow {
                label("Pull request")
                Button { store.page = .changes; store.noteInteraction() } label: {
                    Text(delivery.headline).multilineTextAlignment(.leading).fixedSize(horizontal: false, vertical: true)
                }
                .buttonStyle(WorkPlainButtonStyle()).help("Show Diff")
                .accessibilityIdentifier("brief-delivery")
            }
            if !related.isEmpty {
                GridRow {
                    label("Related")
                    Text(TaskBrief.links(related.compactMap { identifier in url(identifier).map { (identifier, $0) } }, mono: true))
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityIdentifier("brief-related")
                }
            }
            if !outline.isEmpty {
                GridRow {
                    label("In this brief")
                    Text(TaskBrief.links(outline.compactMap { heading in
                        URL(string: "\(TaskBrief.outlineScheme)://\(heading.block)").map { (heading.title, $0) }
                    }, mono: false))
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier("brief-outline")
                }
            }
            if let due = task.task.dueDate {
                GridRow {
                    label("Due")
                    Text(due)
                }
            }
        }
        .font(Typography.text).foregroundStyle(palette.text)
        .tint(palette.linkInk)
    }

    private func label(_ text: String) -> some View {
        Text(text).font(Typography.text).foregroundStyle(palette.textSecondary)
            .gridColumnAlignment(.leading).fixedSize()
    }

    private func url(_ identifier: String) -> URL? {
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "-._~"))
        let repository = URL(fileURLWithPath: task.task.repo).lastPathComponent
            .addingPercentEncoding(withAllowedCharacters: allowed) ?? ""
        return URL(string: "loopflow://\(repository)/task/\(identifier)")
    }
}
#endif
