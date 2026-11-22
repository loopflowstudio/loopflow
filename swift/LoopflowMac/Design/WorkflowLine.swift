import Loopflow
import SwiftUI

/// A Task's Graph in one line: its name, its top-level steps with a person
/// before each one that waits for you, then Rust's sentence for where the Task
/// stands. Rust places the Task (`currentStep`); this reads no step names.
/// When the steps do not fit, only the sentence stays.
struct WorkflowLine: View {
    let graph: TaskGraph
    /// The Task's auto setting is on: its run takes the interactive steps.
    let auto: Bool
    /// Stands in for the sentence before the run has started, such as setup's
    /// reason for not starting it yet.
    var setup: String? = nil
    @Environment(\.palette) private var palette

    /// How a step reads against where the Task stands.
    enum Tone: Equatable {
        case done, ahead
        /// The step a run carries the Task through now.
        case running
        /// The step the Task stopped in or waits at for you.
        case waiting
    }

    static func tones(_ graph: TaskGraph) -> [Tone] {
        let running = if case .run(_, _, _, true) = graph.position { true } else { false }
        let ended = graph.position == .at(step: TaskGraph.end)
        return graph.steps.indices.map { index in
            guard let current = graph.currentStep else { return ended ? .done : .ahead }
            if index < current { return .done }
            if index > current { return .ahead }
            return running ? .running : .waiting
        }
    }

    var sentence: String {
        if graph.position == .at(step: TaskGraph.start), let setup { return setup }
        return graph.sentence
    }

    var body: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: Spacing.md) {
                name
                steps
                Text(sentence).fixedSize()
                    .foregroundStyle(palette.textSecondary)
                    .accessibilityIdentifier("task-workflow-bar-live")
            }
            Text(sentence)
                .lineLimit(1).truncationMode(.tail)
                .foregroundStyle(palette.textSecondary)
                .help(sentence)
                .accessibilityIdentifier("task-workflow-bar-live")
        }
        .font(Typography.text)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(graph.name): \(sentence)")
    }

    private var name: some View {
        Text(graph.name).font(Typography.mono).fixedSize()
            .foregroundStyle(palette.text)
            .accessibilityIdentifier("task-workflow-bar-name")
    }

    private var steps: some View {
        let tones = Self.tones(graph)
        return HStack(spacing: Spacing.sm) {
            ForEach(Array(graph.steps.enumerated()), id: \.offset) { index, step in
                HStack(spacing: 2) {
                    if step.interactive {
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
