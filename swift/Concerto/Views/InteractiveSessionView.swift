// Interactive session view wrapping GhosttyTerminalView with session controls.
// Displays embedded terminal running `lf <step>` in the wave's worktree.

import SwiftUI
import LoopflowCore

struct InteractiveSessionView: View {
    let session: InteractiveSession

    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.colorScheme) private var colorScheme
    @StateObject private var ghosttyManager = GhosttyManager.shared

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    private var wave: WaveViewModel? {
        repoState.waveStore.wave(for: session.waveId)
    }

    var body: some View {
        VStack(spacing: 0) {
            sessionHeader
            Divider()
            terminalContent
            Divider()
            sessionFooter
        }
        .background(palette.background)
        .task {
            // Set up callback for when terminal process exits
            let currentOutputBuffer = outputBuffer
            GhosttyManager.shared.onSessionClosed = {
                Task { @MainActor in
                    currentOutputBuffer.endInteractiveSession()
                }
            }
        }
    }

    // MARK: - Header

    private var sessionHeader: some View {
        HStack(spacing: 12) {
            // Status indicator
            Circle()
                .fill(Color.statusSuccess)
                .frame(width: 8, height: 8)

            // Wave name
            if let wave {
                Text(wave.displayName)
                    .font(.headline)
                    .fontWeight(.semibold)
            } else {
                Text("Session")
                    .font(.headline)
                    .fontWeight(.semibold)
            }

            // Step name
            Text(session.step)
                .font(.subheadline)
                .foregroundStyle(.secondary)

            // Interactive badge
            Text("interactive")
                .font(.caption2)
                .fontWeight(.medium)
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(Color.blue.opacity(0.15))
                .foregroundStyle(.blue)
                .clipShape(Capsule())

            Spacer()

            // Ghostty status indicator
            switch ghosttyManager.state {
            case .uninitialized:
                Text("Terminal not initialized")
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            case .initializing:
                ProgressView()
                    .scaleEffect(0.5)
            case .ready:
                EmptyView()
            case .failed(let error):
                Label("Error", systemImage: "exclamationmark.triangle")
                    .font(.caption2)
                    .foregroundStyle(Color.statusError)
                    .help(error)
            }
        }
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.md)
        .background(palette.surface)
    }

    // MARK: - Footer

    private var sessionFooter: some View {
        HStack(spacing: Spacing.lg) {
            Spacer()

            Button {
                cancelSession()
            } label: {
                Text("Cancel")
            }
            .buttonStyle(DarkButtonStyle())
            .keyboardShortcut(.escape, modifiers: [])
            .help("Cancel this session without advancing the flow")

            Button {
                continueSession()
            } label: {
                HStack(spacing: Spacing.xs) {
                    Image(systemName: "checkmark")
                    Text("Continue")
                }
            }
            .buttonStyle(.borderedProminent)
            .tint(Color.statusSuccess)
            .keyboardShortcut(.return, modifiers: .command)
            .help("Signal completion and advance to the next step")
        }
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.md)
        .background(palette.surface)
    }

    // MARK: - Terminal

    private var terminalContent: some View {
        GhosttyTerminalView(
            workingDirectory: session.worktreePath,
            command: session.command,
            sessionId: session.id,
            manager: ghosttyManager
        )
        .background(Color.black)
    }

    // MARK: - Actions

    private func cancelSession() {
        // Destroy the terminal surface, killing the child process (SIGTERM)
        GhosttyManager.shared.destroyActiveSession()
        // Clear the session state - wave stays in WAITING, can reconnect
        outputBuffer.endInteractiveSession()
    }

    private func continueSession() {
        // Send EOF (Ctrl+D = ASCII 4) to terminal
        // Process will exit gracefully, triggering onSessionClosed callback
        // which calls outputBuffer.endInteractiveSession()
        // Daemon sees exit code 0, advances flow
        let eof = "\u{04}"
        GhosttyManager.shared.sendText(eof)
    }
}

#Preview {
    let repoState = RepoState()
    repoState.configureMockWaves()

    // Create a mock session
    let wave = repoState.waves.first!
    let session = InteractiveSession(
        waveId: wave.id,
        step: "design",
        worktreePath: "/tmp/test-worktree"
    )
    let outputBuffer = OutputBuffer()
    outputBuffer.interactiveSession = session

    return InteractiveSessionView(session: session)
        .environment(repoState)
        .environment(outputBuffer)
        .frame(width: 600, height: 500)
}
