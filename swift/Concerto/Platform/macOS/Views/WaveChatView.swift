#if os(macOS)
import SwiftUI
import LoopflowCore

/// WaveChat: the live conversation with a running `lf wave <name>`. Discovers the
/// wave's chat server through its `.wave-endpoint` pointer, replays + streams the
/// thread over SSE, and posts messages back through the composer. The composer is
/// verb-aware — Send while idle, Steer / Interrupt & Send / Interrupt while a turn
/// runs — keyed off the streamed mind state. When the wave isn't running (no
/// pointer file, or the server refuses), it shows a clear not-running state and
/// keeps polling so it attaches the moment the wave comes up.
struct WaveChatView: View {
    let repoPath: String
    let waveName: String

    @Environment(\.palette) private var palette
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    @State private var connection: WaveChatConnection?
    @State private var composerText = ""
    @State private var sendError: String?
    @FocusState private var composerFocused: Bool

    private var identity: String { "\(repoPath)|\(waveName)" }

    private static let timestampFormatter: RelativeDateTimeFormatter = {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter
    }()

    var body: some View {
        VStack(spacing: 0) {
            transcript
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            Divider()
            composer
        }
        .background(palette.background)
        .task(id: identity) {
            connection?.stop()
            sendError = nil
            let conn = WaveChatConnection(repoPath: repoPath, waveName: waveName)
            connection = conn
            conn.start()
        }
        .onDisappear { connection?.stop() }
    }

    // MARK: - Transcript

    @ViewBuilder
    private var transcript: some View {
        let turns = connection?.turns ?? []
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: Spacing.lg) {
                    ForEach(turns) { turn in
                        MessageRow(turn: turn, timestampLabel: timestampLabel(for: turn))
                            .id(turn.id)
                    }
                    Color.clear
                        .frame(height: 1)
                        .id("transcript-bottom")
                }
                .padding(.horizontal, Spacing.xl)
                .padding(.vertical, Spacing.lg)
                .frame(maxWidth: 760, alignment: .leading)
                .frame(maxWidth: .infinity, alignment: .center)
            }
            .accessibilityIdentifier("wave-chat-transcript")
            .overlay {
                if turns.isEmpty {
                    statusOverlay
                }
            }
            .onChange(of: turns.count) { _, _ in
                scrollToBottom(proxy)
            }
            .onChange(of: turns.last?.text) { _, _ in
                scrollToBottom(proxy)
            }
        }
    }

    private func scrollToBottom(_ proxy: ScrollViewProxy) {
        withAnimation(DesignAnimation.fast(reduceMotion)) {
            proxy.scrollTo("transcript-bottom", anchor: .bottom)
        }
    }

    // MARK: - Status overlay (empty transcript)

    @ViewBuilder
    private var statusOverlay: some View {
        switch connection?.phase ?? .idle {
        case .notRunning, .idle:
            emptyState(
                icon: "moon.zzz",
                title: "Wave isn't running",
                message: "Start it with  lf wave \(waveName)  and its conversation appears here live."
            )
        case .connecting:
            VStack(spacing: Spacing.md) {
                ProgressView()
                    .controlSize(.small)
                Text("Connecting to \(waveName)…")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .live:
            emptyState(
                icon: "bubble.left.and.bubble.right",
                title: "No turns yet",
                message: "Send a message to start the conversation."
            )
        }
    }

    private func emptyState(icon: String, title: String, message: String) -> some View {
        VStack(spacing: Spacing.md) {
            Image(systemName: icon)
                .font(Typography.heroTitle(28))
                .foregroundStyle(palette.textSecondary.opacity(0.5))
            Text(title)
                .font(Typography.sectionTitle())
                .foregroundStyle(palette.text)
            Text(message)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 340)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    // MARK: - Composer
    //
    // The composer is verb-aware: it keys off the streamed mind state.
    // Idle + text → Send (op=message). Turning + text → Steer into the live
    // turn, with Interrupt & Send one click away. Turning + empty → Interrupt.
    // Verb selection lives in `composerVerbs` (LoopflowCore), tested there.

    private var isLive: Bool { connection?.phase == .live }

    private var hasText: Bool {
        !composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var verbs: ComposerVerbs {
        composerVerbs(state: connection?.mindState ?? .idle, hasText: hasText)
    }

    private var composer: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            if let sendError {
                Text(sendError)
                    .font(Typography.caption())
                    .foregroundStyle(Color.statusError)
                    .accessibilityIdentifier("wave-chat-send-error")
            }
            composerRow
        }
        .padding(Spacing.lg)
        .background(palette.background)
    }

    private var composerRow: some View {
        let verbs = self.verbs
        return HStack(alignment: .bottom, spacing: Spacing.sm) {
            TextField(composerPlaceholder, text: $composerText, axis: .vertical)
                .textFieldStyle(.plain)
                .font(Typography.body())
                .foregroundStyle(palette.text)
                .lineLimit(1...6)
                .padding(.horizontal, Spacing.md)
                .padding(.vertical, Spacing.sm)
                .background(palette.surfaceMuted)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                .focused($composerFocused)
                .disabled(!isLive)
                .onSubmit { perform(verbs.primary) }
                .accessibilityIdentifier("wave-chat-composer")

            if let secondary = verbs.secondary {
                Button(label(for: secondary)) { perform(secondary) }
                    .buttonStyle(.bordered)
                    .disabled(!isLive)
                    .accessibilityIdentifier("wave-chat-secondary")
            }

            Button(label(for: verbs.primary)) { perform(verbs.primary) }
                .keyboardShortcut(.return, modifiers: .command)
                .buttonStyle(DarkButtonStyle())
                .disabled(!isLive || !verbs.primaryEnabled)
                .accessibilityIdentifier("wave-chat-primary")
        }
    }

    private var composerPlaceholder: String {
        isLive ? "Message \(waveName)" : "Wave isn't running"
    }

    private func label(for verb: ComposerVerb) -> String {
        switch verb {
        case .send: return "Send"
        case .steer: return "Steer"
        case .interrupt: return "Interrupt"
        case .interruptAndSend: return "Interrupt & Send"
        }
    }

    private func perform(_ verb: ComposerVerb) {
        let text = composerText.trimmingCharacters(in: .whitespacesAndNewlines)
        let op: WaveMessageOp
        switch verb {
        case .send: op = .message
        case .steer: op = .steer
        case .interrupt, .interruptAndSend: op = .interrupt
        }
        // A bare interrupt carries no text; everything else requires some.
        guard verb == .interrupt || !text.isEmpty else { return }
        guard let connection, isLive else { return }
        composerText = ""
        sendError = nil
        Task {
            do {
                try await connection.send(text, op: op)
            } catch {
                // Don't lose the message: put it back in the composer (unless the
                // user already started typing something new) and say what failed.
                if !text.isEmpty, composerText.isEmpty {
                    composerText = text
                }
                sendError = "Send failed: \(error.localizedDescription)"
            }
        }
    }

    private func timestampLabel(for turn: ChatTurn) -> String? {
        guard let date = turn.createdAtDate else { return nil }
        return Self.timestampFormatter.localizedString(for: date, relativeTo: Date())
    }
}

#endif
