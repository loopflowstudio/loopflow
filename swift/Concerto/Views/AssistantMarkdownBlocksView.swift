import SwiftUI
import LoopflowCore

struct AssistantMarkdownBlocksView: View {
    @Environment(\.palette) private var palette

    let blocks: [MarkdownBlock]
    let selectionResetToken: Int
    let onSelectionChanged: (String?) -> Void
    let onQuoteReply: (String) -> Void
    let onEmojiReact: (String, String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in
                blockView(block)
            }
        }
    }

    @ViewBuilder
    private func blockView(_ block: MarkdownBlock) -> some View {
        switch block {
        case .paragraph(let text):
            selectableInlineText(text)

        case .heading(let level, let text):
            Text(text)
                .font(headingFont(level: level))
                .foregroundStyle(palette.accent)
                .textSelection(.enabled)
                .accessibilityAddTraits(.isHeader)

        case .list(let ordered, let items):
            VStack(alignment: .leading, spacing: Spacing.xs) {
                ForEach(Array(items.enumerated()), id: \.offset) { index, item in
                    HStack(alignment: .top, spacing: Spacing.sm) {
                        Text(ordered ? "\(index + 1)." : "•")
                            .font(Typography.body())
                            .foregroundStyle(palette.textSecondary)
                            .frame(minWidth: ordered ? 24 : 12, alignment: .trailing)
                        Text(item)
                            .font(Typography.body())
                            .foregroundStyle(palette.text)
                            .textSelection(.enabled)
                    }
                }
            }

        case .blockquote(let quotedBlocks):
            HStack(alignment: .top, spacing: Spacing.sm) {
                RoundedRectangle(cornerRadius: CornerRadius.full)
                    .fill(palette.accent.opacity(0.45))
                    .frame(width: 3)
                    .accessibilityHidden(true)

                AssistantMarkdownBlocksView(
                    blocks: quotedBlocks,
                    selectionResetToken: selectionResetToken,
                    onSelectionChanged: onSelectionChanged,
                    onQuoteReply: onQuoteReply,
                    onEmojiReact: onEmojiReact
                )
            }
            .padding(.vertical, Spacing.xs)
            .padding(.leading, Spacing.xs)

        case .code(let language, let content):
            CodeBlockView(language: language, content: content)

        case .diff(let diff):
            DiffLinesView(diff: diff)
                .padding(Spacing.md)
                .background(palette.surface)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                .overlay(
                    RoundedRectangle(cornerRadius: CornerRadius.md)
                        .stroke(palette.border, lineWidth: 1)
                )

        case .rule:
            Divider()
                .overlay(palette.border)
                .padding(.vertical, Spacing.xs)
        }
    }

    private func headingFont(level: Int) -> Font {
        switch level {
        case 1: return Typography.sectionTitle(24)
        case 2: return Typography.sectionTitle(20)
        case 3: return Typography.sectionTitle(18)
        default: return Typography.body(15).weight(.bold)
        }
    }

    @ViewBuilder
    private func selectableInlineText(_ text: AttributedString) -> some View {
        #if os(macOS)
        SelectableAssistantMessageTextView(
            text: text,
            selectionResetToken: selectionResetToken,
            onSelectionChanged: onSelectionChanged
        )
        #else
        SelectableAssistantTextView(attributedText: text) { action in
            switch action {
            case .quoteReply(let selected):
                onQuoteReply(selected)
            case .emojiReact(let selected, let emoji):
                onEmojiReact(selected, emoji)
            }
        }
        #endif
    }
}
