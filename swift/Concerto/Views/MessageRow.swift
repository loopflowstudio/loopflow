#if os(macOS)
import SwiftUI
import LoopflowCore

struct MessageRow: View {
    @Environment(\.palette) private var palette
    let message: SessionMessage
    let onQueueEntry: (ReplyEntry) -> Void

    @State private var selectedQuote: String?
    @State private var replyDraft = ""
    @State private var selectionResetToken = 0

    var body: some View {
        if message.role == .system {
            Text(message.content)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
                .frame(maxWidth: .infinity, alignment: .center)
                .padding(.vertical, Spacing.xs)
        } else {
            HStack(alignment: .top, spacing: Spacing.sm) {
                accentBar

                VStack(alignment: .leading, spacing: Spacing.xs) {
                    content
                    Text(message.timestamp, style: .time)
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    @ViewBuilder
    private var content: some View {
        if message.role == .assistant {
            SelectableAssistantMessageTextView(
                text: message.content,
                selectionResetToken: selectionResetToken
            ) { newSelection in
                selectedQuote = newSelection
                if newSelection == nil {
                    replyDraft = ""
                }
            }
            .popover(
                isPresented: quotePopoverIsPresented,
                attachmentAnchor: .point(.top),
                arrowEdge: .top
            ) {
                if let selectedQuote {
                    ReplyComposerPopover(
                        quoted: selectedQuote,
                        replyDraft: $replyDraft,
                        onSubmitText: queueDraftReply,
                        onEmoji: queueEmojiReply
                    )
                }
            }
        } else {
            defaultMessageText
        }
    }

    @ViewBuilder
    private var defaultMessageText: some View {
        if message.role == .assistant,
           let markdown = try? AttributedString(
               markdown: message.content,
               options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
           ) {
            Text(markdown)
                .font(Typography.body())
                .foregroundStyle(palette.text)
                .textSelection(.enabled)
        } else {
            Text(message.content)
                .font(Typography.body())
                .foregroundStyle(message.role == .error ? .statusError : palette.text)
                .textSelection(.enabled)
        }
    }

    private var accentBar: some View {
        RoundedRectangle(cornerRadius: 1.5)
            .fill(accentColor)
            .frame(width: 3)
            .frame(minHeight: Spacing.lg)
            .accessibilityHidden(true)
    }

    private var accentColor: Color {
        switch message.role {
        case .user: return palette.accent
        case .assistant: return palette.textSecondary.opacity(0.4)
        case .error: return .statusError
        case .system: return .clear
        }
    }

    private var quotePopoverIsPresented: Binding<Bool> {
        Binding(
            get: { selectedQuote != nil },
            set: { isPresented in
                if !isPresented {
                    closeReplyComposer()
                }
            }
        )
    }

    private func queueDraftReply() {
        guard let selectedQuote else { return }
        let reply = replyDraft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !reply.isEmpty else { return }
        onQueueEntry(.quoteReply(quoted: selectedQuote, reply: reply))
        closeReplyComposer()
    }

    private func queueEmojiReply(_ emoji: String) {
        guard let selectedQuote else { return }
        onQueueEntry(.emojiReact(quoted: selectedQuote, emoji: emoji))
        closeReplyComposer()
    }

    private func closeReplyComposer() {
        guard selectedQuote != nil else { return }
        selectedQuote = nil
        replyDraft = ""
        selectionResetToken += 1
    }
}

#else
import SwiftUI
import LoopflowCore

struct MessageRow: View {
    @Environment(\.palette) private var palette
    let message: SessionMessage
    let onQueueEntry: (ReplyEntry) -> Void

    var body: some View {
        if message.role == .system {
            Text(message.content)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
                .frame(maxWidth: .infinity, alignment: .center)
                .padding(.vertical, Spacing.xs)
        } else {
            HStack(alignment: .top, spacing: Spacing.sm) {
                accentBar

                VStack(alignment: .leading, spacing: Spacing.xs) {
                    defaultMessageText
                    Text(message.timestamp, style: .time)
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    @ViewBuilder
    private var defaultMessageText: some View {
        if message.role == .assistant,
           let markdown = try? AttributedString(
               markdown: message.content,
               options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
           ) {
            Text(markdown)
                .font(Typography.body())
                .foregroundStyle(palette.text)
                .textSelection(.enabled)
        } else {
            Text(message.content)
                .font(Typography.body())
                .foregroundStyle(message.role == .error ? .statusError : palette.text)
                .textSelection(.enabled)
        }
    }

    private var accentBar: some View {
        RoundedRectangle(cornerRadius: 1.5)
            .fill(accentColor)
            .frame(width: 3)
            .frame(minHeight: Spacing.lg)
            .accessibilityHidden(true)
    }

    private var accentColor: Color {
        switch message.role {
        case .user: return palette.accent
        case .assistant: return palette.textSecondary.opacity(0.4)
        case .error: return .statusError
        case .system: return .clear
        }
    }
}

#endif
