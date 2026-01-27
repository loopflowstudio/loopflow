// Interactive session view wrapping GhosttyTerminalView with session controls.
// Displays embedded terminal running `lf <step>` in the wave's worktree.

import SwiftUI
import LoopflowCore

struct InteractiveSessionView: View {
    let session: InteractiveSession

    @Environment(RepoState.self) private var repoState
    @Environment(SessionState.self) private var sessionState
    @Environment(\.colorScheme) private var colorScheme
    @StateObject private var ghosttyManager = GhosttyManager.shared

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    private var wave: Wave? {
        repoState.waves.first { $0.id == session.waveId }
    }

    var body: some View {
        VStack(spacing: 0) {
            sessionHeader
            Divider()
            terminalContent
        }
        .background(palette.background)
        .task {
            // Set up callback for when terminal process exits
            let currentSessionState = sessionState
            GhosttyManager.shared.onSessionClosed = {
                Task { @MainActor in
                    currentSessionState.endInteractiveSession()
                }
            }
        }
    }

    // MARK: - Header

    private var sessionHeader: some View {
        HStack(spacing: 12) {
            // Status indicator
            Circle()
                .fill(.green)
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
                    .foregroundStyle(.red)
                    .help(error)
            }

            // End session button
            Button {
                endSession()
            } label: {
                HStack(spacing: 4) {
                    Image(systemName: "xmark")
                        .font(.caption)
                    Text("End")
                }
            }
            .buttonStyle(DarkButtonStyle())
            .help("End this interactive session")
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 12)
        .background(palette.surface)
    }

    // MARK: - Terminal

    private var terminalContent: some View {
        GhosttyTerminalView(
            workingDirectory: session.worktreePath,
            command: "lf \(session.step)",
            sessionId: session.id,
            manager: ghosttyManager
        )
        .background(Color.black)
    }

    // MARK: - Actions

    private func endSession() {
        // Destroy the terminal surface, killing the child process
        GhosttyManager.shared.destroyActiveSession()
        // Clear the session state
        sessionState.endInteractiveSession()
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
    let sessionState = SessionState()
    sessionState.interactiveSession = session

    return InteractiveSessionView(session: session)
        .environment(repoState)
        .environment(sessionState)
        .frame(width: 600, height: 500)
}
