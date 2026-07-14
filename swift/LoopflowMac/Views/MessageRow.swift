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
/// rebound from the old `SessionMessage`/transcript machinery to the live
/// `ChatTurn` wire model.
struct MessageRow: View {
    @Environment(\.palette) private var palette

    let turn: ChatTurn
    let timestampLabel: String?
    let attemptFailure: AttemptFailurePresentation?

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
            // Attributed emissions (`lf radio pub` — worker reports, child-wave
            // escalations) carry a speaker byline; plain turns don't.
            if let from = turn.from {
                Text(from)
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
            }

            if turn.role == .assistant {
                assistantBody
            } else {
                Text(turn.text)
                    .font(Typography.body())
                    .foregroundStyle(palette.text)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }

            if turn.status == .failed {
                failedBadge
            }

            if let timestampLabel {
                Text(timestampLabel)
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
            }
        }
    }

    @ViewBuilder
    private var assistantBody: some View {
        // The prose path is deliberately unmediated: `presentation.prose` goes
        // straight into the text view, so nothing sits between an arriving SSE
        // frame and the glyph.
        let presentation = self.presentation
        let segments = parseMessageSegments(presentation.prose)
        VStack(alignment: .leading, spacing: Spacing.sm) {
            ForEach(Array(segments.enumerated()), id: \.offset) { _, segment in
                switch segment {
                case .text(let text):
                    SelectableAssistantMessageTextView(text: text)
                case .code(let language, let code):
                    CodeBlockView(language: language, content: code)
                }
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

                Text("\(attemptFailure.flow) / \(attemptFailure.step)")
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
            }
        }
    }
}

#endif
