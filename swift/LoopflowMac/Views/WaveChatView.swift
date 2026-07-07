#if os(macOS)
import SwiftUI
import Loopflow

/// WaveChat: the live conversation with a running `lf wave <name>`. Discovers the
/// wave's chat server through its `.wave-endpoint` pointer, replays + streams the
/// thread over SSE, and posts messages back through the composer. The composer is
/// verb-aware — Send while idle, Steer / Interrupt & Send / Interrupt while a turn
/// runs — keyed off the streamed flowloop state. When the wave isn't running (no
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
    @State private var launch: LaunchState = .idle
    @FocusState private var composerFocused: Bool

    enum LaunchState: Equatable {
        case idle
        case starting
        case failed(String)
    }

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
            launch = .idle
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
            notRunningState
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

    // MARK: - Not running (start the wave)
    //
    // The wave is a detached tmux session, launched here through the same door
    // as a terminal: `lf wave <name>` at the wave's repo. Quitting Loopflow
    // never touches it. After a launch, the connection's 1s endpoint poll picks
    // the wave up on its own — this view just waits for the phase to move.

    private var notRunningState: some View {
        VStack(spacing: Spacing.md) {
            Image(systemName: "moon.zzz")
                .font(Typography.heroTitle(28))
                .foregroundStyle(palette.textSecondary.opacity(0.5))
            Text("Wave isn't running")
                .font(Typography.sectionTitle())
                .foregroundStyle(palette.text)
            Text(waveStartHint(waveName: waveName))
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 340)
            Button {
                startWave()
            } label: {
                HStack(spacing: Spacing.xs) {
                    if launch == .starting {
                        ProgressView()
                            .controlSize(.small)
                    }
                    Text(launch == .starting ? "Starting…" : "Start wave")
                }
            }
            .buttonStyle(DarkButtonStyle())
            .disabled(launch == .starting)
            .accessibilityIdentifier("wave-chat-start")
            if case .failed(let message) = launch {
                Text(message)
                    .font(Typography.caption())
                    .foregroundStyle(Color.statusError)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 340)
                    .accessibilityIdentifier("wave-chat-start-error")
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    /// Launch `lf wave` detached, then wait for the endpoint poll to attach.
    /// The launch itself is quick (tmux returns immediately); the wave server
    /// takes a few seconds to publish its endpoint.
    private func startWave() {
        guard launch != .starting else { return }
        launch = .starting
        let repoPath = repoPath
        let waveName = waveName
        Task {
            do {
                try await Task.detached {
                    try LocalWaveAgentLauncher.launchWave(repoPath: repoPath, waveName: waveName)
                }.value
            } catch {
                launch = .failed(error.localizedDescription)
                return
            }
            // The connection polls the endpoint pointer every second; give the
            // wave server up to 20s to come up before calling it failed.
            let deadline = Date().addingTimeInterval(20)
            while Date() < deadline {
                // Bail if the pane moved to a different wave mid-wait.
                guard let conn = connection, conn.repoPath == repoPath, conn.waveName == waveName else { return }
                let phase = conn.phase
                if phase != .notRunning && phase != .idle {
                    launch = .idle
                    return
                }
                try? await Task.sleep(nanoseconds: 500_000_000)
            }
            guard let conn = connection, conn.repoPath == repoPath, conn.waveName == waveName else { return }
            launch = .failed(
                "Wave didn't come up. Check the tmux session for what went wrong: "
                    + "tmux attach -t \(PortfolioRepoState.waveAgentSessionName(repoPath: repoPath, waveName: waveName))"
            )
        }
    }

    // MARK: - Composer
    //
    // The composer is verb-aware: it keys off the streamed flowloop state.
    // Idle + text → Send (op=message). Turning + text → Steer into the live
    // turn, with Interrupt & Send one click away. Turning + empty → Interrupt.
    // Verb selection lives in `composerVerbs` (Loopflow), tested there.

    private var isLive: Bool { connection?.phase == .live }

    private var hasText: Bool {
        !composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var verbs: ComposerVerbs {
        composerVerbs(state: connection?.flowloopState ?? .idle, hasText: hasText)
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

/// The not-running hint, with the launch command as inline code so `lf` can't
/// be misread as "If". Plain-string fallback only if markdown parsing fails.
func waveStartHint(waveName: String) -> AttributedString {
    let markdown = "Start it here, or run `lf wave \(waveName)` in a terminal — "
        + "its conversation appears here live."
    return (try? AttributedString(markdown: markdown))
        ?? AttributedString(markdown.replacingOccurrences(of: "`", with: ""))
}

#endif
