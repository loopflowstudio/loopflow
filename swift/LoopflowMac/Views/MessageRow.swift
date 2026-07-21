#if os(macOS)
import SwiftUI
import Loopflow

/// One turn in a wave's conversation. A `user` turn is the operator's message
/// (accent bar + selectable prose). An `assistant` turn renders its accumulated
/// prose with fenced-code segmentation and a blinking cursor while it streams.
///
/// Tool calls, shell commands, and file edits never render here. Turn and child
/// failures have their own human-level presentation; the raw execution record
/// stays in the journal.
///
/// Recovered and rebuilt from the conversations `MessageRow` (git 45ab5d36e^),
/// rebound from the old transcript machinery to the live
/// `ChatTurn` wire model.
struct MessageRow: View {
    @Environment(\.palette) private var palette

    let turn: ChatTurn
    let timestampLabel: String?
    let attemptFailure: AttemptFailurePresentation?
    var source: ChatMessageSource?
    var references: ReferenceContext = .inert

    private var presentation: TurnPresentation {
        turnPresentation(turn)
    }

    var body: some View {
        if turn.role == .assistant {
            content
                .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            HStack(alignment: .top, spacing: Spacing.sm) {
                accentBar
                content
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            if turn.role == .assistant {
                assistantBody
            } else {
                ReferenceTextView(
                    text: turn.text,
                    textColor: palette.text,
                    references: references
                )
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            if turn.status == .failed {
                failedBadge
            }

            let showsTimestamp = attemptFailure?.count ?? 1 == 1 && timestampLabel != nil
            if showsTimestamp || source != nil {
                HStack(spacing: Spacing.xs) {
                    if showsTimestamp, let timestampLabel {
                        Text(timestampLabel)
                    }
                    if showsTimestamp, source != nil {
                        Text("·")
                    }
                    sourceLabel
                }
                .font(Typography.caption(11))
                .foregroundStyle(palette.textSecondary)
            }
        }
    }

    @ViewBuilder
    private var sourceLabel: some View {
        switch source {
        case .local:
            Label("Local", systemImage: "macbook")
        case let .discord(_, _, _, _, url):
            if let destination = URL(string: url) {
                Link(destination: destination) {
                    Label("Discord", systemImage: "arrow.up.right")
                }
                .foregroundStyle(palette.accent)
                .accessibilityLabel("Open original Discord message")
            } else {
                Label("Discord", systemImage: "bubble.left.and.bubble.right")
            }
        case nil:
            EmptyView()
        }
    }

    @ViewBuilder
    private var assistantBody: some View {
        // The prose path is deliberately unmediated: `presentation.prose` goes
        // straight into the text view, so nothing sits between an arriving SSE
        // frame and the glyph.
        let presentation = self.presentation
        let segments = parseMessageSegments(presentation.conclusion)
        VStack(alignment: .leading, spacing: Spacing.sm) {
            ForEach(Array(segments.enumerated()), id: \.offset) { _, segment in
                switch segment {
                case .text(let text):
                    ReferenceTextView(
                        text: text,
                        textColor: palette.text,
                        references: references
                    )
                case .code(let language, let code):
                    CodeBlockView(language: language, content: code)
                }
            }

            if presentation.hasSteps {
                stepsDisclosure(presentation.steps)
            }

            if !presentation.hasProse, turn.status == .running {
                HStack(spacing: Spacing.sm) {
                    ProgressView()
                        .controlSize(.small)
                    Text("Working…")
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                }
                .accessibilityIdentifier("wave-chat-working")
            }

            if turn.status == .running {
                StreamingCursorView()
                    .id("streaming-cursor-\(turn.id)")
            }
        }
    }

    /// Operational narration the harness tagged `commentary`, folded away so the
    /// wave's decision reads first. The raw record is untouched; this only curates
    /// what the eye meets. References inside a step stay live.
    private func stepsDisclosure(_ steps: [String]) -> some View {
        DisclosureGroup(steps.count == 1 ? "Show 1 step" : "Show \(steps.count) steps") {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                ForEach(Array(steps.enumerated()), id: \.offset) { _, step in
                    ReferenceTextView(
                        text: step,
                        textColor: palette.textSecondary,
                        references: references
                    )
                }
            }
            .padding(.top, Spacing.xs)
        }
        .font(Typography.caption(11))
        .foregroundStyle(palette.textSecondary)
        .accessibilityIdentifier("wave-chat-steps")
    }

    private var accentBar: some View {
        RoundedRectangle(cornerRadius: 1.5)
            .fill(palette.accent)
            .frame(width: 3)
            .frame(minHeight: Spacing.lg)
            .accessibilityHidden(true)
    }

    private var failedBadge: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            HStack(spacing: Spacing.xs) {
                Image(systemName: "exclamationmark.triangle.fill")
                Text(attemptFailure?.title ?? "Turn failed")
            }
            .font(Typography.caption(11))
            .foregroundStyle(Color.statusError)

            if let attemptFailure {
                Text(attemptFailure.reason)
                    .font(Typography.caption())
                    .foregroundStyle(palette.text)
                    .textSelection(.enabled)

                HStack(spacing: Spacing.xs) {
                    Text("\(attemptFailure.flow) / \(attemptFailure.step)")
                    if attemptFailure.count > 1, let timestampLabel {
                        Text("· latest \(timestampLabel)")
                    }
                }
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)

                if attemptFailure.count > 1 {
                    DisclosureGroup("Details") {
                        VStack(alignment: .leading, spacing: Spacing.sm) {
                            ForEach(attemptFailure.attempts) { attempt in
                                VStack(alignment: .leading, spacing: Spacing.xxs) {
                                    Text("\(attempt.turnId) · \(attempt.createdAt)")
                                        .font(Typography.caption(10).monospaced())
                                        .foregroundStyle(palette.textSecondary)
                                    Text(attempt.reason)
                                        .font(Typography.caption(11))
                                        .foregroundStyle(palette.text)
                                        .textSelection(.enabled)
                                }
                            }
                        }
                        .padding(.top, Spacing.xs)
                    }
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
                }
            }
        }
    }
}

#endif
