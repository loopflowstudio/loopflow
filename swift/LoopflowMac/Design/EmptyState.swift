import Loopflow
import SwiftUI

extension EnvironmentValues {
    /// Dashboard slots: an empty state reads as one line sized to the slot.
    @Entry var compactPanes = false
}

/// A pane with nothing to show, or one whose read failed: what happened in a
/// serif title, one sentence, then buttons for what to do next. `machine` is
/// the Machine's message as it came; a state that has one is a failure.
/// At most one action is `.accent`; the rest take the neutral button.
/// In a dashboard slot it is one line: the title and the buttons, the buttons
/// alone when both do not fit, or the title cut short when there are none.
struct EmptyState<Actions: View>: View {
    let title: String
    let sentence: String?
    let machine: String?
    @ViewBuilder let actions: Actions
    @Environment(\.compactPanes) private var compact
    @Environment(\.palette) private var palette

    /// About 52 characters of `Typography.text`.
    private static var measure: CGFloat { 400 }

    init(title: String, sentence: String? = nil, machine: String? = nil, @ViewBuilder actions: () -> Actions) {
        self.title = title
        self.sentence = sentence
        self.machine = machine
        self.actions = actions()
    }

    var body: some View {
        if compact {
            Group {
                if Actions.self == EmptyView.self {
                    // Nothing to press: the title stays, cut short where the slot ends.
                    Text(title).foregroundStyle(palette.text).lineLimit(1).truncationMode(.tail)
                } else {
                    // A narrow slot keeps the action rather than a clipped title.
                    ViewThatFits(in: .horizontal) {
                        HStack(spacing: Spacing.sm) {
                            Text(title).foregroundStyle(palette.text).fixedSize()
                            buttons.fixedSize()
                        }
                        buttons.fixedSize()
                    }
                }
            }
            .font(Typography.text)
            .help(machine ?? sentence ?? title)
            .padding(Spacing.sm)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                Text(title).serif(.title).foregroundStyle(palette.emphasisInk)
                    .fixedSize(horizontal: false, vertical: true)
                if let sentence {
                    Text(sentence).font(Typography.text).foregroundStyle(palette.text).lineSpacing(Spacing.xs)
                        .frame(maxWidth: Self.measure, alignment: .leading)
                        .fixedSize(horizontal: false, vertical: true)
                }
                if let machine {
                    Text(machine).font(Typography.mono).foregroundStyle(palette.textSecondary)
                        .textSelection(.enabled)
                        .frame(maxWidth: Self.measure, alignment: .leading)
                        .fixedSize(horizontal: false, vertical: true)
                }
                buttons.padding(.top, Spacing.sm)
            }
            .padding(Spacing.xxxl)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        }
    }

    private var buttons: some View {
        HStack(spacing: Spacing.sm) { actions }.buttonStyle(LoopflowButtonStyle())
    }
}

extension EmptyState where Actions == EmptyView {
    init(title: String, sentence: String? = nil, machine: String? = nil) {
        self.init(title: title, sentence: sentence, machine: machine) { EmptyView() }
    }
}

extension View {
    /// A list or section with nothing in it: one sentence in the secondary ink.
    /// The serif title belongs to the pane's `EmptyState`, one to a screen.
    func emptyLine() -> some View { modifier(EmptyLine()) }
}

private struct EmptyLine: ViewModifier {
    @Environment(\.palette) private var palette

    func body(content: Content) -> some View {
        content.font(Typography.text).foregroundStyle(palette.textSecondary)
    }
}

/// A failure beside content that stays on screen: what failed in the reading
/// face, the Machine's message in mono and selectable, then the way to try again.
struct FailureLine<Actions: View>: View {
    let what: String
    let machine: String
    @ViewBuilder let actions: Actions
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
            Text(what).font(Typography.text).foregroundStyle(palette.failure)
            Text(machine).font(Typography.mono).foregroundStyle(palette.textSecondary)
                .textSelection(.enabled).fixedSize(horizontal: false, vertical: true)
            actions
        }
    }
}

extension FailureLine where Actions == EmptyView {
    init(_ what: String, machine: String) {
        self.init(what: what, machine: machine) { EmptyView() }
    }
}
