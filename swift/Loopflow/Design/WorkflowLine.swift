import SwiftUI

/// A Graph in one line: its name, its top-level steps with a person before
/// each one where you take part, then Rust's sentence. For a Task the sentence
/// says where it stands and Rust places it (`currentStep`); for an outline, a
/// Graph no Task stands on yet, every step is ahead. This reads no step names.
public struct WorkflowLine: View {
    /// The Graph's name; nil where a menu or title beside the line says it.
    let name: String?
    let steps: [TaskGraph.Step]
    let tones: [Tone]
    /// Where the Task stands, or where you take part; nil in an unstarted
    /// Task's bar, whose pane says it.
    public let sentence: String?
    /// The Task's auto setting is on: its run continues through breakpoints.
    let auto: Bool
    let sentenceOnly: Bool
    @Environment(\.palette) private var palette

    /// How a step reads against where the Task stands.
    public enum Tone: Equatable {
        case done, ahead
        /// The step a run carries the Task through now.
        case running
        /// The step the Task stopped in or waits at for you.
        case waiting
    }

    /// A Task's Graph. `setup` stands in for the sentence before the run has
    /// started, such as setup's reason for not starting it yet.
    public init(graph: TaskGraph, auto: Bool, setup: String? = nil, sentenceOnly: Bool = false) {
        name = graph.name
        steps = graph.steps
        tones = Self.tones(graph)
        sentence = if graph.position == .at(step: TaskGraph.start), let setup { setup } else { graph.sentence }
        self.auto = auto
        self.sentenceOnly = sentenceOnly
    }

    /// The workflow an unstarted Task takes up, in its bar: the name, and the
    /// steps once the catalog has them.
    public init(name: String, outline: WorkflowOutline?) {
        self.name = name
        steps = outline?.steps ?? []
        tones = Array(repeating: .ahead, count: steps.count)
        sentence = nil
        auto = false
        sentenceOnly = false
    }

    /// A workflow where its name is already said: the steps, wrapping, and
    /// under them where you take part.
    public init(outline: WorkflowOutline) {
        name = nil
        steps = outline.steps
        tones = Array(repeating: .ahead, count: outline.steps.count)
        sentence = outline.sentence
        auto = false
        sentenceOnly = false
    }

    public static func tones(_ graph: TaskGraph) -> [Tone] {
        let running = if case .run(_, _, _, true) = graph.position { true } else { false }
        let ended = graph.position == .at(step: TaskGraph.end)
        return graph.steps.indices.map { index in
            guard let current = graph.currentStep else { return ended ? .done : .ahead }
            if index < current { return .done }
            if index > current { return .ahead }
            return running ? .running : .waiting
        }
    }

    public var body: some View {
        Group {
            if sentenceOnly, let sentence {
                sentenceText(sentence)
            } else if let name {
                ViewThatFits(in: .horizontal) {
                    HStack(spacing: Spacing.md) {
                        nameText(name)
                        HStack(spacing: Spacing.sm) { stepViews }
                        if let sentence { sentenceText(sentence).fixedSize() }
                    }
                    // When the steps do not fit, only the sentence stays.
                    if let sentence {
                        sentenceText(sentence)
                    } else {
                        nameText(name)
                    }
                }
            } else {
                VStack(alignment: .leading, spacing: Spacing.sm) {
                    WrappingRow(spacing: Spacing.sm) { stepViews }
                    if let sentence { sentenceText(sentence) }
                }
            }
        }
        .font(Typography.text)
        .accessibilityElement(children: .combine)
        .accessibilityLabel([name, sentence].compactMap { $0 }.joined(separator: ": "))
    }

    private func nameText(_ name: String) -> some View {
        Text(name).font(Typography.mono).fixedSize()
            .foregroundStyle(palette.text)
            .accessibilityIdentifier("task-workflow-bar-name")
    }

    private func sentenceText(_ sentence: String) -> some View {
        Text(sentence)
            .foregroundStyle(palette.textSecondary)
            .accessibilityIdentifier("task-workflow-bar-live")
    }

    private var stepViews: some View {
        ForEach(Array(steps.enumerated()), id: \.offset) { index, step in
            HStack(spacing: Spacing.xxs) {
                if step.breakpoint {
                    Image(systemName: "person.fill").font(Typography.meta)
                        .accessibilityLabel("you take part")
                }
                Text(step.name)
                    .font(tones[index] == .running || tones[index] == .waiting ? Typography.textStrong : Typography.text)
            }
            .foregroundStyle(ink(tones[index]))
            .fixedSize()
            .accessibilityIdentifier("task-workflow-step-\(index)")
        }
    }

    private func ink(_ tone: Tone) -> Color {
        switch tone {
        case .done: palette.textSecondary
        case .ahead: palette.textTertiary
        case .running: auto ? palette.success : palette.working
        case .waiting: palette.warning
        }
    }
}

/// Leading-aligned children in rows, starting a new row when the next child
/// does not fit the proposed width.
private struct WrappingRow: Layout {
    let spacing: CGFloat

    private func frames(_ subviews: Subviews, width: CGFloat) -> [CGRect] {
        var origin = CGPoint.zero
        var rowHeight: CGFloat = 0
        return subviews.map { subview in
            let size = subview.sizeThatFits(.unspecified)
            if origin.x > 0, origin.x + size.width > width {
                origin = CGPoint(x: 0, y: origin.y + rowHeight + spacing)
                rowHeight = 0
            }
            defer {
                origin.x += size.width + spacing
                rowHeight = max(rowHeight, size.height)
            }
            return CGRect(origin: origin, size: size)
        }
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let frames = frames(subviews, width: proposal.width ?? .infinity)
        return CGSize(width: frames.map(\.maxX).max() ?? 0, height: frames.map(\.maxY).max() ?? 0)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        for (subview, frame) in zip(subviews, frames(subviews, width: bounds.width)) {
            subview.place(at: CGPoint(x: bounds.minX + frame.minX, y: bounds.minY + frame.minY), proposal: .unspecified)
        }
    }
}
