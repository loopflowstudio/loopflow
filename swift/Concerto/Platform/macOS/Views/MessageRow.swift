#if os(macOS)
import SwiftUI
import LoopflowCore

/// One turn in a WaveChat transcript. Recovered from the removed Conversations
/// subsystem and trimmed to what a read-only wave chat needs: role-styled prose
/// with a selectable assistant body. The reply composer, quote popovers, code-
/// block segmentation, and streaming cursor were dropped — they belong to the
/// live per-wave chat server that hasn't landed yet.
struct MessageRow: View {
    @Environment(\.palette) private var palette
    let message: SessionMessage
    let timestampLabel: String?

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
            SelectableAssistantMessageTextView(
                text: message.content,
                selectionResetToken: selectionResetToken
            ) { _ in }
        } else {
            Text(message.content)
                .font(Typography.body())
                .foregroundStyle(message.role == .error ? Color.statusError : palette.text)
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
        case .error: return Color.statusError
        case .system: return .clear
        }
    }
}

#endif
