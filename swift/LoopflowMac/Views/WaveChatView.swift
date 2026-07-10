#if os(macOS)
import SwiftUI
import Loopflow

/// WaveChat: the live conversation with a running `lf serve <name>`. Discovers the
/// wave's chat server through its `.wave-endpoint` pointer, replays + streams the
/// thread over SSE, and posts messages back through the composer. The composer is
/// verb-aware — Send while idle, Steer / Interrupt & Send / Interrupt while a turn
/// runs — keyed off the streamed loop state. When the wave isn't running (no
/// pointer file, or the server refuses), it shows a clear not-running state and
/// keeps polling so it attaches the moment the wave comes up.
struct WaveChatView: View {
    let repoPath: String
    let waveName: String

    @Environment(\.palette) private var palette
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    @State private var connection: WaveChatConnection?
    @State private var composerText = ""
    @State private var enqueueFlow = ""
    @State private var sendError: String?
    @State private var launch: LaunchState = .idle
    @State private var isStopping = false
    @State private var confirmStop = false
    @State private var isFollowingLatest = true
    @State private var isNearTranscriptBottom = true
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
            if isLive {
                playheadHeader
                Divider()
            }
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
            isStopping = false
            confirmStop = false
            isFollowingLatest = true
            isNearTranscriptBottom = true
            let conn = WaveChatConnection(repoPath: repoPath, waveName: waveName)
            connection = conn
            conn.start()
        }
        .onDisappear { connection?.stop() }
        .confirmationDialog(
            "Stop \(waveName)?",
            isPresented: $confirmStop,
            titleVisibility: .visible
        ) {
            Button("Stop wave", role: .destructive) { stopWave() }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Stops the wave listener and its resident. Detached worker loops keep running.")
        }
    }

    // MARK: - Transcript

    @ViewBuilder
    private var transcript: some View {
        let turns = connection?.turns ?? []
        let failures = attemptFailurePresentations(
            turns: turns,
            playhead: connection?.playhead,
            loopState: connection?.loopState ?? .idle
        )
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: Spacing.lg) {
                    ForEach(turns) { turn in
                        if let body = turn.body, turn.role == .assistant {
                            bodyBoundary(body, status: turn.status)
                        }
                        MessageRow(
                            turn: turn,
                            timestampLabel: timestampLabel(for: turn),
                            attemptFailure: failures[turn.id]
                        )
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
                if isFollowingLatest {
                    scrollToBottom(proxy)
                }
            }
            .onChange(of: turns.last?.text) { _, _ in
                if isFollowingLatest {
                    scrollToBottom(proxy)
                }
            }
            .onScrollGeometryChange(for: Bool.self) { geometry in
                geometry.visibleRect.maxY >= geometry.contentSize.height - Spacing.xl
            } action: { _, isNearBottom in
                isNearTranscriptBottom = isNearBottom
            }
            .onScrollPhaseChange { _, phase in
                switch phase {
                case .tracking, .interacting, .decelerating:
                    isFollowingLatest = false
                case .idle:
                    isFollowingLatest = isNearTranscriptBottom
                case .animating:
                    break
                }
            }
        }
    }

    // MARK: - Playhead

    private var playheadHeader: some View {
        let playhead = connection?.playhead
        let breadcrumb = (playhead?.stack.map(\.flow) ?? [])
            + (playhead?.now.map { [$0.step] } ?? [])
        return VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.sm) {
                Text(breadcrumb.isEmpty ? waveName : breadcrumb.joined(separator: " › "))
                    .font(Typography.caption().weight(.semibold))
                    .foregroundStyle(palette.text)
                    .lineLimit(1)
                Spacer()
                if let now = playhead?.now {
                    Text("\(now.index + 1)/\(now.total) · loop \(now.iteration + 1)")
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                }
                Button("Skip") { skipStep() }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .disabled(playhead?.now == nil)
                    .accessibilityIdentifier("wave-chat-skip")
                Button {
                    confirmStop = true
                } label: {
                    HStack(spacing: Spacing.xs) {
                        if isStopping {
                            ProgressView()
                                .controlSize(.small)
                        }
                        Text(isStopping ? "Stopping…" : "Stop")
                    }
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .foregroundStyle(Color.statusError)
                .disabled(isStopping)
                .accessibilityIdentifier("wave-chat-stop")
            }

            HStack(spacing: Spacing.lg) {
                playheadFact("now", playhead?.now.map { "\($0.flow) / \($0.step)" })
                playheadFact("next", playhead?.next.map { "\($0.flow) / \($0.step)" })
                playheadFact("return", playhead?.returnTo.map { "\($0.flow) / \($0.step)" })
            }

            if let queued = playhead?.stack.last?.queue, !queued.isEmpty {
                Text("queued  " + queued.map(\.flow).joined(separator: "  →  "))
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }

            HStack(spacing: Spacing.sm) {
                TextField("Enqueue flow", text: $enqueueFlow)
                    .textFieldStyle(.roundedBorder)
                    .controlSize(.small)
                    .onSubmit { enqueue() }
                    .accessibilityIdentifier("wave-chat-enqueue-field")
                Button("Enqueue") { enqueue() }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .disabled(enqueueFlow.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .accessibilityIdentifier("wave-chat-enqueue")
            }
        }
        .padding(.horizontal, Spacing.lg)
        .padding(.vertical, Spacing.sm)
        .background(palette.surfaceMuted.opacity(0.45))
    }

    private func playheadFact(_ label: String, _ value: String?) -> some View {
        HStack(spacing: Spacing.xs) {
            Text(label)
                .foregroundStyle(palette.textSecondary)
            Text(value ?? "—")
                .foregroundStyle(palette.text)
                .lineLimit(1)
        }
        .font(Typography.caption())
    }

    private func bodyBoundary(_ body: BodyProvenance, status: Lifecycle) -> some View {
        DisclosureGroup {
            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text("body  \(body.bodyId)")
                Text("invocation  \(body.invocationId)")
                Text("step index  \(body.stepIndex)")
                Text("session  \(body.sessionId ?? "—")")
                Text("harness  \(body.harness ?? "—") · model  \(body.model ?? "—")")
                Text("host  \(body.host)")
                Text("worktree  \(body.worktree)")
                Text("started  \(body.startedAt)")
                Text("finished  \(body.endedAt ?? "—")")
                if let reason = body.terminationReason {
                    Text("reason  \(reason)")
                }
            }
            .font(Typography.caption())
            .foregroundStyle(palette.textSecondary)
            .textSelection(.enabled)
        } label: {
            HStack(spacing: Spacing.sm) {
                Rectangle()
                    .fill(palette.border)
                    .frame(height: 1)
                Text("\(body.flow) / \(body.step) · \(bodyLabel(status, body))")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                    .fixedSize()
                Rectangle()
                    .fill(palette.border)
                    .frame(height: 1)
            }
        }
        .accessibilityIdentifier("wave-chat-body-\(body.bodyId)")
    }

    private func bodyLabel(_ status: Lifecycle, _ body: BodyProvenance) -> String {
        if body.terminationReason == "skipped by user" { return "skipped" }
        switch status {
        case .running: return "now playing"
        case .completed: return "completed"
        case .interrupted: return "interrupted"
        case .failed: return "failed"
        default: return String(describing: status)
        }
    }

    private func enqueue() {
        let flow = enqueueFlow.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !flow.isEmpty, let connection else { return }
        enqueueFlow = ""
        Task {
            do {
                try await connection.enqueue(flow)
            } catch {
                enqueueFlow = flow
                sendError = "Enqueue failed: \(error.localizedDescription)"
            }
        }
    }

    private func skipStep() {
        guard let connection else { return }
        Task {
            do {
                try await connection.skip()
            } catch {
                sendError = "Skip failed: \(error.localizedDescription)"
            }
        }
    }

    private func stopWave() {
        guard !isStopping else { return }
        isStopping = true
        sendError = nil
        let repoPath = repoPath
        let waveName = waveName
        Task {
            do {
                try await Task.detached {
                    try LocalWaveAgentLauncher.stopWave(repoPath: repoPath, waveName: waveName)
                }.value
            } catch {
                sendError = "Stop failed: \(error.localizedDescription)"
            }
            isStopping = false
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
    // as a terminal: `lf serve <name>` at the wave's repo. Quitting Loopflow
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

    /// Launch `lf serve` detached, then wait for the endpoint poll to attach.
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
                    + "tmux attach -r -t \(PortfolioRepoState.waveAgentSessionName(repoPath: repoPath, waveName: waveName))"
            )
        }
    }

    // MARK: - Composer
    //
    // The composer is verb-aware: it keys off the streamed loop state.
    // Idle + text → Send (op=message). Turning + text → Steer into the live
    // turn, with Interrupt & Send one click away. Turning + empty → Interrupt.
    // Verb selection lives in `composerVerbs` (Loopflow), tested there.

    private var isLive: Bool { connection?.phase == .live }

    private var hasText: Bool {
        !composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var verbs: ComposerVerbs {
        composerVerbs(state: connection?.loopState ?? .idle, hasText: hasText)
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
    let markdown = "Start it here, or run `lf serve \(waveName)` in a terminal — "
        + "its conversation appears here live."
    return (try? AttributedString(markdown: markdown))
        ?? AttributedString(markdown.replacingOccurrences(of: "`", with: ""))
}

#endif
