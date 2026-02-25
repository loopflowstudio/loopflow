import SwiftUI
import LoopflowCore

struct WaveChatView: View {
    @Environment(\.palette) private var palette
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass

    @Bindable var state: ChatState

    @State private var composerText = ""
    @State private var expandedItemIds: Set<UUID> = []

    var body: some View {
        VStack(spacing: Spacing.md) {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: Spacing.md) {
                        ForEach(state.transcript) { entry in
                            transcriptRow(entry)
                                .id(entry.id)
                        }

                        if state.isLoading {
                            ProgressView(state.streamPhase == .replaying ? "Replaying…" : "Thinking…")
                                .font(Typography.caption())
                                .foregroundStyle(palette.textSecondary)
                                .padding(.top, Spacing.sm)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(Spacing.lg)
                }
                .background(palette.background)
                .onChange(of: state.transcript.count) { _, _ in
                    if let last = state.transcript.last {
                        withAnimation {
                            proxy.scrollTo(last.id, anchor: .bottom)
                        }
                    }
                }
            }

            if !state.suggestedActions.isEmpty {
                ActionButtonsView(actions: state.suggestedActions) { action in
                    composerText = ""
                    Task {
                        await state.sendSuggestedAction(action)
                    }
                }
                .padding(.horizontal, Spacing.lg)
            }

            HStack(alignment: .bottom, spacing: Spacing.sm) {
                TextField("Message", text: $composerText, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .lineLimit(1...6)
                    .disabled(!state.canSend)

                Button("End") {
                    Task {
                        await state.endSession()
                    }
                }
                .buttonStyle(DarkButtonStyle())
                .disabled(!state.canEndSession)

                Button("Send") {
                    sendMessage()
                }
                .keyboardShortcut(.return, modifiers: .command)
                .buttonStyle(DarkButtonStyle())
                .disabled(composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !state.canSend)
            }
            .padding(.horizontal, Spacing.lg)
            .padding(.bottom, Spacing.lg)
        }
        .background(palette.background)
        .task {
            state.configureClientContext(compact: horizontalSizeClass == .compact)
            await state.onAppear()
        }
        .onChange(of: horizontalSizeClass) { _, value in
            state.configureClientContext(compact: value == .compact)
        }
        .onChange(of: composerText) { _, value in
            state.composerTextDidChange(value)
        }
        .onDisappear {
            state.onDisappear()
        }
    }

    @ViewBuilder
    private func transcriptRow(_ entry: TranscriptEntry) -> some View {
        switch entry {
        case .message(let message):
            ChatBubble(message: message)

        case .item(let item):
            TranscriptItemCardView(
                item: item,
                isExpanded: expandedItemIds.contains(item.id),
                onToggleExpanded: {
                    if !expandedItemIds.insert(item.id).inserted {
                        expandedItemIds.remove(item.id)
                    }
                }
            )
        }
    }

    private func sendMessage() {
        let text = composerText
        composerText = ""
        Task {
            await state.send(text)
        }
    }
}

private struct ChatBubble: View {
    @Environment(\.palette) private var palette
    let message: ChatMessage

    var body: some View {
        if message.role == .system {
            Text(message.content)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
                .frame(maxWidth: .infinity, alignment: .center)
                .padding(.vertical, Spacing.xs)
        } else {
            HStack {
                if message.role == .user { Spacer(minLength: 48) }

                VStack(alignment: .leading, spacing: Spacing.xs) {
                    content
                    Text(message.timestamp, style: .time)
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                }
                .padding(Spacing.md)
                .background(backgroundColor)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                .frame(maxWidth: 620, alignment: .leading)

                if message.role != .user { Spacer(minLength: 48) }
            }
        }
    }

    @ViewBuilder
    private var content: some View {
        if message.role == .assistant,
           let markdown = try? AttributedString(
               markdown: message.content,
               options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
           ) {
            Text(markdown)
                .font(Typography.body())
                .foregroundStyle(palette.text)
        } else {
            Text(message.content)
                .font(Typography.body())
                .foregroundStyle(message.role == .error ? .statusError : palette.text)
        }
    }

    private var backgroundColor: Color {
        switch message.role {
        case .user:
            return palette.accent.opacity(0.12)
        case .assistant:
            return palette.surface
        case .error:
            return Color.statusError.opacity(0.12)
        case .system:
            return .clear
        }
    }
}

private struct TranscriptItemCardView: View {
    @Environment(\.palette) private var palette

    let item: TranscriptItem
    let isExpanded: Bool
    let onToggleExpanded: () -> Void

    private var card: TranscriptItemCard { item.card }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                Text(card.statusSymbol)
                    .font(Typography.body())
                    .foregroundStyle(card.statusColor)

                Text(card.typeIcon)
                    .font(Typography.body())

                Text(card.label)
                    .font(card.type == .thought ? Typography.caption() : Typography.body())
                    .foregroundStyle(card.type == .thought ? palette.textSecondary : palette.text)
                    .lineLimit(2)

                Spacer(minLength: 0)

                if card.detail != nil {
                    Button {
                        onToggleExpanded()
                    } label: {
                        Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(isExpanded ? "Collapse output" : "Expand output")
                }
            }

            if isExpanded, let detail = card.detail {
                Text(detail)
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .padding(Spacing.md)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        .frame(maxWidth: 700, alignment: .leading)
    }
}

private extension TranscriptItemCard {
    var typeIcon: String {
        switch type {
        case .command: return "⌘"
        case .file: return "📄"
        case .message: return "💬"
        case .thought: return "💭"
        case .tool: return "🔧"
        case .unknown: return "•"
        }
    }

    var statusSymbol: String {
        guard let status else { return "•" }
        switch status {
        case .inProgress: return "▶"
        case .completed: return "✓"
        case .failed: return "✗"
        case .declined: return "⊘"
        }
    }

    var statusColor: Color {
        guard let status else { return .statusNeutral }
        switch status {
        case .inProgress: return .statusInfo
        case .completed: return .statusSuccess
        case .failed: return .statusError
        case .declined: return .statusWarning
        }
    }
}
