import SwiftUI
import LoopflowCore

struct WaveSessionView: View {
    @Environment(\.palette) private var palette
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass

    @Bindable var state: SessionState
    let showsSuggestedActions: Bool
    let managesLifecycle: Bool

    @State private var composerText = ""
    @State private var expandedItemIds: Set<UUID> = []
    @State private var expandedRunIds: Set<UUID> = []
    @State private var isNearBottom = true
    @State private var replyQueue = ReplyQueue()
    @State private var isReplyTrayExpanded = false
    @State private var replyTrayFreeText = ""

    var body: some View {
        VStack(spacing: Spacing.md) {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: Spacing.md) {
                        ForEach(groupedTranscript) { group in
                            transcriptGroupRow(group)
                                .id(group.id)
                        }

                        if state.isLoading {
                            ProgressView(state.streamPhase == .replaying ? "Replaying…" : "Thinking…")
                                .font(Typography.caption())
                                .foregroundStyle(palette.textSecondary)
                                .padding(.top, Spacing.sm)
                        }

                        Color.clear
                            .frame(height: 1)
                            .id("transcript-bottom")
                            .onAppear { isNearBottom = true }
                            .onDisappear { isNearBottom = false }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(Spacing.lg)
                }
                .background(palette.background)
                .onChange(of: state.transcript.count) { _, _ in
                    guard isNearBottom, let last = state.transcript.last else { return }
                    withAnimation {
                        proxy.scrollTo(last.id, anchor: .bottom)
                    }
                }
            }

            if showsSuggestedActions, !state.suggestedActions.isEmpty {
                ActionButtonsView(actions: state.suggestedActions) { action in
                    composerText = ""
                    replyQueue.clear()
                    replyTrayFreeText = ""
                    Task {
                        await state.sendSuggestedAction(action)
                    }
                }
                .padding(.horizontal, Spacing.lg)
            }

            ReplyDraftTray(
                queue: replyQueue,
                isExpanded: $isReplyTrayExpanded,
                freeTextDraft: $replyTrayFreeText,
                onSend: sendMessage
            )
            .padding(.horizontal, Spacing.lg)

            HStack(alignment: .bottom, spacing: Spacing.sm) {
                TextField("Message", text: $composerText, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .lineLimit(1...6)

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
                .disabled(!canSendMessage || !state.canSend)
            }
            .padding(.horizontal, Spacing.lg)
            .padding(.bottom, Spacing.lg)
        }
        .background(palette.background)
        .task {
            guard managesLifecycle else { return }
            state.configureClientContext(compact: horizontalSizeClass == .compact)
            await state.onAppear()
        }
        .onChange(of: horizontalSizeClass) { _, value in
            guard managesLifecycle else { return }
            state.configureClientContext(compact: value == .compact)
        }
        .onDisappear {
            guard managesLifecycle else { return }
            state.onDisappear()
        }
    }

    init(
        state: SessionState,
        showsSuggestedActions: Bool = true,
        managesLifecycle: Bool = true
    ) {
        self.state = state
        self.showsSuggestedActions = showsSuggestedActions
        self.managesLifecycle = managesLifecycle
    }

    private var groupedTranscript: [TranscriptGroup] {
        var groups: [TranscriptGroup] = []
        var currentRun: [TranscriptItem] = []

        func flushRun() {
            guard !currentRun.isEmpty else { return }
            if currentRun.count == 1 {
                groups.append(.single(.item(currentRun[0])))
            } else {
                groups.append(.toolRun(currentRun))
            }
            currentRun = []
        }

        for entry in state.transcript {
            switch entry {
            case .item(let item) where item.card.type.isToolLike:
                currentRun.append(item)
            default:
                flushRun()
                groups.append(.single(entry))
            }
        }
        flushRun()
        return groups
    }

    @ViewBuilder
    private func transcriptGroupRow(_ group: TranscriptGroup) -> some View {
        switch group {
        case .single(let entry):
            transcriptRow(entry)
        case .toolRun(let items):
            ToolRunView(
                items: items,
                isExpanded: expandedRunIds.contains(group.id),
                expandedItemIds: $expandedItemIds,
                onToggleRun: {
                    if !expandedRunIds.insert(group.id).inserted {
                        expandedRunIds.remove(group.id)
                    }
                }
            )
        }
    }

    @ViewBuilder
    private func transcriptRow(_ entry: TranscriptEntry) -> some View {
        switch entry {
        case .message(let message):
            MessageRow(message: message) { newEntry in
                replyQueue.entries.append(newEntry)
                isReplyTrayExpanded = true
            }

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
        let text = replyQueue.assembleMessage(extraFreeText: composerText)
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        composerText = ""
        replyQueue.clear()
        replyTrayFreeText = ""
        isReplyTrayExpanded = false
        Task {
            await state.send(text)
        }
    }

    private var canSendMessage: Bool {
        !composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !replyQueue.isEmpty
    }
}

private struct MessageRow: View {
    @Environment(\.palette) private var palette
    let message: SessionMessage
    var onQueueEntry: ((ReplyEntry) -> Void)? = nil

#if os(macOS)
    @State private var selectedQuote: String?
    @State private var replyDraft = ""
    @State private var selectionResetToken = 0
#endif

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
#if os(macOS)
        if message.role == .assistant, onQueueEntry != nil {
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
        } else if message.role == .assistant,
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
#else
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
#endif
    }

    private var accentBar: some View {
        RoundedRectangle(cornerRadius: 1.5)
            .fill(accentColor)
            .frame(width: 3)
            .frame(minHeight: Spacing.lg)
    }

    private var accentColor: Color {
        switch message.role {
        case .user: return palette.accent
        case .assistant: return palette.textSecondary.opacity(0.4)
        case .error: return .statusError
        case .system: return .clear
        }
    }

#if os(macOS)
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
        onQueueEntry?(.quoteReply(quoted: selectedQuote, reply: reply))
        closeReplyComposer()
    }

    private func queueEmojiReply(_ emoji: String) {
        guard let selectedQuote else { return }
        onQueueEntry?(.emojiReact(quoted: selectedQuote, emoji: emoji))
        closeReplyComposer()
    }

    private func closeReplyComposer() {
        selectedQuote = nil
        replyDraft = ""
        selectionResetToken += 1
    }
#endif
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
    }
}

private enum TranscriptGroup: Identifiable {
    case single(TranscriptEntry)
    case toolRun([TranscriptItem])

    var id: UUID {
        switch self {
        case .single(let entry): return entry.id
        case .toolRun(let items): return items[0].id
        }
    }
}

private struct ToolRunView: View {
    @Environment(\.palette) private var palette

    let items: [TranscriptItem]
    let isExpanded: Bool
    @Binding var expandedItemIds: Set<UUID>
    let onToggleRun: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Button {
                onToggleRun()
            } label: {
                HStack(spacing: Spacing.sm) {
                    Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)

                    Text("\(items.count) tool calls")
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)

                    Spacer()
                }
                .padding(.vertical, Spacing.xs)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(isExpanded ? "Collapse tool calls" : "Expand \(items.count) tool calls")

            if isExpanded {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    ForEach(items) { item in
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
            }
        }
    }
}

extension SessionItemType {
    var isToolLike: Bool {
        switch self {
        case .command, .tool, .file: return true
        case .message, .thought, .unknown: return false
        }
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
