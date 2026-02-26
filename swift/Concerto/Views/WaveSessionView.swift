import SwiftUI
import LoopflowCore

struct WaveSessionView: View {
    @Environment(\.palette) private var palette
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass

    @Bindable var state: SessionState
    let showsSuggestedActions: Bool
    let managesLifecycle: Bool

    @State private var composerText = ""
    @State private var voiceService = VoiceInputService()
    @State private var expandedItemIds: Set<UUID> = []
    @State private var expandedRunIds: Set<UUID> = []
    @State private var isNearBottom = true
    @State private var replyQueue = ReplyQueue()
    @State private var isReplyTrayExpanded = false
    @State private var voiceComposerBaseline: String?
    @FocusState private var focusedField: FocusField?

    private enum FocusField: Hashable {
        case transcript
        case composer
    }

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
                            SessionThinkingIndicator(
                                turnState: state.turnState,
                                streamPhase: state.streamPhase,
                                awaitingSessionJoin: state.awaitingSessionJoin
                            )
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
                .overlay {
                    if state.transcript.isEmpty && !state.isLoading {
                        SessionEmptyStateView()
                    }
                }
                .macOSFocusable()
                .focused($focusedField, equals: .transcript)
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
                    clearReplyInputs()
                    Task {
                        await state.sendSuggestedAction(action)
                    }
                }
                .padding(.horizontal, Spacing.lg)
            }

            if !replyQueue.isEmpty {
                ReplyDraftTray(
                    queue: replyQueue,
                    isExpanded: $isReplyTrayExpanded
                )
                .padding(.horizontal, Spacing.lg)
            }

            VStack(alignment: .leading, spacing: Spacing.xs) {
                HStack(alignment: .bottom, spacing: Spacing.sm) {
                    VoiceInputButton(voiceService: voiceService) { transcript in
                        insertTranscriptIntoComposer(transcript)
                    }

                    TextField("Message", text: $composerText, axis: .vertical)
                        .textFieldStyle(.roundedBorder)
                        .lineLimit(1...6)
                        .focused($focusedField, equals: .composer)
                        .macOSOnExitCommand {
                            focusedField = .transcript
                        }

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

                voiceFeedback
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
        .onReceive(NotificationCenter.default.publisher(for: .focusSessionComposer)) { notification in
            if let waveId = notification.userInfo?["waveId"] as? String, waveId != state.waveId {
                return
            }
            focusedField = .composer
        }
        .onChange(of: horizontalSizeClass) { _, value in
            guard managesLifecycle else { return }
            state.configureClientContext(compact: value == .compact)
        }
        .onChange(of: voiceService.state) { _, newState in
            handleVoiceStateChange(newState)
        }
        .onChange(of: voiceService.partialTranscript) { _, partial in
            syncVoicePartialIntoComposer(partial)
        }
        .onDisappear {
            voiceService.cancel()
            voiceComposerBaseline = nil
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

    private var latestAssistantMessageId: UUID? {
        state.transcript.reversed().compactMap { entry -> UUID? in
            guard case .message(let message) = entry, message.role == .assistant else {
                return nil
            }
            return message.id
        }.first
    }

    private var timestampLabelByMessageId: [UUID: String] {
        messageTimestampLabels(for: state.transcript)
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
                    toggleMembership(group.id, in: &expandedRunIds)
                }
            )
        }
    }

    @ViewBuilder
    private func transcriptRow(_ entry: TranscriptEntry) -> some View {
        switch entry {
        case .message(let message):
            let timestamp = timestampLabelByMessageId[message.id]
            MessageRow(
                message: message,
                timestampLabel: timestamp,
                showStreamingCursor: state.turnState == .running && latestAssistantMessageId == message.id
            ) { newEntry in
                replyQueue.add(newEntry)
                isReplyTrayExpanded = true
            }

        case .item(let item):
            TranscriptItemCardView(
                item: item,
                isExpanded: expandedItemIds.contains(item.id),
                onToggleExpanded: {
                    toggleMembership(item.id, in: &expandedItemIds)
                }
            )
        }
    }

    private func sendMessage() {
        guard state.canSend else { return }
        let text = replyQueue.assembleMessage(extraFreeText: composerText)
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        composerText = ""
        clearReplyInputs(collapseTray: true)
        Task {
            await state.send(text)
        }
    }

    private var canSendMessage: Bool {
        !composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !replyQueue.isEmpty
    }

    private func clearReplyInputs(collapseTray: Bool = false) {
        replyQueue.clear()
        if collapseTray {
            isReplyTrayExpanded = false
        }
    }

    private func insertTranscriptIntoComposer(_ rawTranscript: String) {
        let transcript = rawTranscript.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !transcript.isEmpty else { return }

        beginVoiceComposerSessionIfNeeded()
        composerText = mergedComposerText(base: voiceComposerBaseline ?? composerText, transcript: transcript)
        voiceComposerBaseline = nil

        focusedField = .composer
    }

    private func handleVoiceStateChange(_ newState: VoiceInputService.State) {
        switch newState {
        case .recording:
            beginVoiceComposerSessionIfNeeded()
        case .transcribing:
            break
        case .idle:
            voiceComposerBaseline = nil
        }
    }

    private func syncVoicePartialIntoComposer(_ rawPartial: String) {
        guard voiceService.state == .recording || voiceService.state == .transcribing else { return }

        let partial = rawPartial.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !partial.isEmpty else { return }

        beginVoiceComposerSessionIfNeeded()
        composerText = mergedComposerText(base: voiceComposerBaseline ?? composerText, transcript: partial)
    }

    private func beginVoiceComposerSessionIfNeeded() {
        guard voiceComposerBaseline == nil else { return }
        voiceComposerBaseline = composerText
    }

    private func mergedComposerText(base: String, transcript: String) -> String {
        let cleanBase = base.trimmingCharacters(in: .whitespacesAndNewlines)
        if cleanBase.isEmpty {
            return transcript
        }
        let needsSpacer = base.last?.isWhitespace == false
        return base + (needsSpacer ? " \(transcript)" : transcript)
    }

    @ViewBuilder
    private var voiceFeedback: some View {
        if voiceService.permissionStatus == .denied {
            VoicePermissionNotice()
        } else if voiceService.state == .transcribing,
                  let statusText = voiceService.modelStatusText {
            VoiceStatusRow(
                statusText: statusText,
                progress: voiceService.modelDownloadProgress
            )
        } else if let errorMessage = voiceService.errorMessage {
            HStack(spacing: Spacing.xs) {
                Image(systemName: "exclamationmark.triangle")
                    .font(Typography.caption())
                    .foregroundStyle(Color.statusError)
                Text(errorMessage)
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
        }
    }
}

private struct VoiceStatusRow: View {
    @Environment(\.palette) private var palette

    let statusText: String
    let progress: Double?

    var body: some View {
        HStack(spacing: Spacing.sm) {
            if let progress {
                ProgressView(value: progress)
                    .progressViewStyle(.linear)
                    .frame(width: 120)
            } else {
                ProgressView()
                    .controlSize(.small)
            }

            Text(statusText)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
        }
        .accessibilityElement(children: .combine)
    }
}

private struct VoicePermissionNotice: View {
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.xs) {
            Image(systemName: "mic.slash")
                .font(Typography.caption())
                .foregroundStyle(Color.statusError)
            Text("Microphone access needed.")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            Button("Open Settings") {
                openMicrophoneSettings()
            }
            .font(Typography.caption())
            .buttonStyle(.plain)
            .foregroundStyle(palette.accent)
            .minHitTarget()
            .accessibilityLabel("Open microphone settings")
        }
    }
}

private struct SessionThinkingIndicator: View {
    @Environment(\.palette) private var palette
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    let turnState: TurnState
    let streamPhase: StreamPhase
    let awaitingSessionJoin: Bool

    @State private var isPulsing = false

    var body: some View {
        if turnState == .running {
            HStack(spacing: Spacing.sm) {
                RoundedRectangle(cornerRadius: 1.5)
                    .fill(palette.accent)
                    .frame(width: 3, height: 14)
                    .opacity(reduceMotion ? 1 : (isPulsing ? 1 : 0.3))
                    .onAppear {
                        guard !reduceMotion else { return }
                        isPulsing = false
                        withAnimation(.easeInOut(duration: 1.2).repeatForever(autoreverses: true)) {
                            isPulsing = true
                        }
                    }

                Text("Thinking…")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)

                Spacer(minLength: 0)
            }
            .accessibilityLabel("Thinking")
        } else if streamPhase == .replaying {
            statusLabel("Replaying…", accessibilityLabel: "Replaying")
        } else if awaitingSessionJoin {
            statusLabel("Waiting for session…", accessibilityLabel: "Waiting for session")
        } else if streamPhase == .ending {
            statusLabel("Ending session…", accessibilityLabel: "Ending session")
        }
    }

    private func statusLabel(_ text: String, accessibilityLabel: String) -> some View {
        Text(text)
            .font(Typography.caption())
            .foregroundStyle(palette.textSecondary)
            .accessibilityLabel(accessibilityLabel)
    }
}

private struct SessionEmptyStateView: View {
    @Environment(\.palette) private var palette

    var body: some View {
        VStack {
            Text("What would you like to work on?")
                .font(.custom(Typography.serifFamily, size: 28).italic())
                .foregroundStyle(palette.textSecondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
        .padding(Spacing.xl)
        .allowsHitTesting(false)
    }
}

struct AssistantTextSegment: View {
    @Environment(\.palette) private var palette

    let text: String

    var body: some View {
        if let markdown = try? AttributedString(
            markdown: text,
            options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
        ) {
            Text(markdown)
                .font(Typography.body())
                .foregroundStyle(palette.text)
        } else {
            Text(text)
                .font(Typography.body())
                .foregroundStyle(palette.text)
        }
    }
}

struct StreamingCursorView: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    @State private var isVisible = false

    var body: some View {
        Text("▊")
            .font(Typography.code(13))
            .foregroundStyle(Color.statusInfo)
            .opacity(reduceMotion ? 1 : (isVisible ? 1 : 0))
            .onAppear {
                guard !reduceMotion else {
                    isVisible = true
                    return
                }
                isVisible = false
                withAnimation(.easeInOut(duration: 0.6).repeatForever(autoreverses: true)) {
                    isVisible = true
                }
            }
            .accessibilityLabel("Streaming")
    }
}

struct CodeBlockView: View {
    @Environment(\.palette) private var palette
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass

    let language: String?
    let content: String

    @State private var isHovering = false

    private var showCopyButton: Bool {
        horizontalSizeClass == .compact || isHovering
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.sm) {
                if let language, !language.isEmpty {
                    Text(language.uppercased())
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                        .padding(.horizontal, Spacing.sm)
                        .padding(.vertical, Spacing.xxs)
                        .background(palette.surfaceMuted)
                        .clipShape(Capsule())
                        .accessibilityLabel("Code language \(language)")
                }

                Spacer(minLength: 0)

                CopyButton(content: content, isVisible: showCopyButton)
            }

            ScrollView(.horizontal) {
                Text(content)
                    .font(Typography.code(13))
                    .foregroundStyle(palette.text)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .fixedSize(horizontal: true, vertical: false)
            }
        }
        .padding(Spacing.md)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        .overlay(
            RoundedRectangle(cornerRadius: CornerRadius.md)
                .stroke(palette.border, lineWidth: 1)
        )
        .hoverTracking { hovering in
            isHovering = hovering
        }
    }
}

private struct CopyButton: View {
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    let content: String
    let isVisible: Bool

    @State private var didCopy = false

    private var minimumHitTarget: CGFloat {
        horizontalSizeClass == .compact ? HitTarget.touch : HitTarget.minimum
    }

    private var isInteractive: Bool {
        isVisible || didCopy
    }

    var body: some View {
        Button {
            copyToClipboard(content)
            didCopy = true

            Task {
                try? await Task.sleep(for: .milliseconds(1500))
                withAnimation(DesignAnimation.standard(reduceMotion)) {
                    didCopy = false
                }
            }
        } label: {
            Image(systemName: didCopy ? "checkmark" : "doc.on.doc")
                .font(Typography.caption())
                .foregroundStyle(didCopy ? Color.statusSuccess : Color.primary.opacity(0.75))
                .frame(minWidth: minimumHitTarget, minHeight: minimumHitTarget)
        }
        .buttonStyle(.plain)
        .opacity(isInteractive ? 1 : 0)
        .allowsHitTesting(isInteractive)
        .animation(DesignAnimation.standard(reduceMotion), value: isVisible)
        .animation(DesignAnimation.standard(reduceMotion), value: didCopy)
        .accessibilityLabel(didCopy ? "Copied" : "Copy")
    }
}

private struct TranscriptItemCardView: View {
    @Environment(\.palette) private var palette
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass

    let item: TranscriptItem
    let isExpanded: Bool
    let onToggleExpanded: () -> Void

    @State private var isHoveringDetail = false

    private var card: TranscriptItemCard { item.card }

    private var usesMonospaceDetail: Bool {
        card.type == .command || card.type == .tool
    }

    private var showCopyButton: Bool {
        horizontalSizeClass == .compact || isHoveringDetail
    }

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
                if card.type == .file {
                    DiffLinesView(diff: detail)
                        .hoverTracking { hovering in isHoveringDetail = hovering }
                } else {
                    VStack(alignment: .leading, spacing: Spacing.xs) {
                        HStack {
                            Spacer(minLength: 0)
                            CopyButton(content: detail, isVisible: showCopyButton)
                        }

                        if usesMonospaceDetail {
                            ScrollView(.horizontal) {
                                Text(detail)
                                    .font(Typography.code(12))
                                    .foregroundStyle(palette.textSecondary)
                                    .textSelection(.enabled)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .fixedSize(horizontal: true, vertical: false)
                            }
                        } else {
                            Text(detail)
                                .font(Typography.caption())
                                .foregroundStyle(palette.textSecondary)
                                .textSelection(.enabled)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }
                    .hoverTracking { hovering in
                        isHoveringDetail = hovering
                    }
                }
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
                                toggleMembership(item.id, in: &expandedItemIds)
                            }
                        )
                    }
                }
            }
        }
    }
}

enum MessageSegment: Equatable {
    case text(String)
    case code(language: String?, content: String)
}

func messageTimestampLabels(for transcript: [TranscriptEntry]) -> [UUID: String] {
    var labels: [UUID: String] = [:]
    var previousTimestamp: Date?

    for entry in transcript {
        guard case .message(let message) = entry,
              message.role != .system else {
            continue
        }

        let timestamp = message.timestamp

        guard let previous = previousTimestamp else {
            labels[message.id] = formatMessageTimestamp(timestamp)
            previousTimestamp = timestamp
            continue
        }

        if message.role == .user {
            labels[message.id] = formatMessageTimestamp(timestamp)
        } else if let label = timestampLabel(
            forGapSincePreviousMessage: timestamp.timeIntervalSince(previous),
            timestamp: timestamp
        ) {
            labels[message.id] = label
        }

        previousTimestamp = timestamp
    }

    return labels
}

func timestampLabel(forGapSincePreviousMessage gap: TimeInterval, timestamp: Date) -> String? {
    if gap <= 60 {
        return nil
    }
    if gap > 3600 {
        return formatMessageTimestamp(timestamp)
    }
    let minutes = max(1, Int(gap / 60))
    return "\(minutes)m ago"
}

func formatMessageTimestamp(_ timestamp: Date) -> String {
    timestamp.formatted(date: .omitted, time: .shortened)
}

func parseMessageSegments(_ content: String) -> [MessageSegment] {
    guard !content.isEmpty else { return [] }
    guard content.contains("```") else { return [.text(content)] }

    let lines = content.split(omittingEmptySubsequences: false, whereSeparator: \.isNewline)
    var segments: [MessageSegment] = []
    var textLines: [String] = []
    var codeLines: [String] = []
    var currentLanguage: String?
    var inCodeBlock = false

    func flushText() {
        let value = textLines.joined(separator: "\n")
        if !value.isEmpty {
            segments.append(.text(value))
        }
        textLines.removeAll(keepingCapacity: true)
    }

    func flushCode() {
        let value = codeLines.joined(separator: "\n")
        segments.append(.code(language: currentLanguage, content: value))
        codeLines.removeAll(keepingCapacity: true)
        currentLanguage = nil
    }

    for rawLine in lines {
        let line = String(rawLine)
        let trimmed = line.trimmingCharacters(in: .whitespaces)

        if inCodeBlock {
            if trimmed == "```" {
                flushCode()
                inCodeBlock = false
            } else {
                codeLines.append(line)
            }
            continue
        }

        if trimmed.hasPrefix("```") {
            flushText()
            inCodeBlock = true
            let suffix = trimmed.dropFirst(3).trimmingCharacters(in: .whitespaces)
            currentLanguage = suffix.isEmpty ? nil : String(suffix)
        } else {
            textLines.append(line)
        }
    }

    if inCodeBlock {
        flushCode()
    } else {
        flushText()
    }

    return segments.isEmpty ? [.text(content)] : segments
}

private func toggleMembership(_ id: UUID, in set: inout Set<UUID>) {
    if !set.insert(id).inserted {
        set.remove(id)
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
