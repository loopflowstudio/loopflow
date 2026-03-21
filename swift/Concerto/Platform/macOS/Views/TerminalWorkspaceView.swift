import SwiftUI
import LoopflowCore

struct TerminalWorkspaceView: View {
    let waveId: String
    var onBackToWork: () -> Void = {}

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    private var sessions: [TerminalSession] {
        repoState.terminalWorkspaceStore.orderedSessions(for: waveId)
    }

    private var selectedSession: TerminalSession? {
        repoState.terminalWorkspaceStore.selectedSession(for: waveId)
    }

    var body: some View {
        if let session = selectedSession {
            HSplitView {
                terminalPane(for: session)
                    .frame(minWidth: 540)

                TerminalContextSidebar(session: session, onBackToWork: onBackToWork)
                    .frame(minWidth: 280, idealWidth: 320)
                    .background(palette.surface)
            }
        } else {
            ContentUnavailableView(
                "No active shell in this wave",
                systemImage: "terminal",
                description: Text("Return to Work or wait for this wave to request terminal attention.")
            )
            .overlay(alignment: .bottom) {
                Button("Back to work") {
                    onBackToWork()
                }
                .buttonStyle(.borderedProminent)
                .padding(.bottom, Spacing.xl)
            }
        }
    }

    private func terminalPane(for selectedSession: TerminalSession) -> some View {
        VStack(spacing: 0) {
            TerminalWorkspaceTabs(
                sessions: sessions,
                selectedSessionId: repoState.terminalWorkspaceStore.selectedSessionId(for: waveId),
                onSelect: { repoState.selectTerminalSession($0, waveId: waveId) }
            )

            Divider()

            ZStack {
                ForEach(sessions) { session in
                    SessionTerminalSurface(session: session)
                        .opacity(session.id == selectedSession.id ? 1 : 0)
                        .allowsHitTesting(session.id == selectedSession.id)
                        .accessibilityHidden(session.id != selectedSession.id)
                }
            }
            .background(Color.black)
        }
    }
}

private struct TerminalWorkspaceTabs: View {
    let sessions: [TerminalSession]
    let selectedSessionId: String?
    let onSelect: (String) -> Void

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: Spacing.sm) {
                ForEach(sessions) { session in
                    Button {
                        onSelect(session.id)
                    } label: {
                        HStack(spacing: Spacing.xs) {
                            Circle()
                                .fill(statusColor(session.status))
                                .frame(width: 8, height: 8)
                            Text(session.step)
                                .font(Typography.body())
                                .lineLimit(1)
                            Text(session.agent)
                                .font(Typography.caption())
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                        }
                        .padding(.horizontal, Spacing.md)
                        .padding(.vertical, Spacing.sm)
                        .background(selectedSessionId == session.id ? Color.loopflowBurgundy.opacity(0.12) : Color.clear)
                        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, Spacing.lg)
            .padding(.vertical, Spacing.sm)
        }
        .background(Color(nsColor: .windowBackgroundColor))
    }

    private func statusColor(_ status: TerminalSessionStatus) -> Color {
        switch status {
        case .pending, .attached: return .statusWarning
        case .running: return .statusSuccess
        case .succeeded: return .statusInfo
        case .failed, .canceled: return .statusError
        }
    }
}

private struct SessionTerminalSurface: View {
    let session: TerminalSession

    @Environment(RepoState.self) private var repoState
    @ObservedObject private var ghosttyManager = GhosttyManager.shared
    @State private var connectionInfo: TerminalConnectionInfo?
    @State private var errorMessage: String?

    var body: some View {
        Group {
            if let command = connectionInfo.map({ TerminalAttachCommand($0) }) {
                GhosttyTerminalView(
                    workingDirectory: command.workingDirectory,
                    argv: command.argv,
                    env: command.env,
                    sessionId: session.id,
                    manager: ghosttyManager
                )
            } else if ghosttyManager.hasSession(session.id) {
                ContentUnavailableView(
                    "Session already attached",
                    systemImage: "terminal",
                    description: Text("This terminal is already open in the current app process.")
                )
            } else if let errorMessage {
                ContentUnavailableView("Terminal unavailable", systemImage: "exclamationmark.triangle", description: Text(errorMessage))
            } else {
                ProgressView("Preparing terminal…")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .onChange(of: session.status) { _, newStatus in
            guard newStatus.isTerminal else { return }
            connectionInfo = nil
            ghosttyManager.destroySession(session.id)
        }
        .task(id: session.id) {
            guard connectionInfo == nil, !session.status.isTerminal else { return }
            do {
                connectionInfo = try await repoState.attachTerminalSession(session.id)
                errorMessage = nil
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }
}

struct TerminalAttachCommand: Equatable {
    let workingDirectory: String
    let argv: [String]
    let env: [String: String]

    init(
        workingDirectory: String,
        argv: [String],
        env: [String: String]
    ) {
        self.workingDirectory = workingDirectory
        self.argv = argv
        self.env = env
    }

    init(_ connection: TerminalConnectionInfo) {
        if connection.usesLocalTmux {
            self.init(
                workingDirectory: connection.cwd,
                argv: ["tmux", "attach-session", "-t", connection.sessionName],
                env: [:]
            )
            return
        }

        self.init(
            workingDirectory: FileManager.default.homeDirectoryForCurrentUser.path,
            argv: [
                "ssh",
                "-t",
                connection.host,
                "tmux attach-session -t \(shellEscape(connection.sessionName))",
            ],
            env: [:]
        )
    }
}

private func shellEscape(_ value: String) -> String {
    let escaped = value.replacingOccurrences(of: "'", with: "'\\''")
    return "'\(escaped)'"
}

private struct TerminalContextSidebar: View {
    let session: TerminalSession
    let onBackToWork: () -> Void

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    private let terminalLauncher = TerminalLauncher()

    private var wave: WaveViewModel? {
        repoState.waveStore.wave(for: session.waveId)
    }

    private var attentionItems: [AttentionItem] {
        repoState.attentionStore.ordered.filter { $0.waveId == session.waveId }
    }

    private var recentRuns: [WaveRun] {
        Array(repoState.runStore.runs(for: session.waveId).prefix(3))
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.lg) {
                identitySection
                currentWorkSection
                queuePressureSection
                recentHistorySection
                controlsSection
            }
            .padding(Spacing.xl)
        }
        .background(palette.surface)
        .task(id: session.waveId) {
            repoState.loadWaveContent(for: session.waveId)
            repoState.loadRuns(for: session.waveId)
        }
    }

    private var identitySection: some View {
        section("Wave") {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                Text(wave?.displayName ?? session.waveId)
                    .font(Typography.sectionTitle())
                keyValue("Status", wave?.statusText ?? session.status.rawValue.capitalized)
                keyValue("Branch", wave?.branch ?? "—")
                keyValue("Worktree", wave?.worktreePath ?? session.cwd)
                keyValue("Step", session.step)
                keyValue("Agent", session.agent)
            }
        }
    }

    private var currentWorkSection: some View {
        section("Current work") {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                if let item = wave?.content?.roadmapItems.first(where: { !$0.isShipped }) {
                    Text(item.title)
                        .font(Typography.body())
                        .fontWeight(.semibold)
                    if let content = item.content, !content.isEmpty {
                        Text(content)
                            .font(Typography.caption())
                            .foregroundStyle(.secondary)
                    }
                } else if let scratchDoc = wave?.content?.scratchDoc, !scratchDoc.isEmpty {
                    Text(scratchDoc)
                        .font(Typography.caption())
                        .foregroundStyle(.secondary)
                } else {
                    Text("No roadmap item loaded yet.")
                        .font(Typography.caption())
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    private var queuePressureSection: some View {
        section("Queue pressure") {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                keyValue("This wave", "\(attentionItems.count) unresolved")
                keyValue("Repo total", "\(repoState.attentionStore.ordered.count) unresolved")
                ForEach(attentionItems.prefix(3)) { item in
                    VStack(alignment: .leading, spacing: Spacing.xs) {
                        Text(item.title)
                            .font(Typography.body())
                        Text(item.summary)
                            .font(Typography.caption())
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
    }

    private var recentHistorySection: some View {
        section("Recent history") {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                if let wave, let url = wave.prURL {
                    keyValue("PR", url.lastPathComponent)
                }
                ForEach(recentRuns.prefix(3)) { run in
                    VStack(alignment: .leading, spacing: Spacing.xs) {
                        Text(run.status.rawValue.capitalized)
                            .font(Typography.body())
                        Text(run.branch ?? "—")
                            .font(Typography.caption())
                            .foregroundStyle(.secondary)
                    }
                }
                ForEach((wave?.commits ?? []).prefix(3)) { commit in
                    Text(commit.message)
                        .font(Typography.caption())
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    private var controlsSection: some View {
        section("Controls") {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                Button("Stop session", role: .destructive) {
                    Task {
                        _ = try? await repoState.cancelTerminalSession(session.id)
                        GhosttyManager.shared.destroySession(session.id)
                    }
                }
                .buttonStyle(.bordered)

                Button("Back to work") {
                    repoState.selectTerminalSession(nil, waveId: session.waveId)
                    onBackToWork()
                }
                .buttonStyle(.bordered)

                if let worktree = wave?.worktreePath {
                    Button("Open in Cursor") {
                        try? terminalLauncher.openInIDE(.cursor, at: URL(fileURLWithPath: worktree), remoteHost: repoState.repoTarget?.remoteHost)
                    }
                    .buttonStyle(.bordered)

                    if !repoState.isRemoteTarget {
                        Button("Reveal in Finder") {
                            terminalLauncher.openInFinder(at: URL(fileURLWithPath: worktree))
                        }
                        .buttonStyle(.bordered)
                    }
                }
            }
        }
    }

    private func section<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text(title)
                .font(Typography.sectionTitle())
                .foregroundStyle(Color.loopflowBurgundy)
            content()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func keyValue(_ title: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text(title)
                .font(Typography.caption())
                .foregroundStyle(.secondary)
            Text(value)
                .font(Typography.body())
        }
    }
}
