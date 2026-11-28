import SwiftUI

/// A Graph's sentence, or an outline's wrapped steps with where a person takes
/// part underneath. Steps and sentence come from Rust; this reads no step names.
public struct WorkflowLine: View {
    let steps: [TaskGraph.Step]
    /// Where the Task stands, or where you take part.
    public let sentence: String
    @Environment(\.palette) private var palette

    /// A Task's Graph sentence, with `setup` standing in before the run has started.
    public init(graph: TaskGraph, setup: String? = nil) {
        steps = []
        sentence = if graph.position == .at(step: TaskGraph.start), let setup {
            setup
        } else {
            graph.sentence
        }
    }

    /// A workflow where its name is already said: the steps, wrapping, and
    /// under them where you take part.
    public init(outline: WorkflowOutline) {
        steps = outline.steps
        sentence = outline.sentence
    }

    public var body: some View {
        Group {
            if steps.isEmpty {
                sentenceText
            } else {
                VStack(alignment: .leading, spacing: Spacing.sm) {
                    WrappingRow(spacing: Spacing.sm) { stepViews }
                    sentenceText
                }
            }
        }
        .font(Typography.text)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(sentence)
    }

    private var sentenceText: some View {
        Text(sentence)
            .foregroundStyle(palette.textSecondary)
            .accessibilityIdentifier("task-workflow-bar-live")
    }

    private var stepViews: some View {
        ForEach(Array(steps.enumerated()), id: \.offset) { index, step in
            WorkflowStepLabel(step: step)
            .foregroundStyle(palette.textTertiary)
            .fixedSize()
            .accessibilityIdentifier("task-workflow-step-\(index)")
        }
    }
}

struct WorkflowStepLabel: View {
    let step: TaskGraph.Step

    var body: some View {
        HStack(spacing: Spacing.xxs) {
            if step.kind != .stage {
                Image(systemName: "person.fill").font(Typography.meta)
                    .accessibilityLabel(step.kind == .breakpoint ? "Breakpoint: \(step.name)" : "You take part")
            }
            if step.kind != .breakpoint { Text(step.name) }
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
