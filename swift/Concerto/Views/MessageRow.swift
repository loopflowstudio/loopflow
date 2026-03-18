#if os(macOS)
import SwiftUI
import LoopflowCore

struct MessageRow: View {
    @Environment(\.palette) private var palette
    let message: SessionMessage
    let timestampLabel: String?
    let showStreamingCursor: Bool
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
        } else if message.role == .assistant {
            messageContent
                .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            HStack(alignment: .top, spacing: Spacing.sm) {
                accentBar
                messageContent
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var messageContent: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            content

            if let timestampLabel {
                Text(timestampLabel)
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
            }
        }
    }

    @ViewBuilder
    private var content: some View {
        if message.role == .assistant {
            let segments = parseMessageSegments(message.content)
            VStack(alignment: .leading, spacing: Spacing.sm) {
                ForEach(Array(segments.enumerated()), id: \.offset) { _, segment in
                    switch segment {
                    case .text(let text):
                        SelectableAssistantMessageTextView(
                            text: text,
                            selectionResetToken: selectionResetToken
                        ) { newSelection in
                            selectedQuote = newSelection
                            if newSelection == nil {
                                replyDraft = ""
                            }
                        }
                    case .code(let language, let codeContent):
                        CodeBlockView(language: language, content: codeContent)
                    }
                }

                if showStreamingCursor {
                    StreamingCursorView()
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
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    let message: SessionMessage
    let timestampLabel: String?
    let showStreamingCursor: Bool
    let onQueueEntry: (ReplyEntry) -> Void

    @State private var composerQuote: String?
    @State private var replyDraft = ""

    var body: some View {
        if message.role == .system {
            Text(message.content)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
                .frame(maxWidth: .infinity, alignment: .center)
                .padding(.vertical, Spacing.xs)
        } else if message.role == .assistant {
            messageContent
                .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            HStack(alignment: .top, spacing: Spacing.sm) {
                accentBar
                messageContent
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var messageContent: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            content

            if let timestampLabel {
                Text(timestampLabel)
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
            }
        }
    }

    @ViewBuilder
    private var content: some View {
        if message.role == .assistant {
            let segments = parseMessageSegments(message.content)
            VStack(alignment: .leading, spacing: Spacing.sm) {
                ForEach(Array(segments.enumerated()), id: \.offset) { _, segment in
                    switch segment {
                    case .text(let text):
                        SelectableAssistantTextView(text: text) { action in
                            handleQuoteAction(action)
                        }
                    case .code(let language, let codeContent):
                        CodeBlockView(language: language, content: codeContent)
                    }
                }

                if showStreamingCursor {
                    StreamingCursorView()
                }
            }
            .modifier(ReplyComposerPresentation(
                composerQuote: $composerQuote,
                replyDraft: $replyDraft,
                isCompact: horizontalSizeClass == .compact,
                onSubmitText: queueDraftReply,
                onEmoji: queueEmojiReply
            ))
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

    private func handleQuoteAction(_ action: QuoteAction) {
        switch action {
        case .quoteReply(let selected):
            composerQuote = selected
        case .emojiReact(let selected, let emoji):
            onQueueEntry(.emojiReact(quoted: selected, emoji: emoji))
        }
    }

    private func queueDraftReply() {
        guard let quoted = composerQuote else { return }
        let reply = replyDraft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !reply.isEmpty else { return }
        onQueueEntry(.quoteReply(quoted: quoted, reply: reply))
        closeComposer()
    }

    private func queueEmojiReply(_ emoji: String) {
        guard let quoted = composerQuote else { return }
        onQueueEntry(.emojiReact(quoted: quoted, emoji: emoji))
        closeComposer()
    }

    private func closeComposer() {
        composerQuote = nil
        replyDraft = ""
    }
}

private struct ReplyComposerPresentation: ViewModifier {
    @Binding var composerQuote: String?
    @Binding var replyDraft: String
    let isCompact: Bool
    let onSubmitText: () -> Void
    let onEmoji: (String) -> Void

    private var isPresented: Binding<Bool> {
        Binding(
            get: { composerQuote != nil },
            set: { if !$0 { composerQuote = nil; replyDraft = "" } }
        )
    }

    func body(content: Content) -> some View {
        if isCompact {
            content.sheet(isPresented: isPresented) {
                if let quoted = composerQuote {
                    composer(for: quoted)
                    .presentationDetents([.medium])
                }
            }
        } else {
            content.popover(isPresented: isPresented, arrowEdge: .top) {
                if let quoted = composerQuote {
                    composer(for: quoted)
                    .frame(width: 320)
                }
            }
        }
    }

    private func composer(for quoted: String) -> ReplyComposerContent {
        ReplyComposerContent(
            quoted: quoted,
            replyDraft: $replyDraft,
            onSubmitText: onSubmitText,
            onEmoji: onEmoji
        )
    }
}

#endif
