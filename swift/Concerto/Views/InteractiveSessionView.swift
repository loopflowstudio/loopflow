// Interactive session view wrapping GhosttyTerminalView with session controls.
// Displays embedded terminal running `lf <step>` in the agent's worktree.

import SwiftUI
import LoopflowCore

struct InteractiveSessionView: View {
    let session: InteractiveSession
    @Bindable var appState: AppState

    @Environment(\.colorScheme) private var colorScheme
    @StateObject private var ghosttyManager = GhosttyManager.shared

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    private var agent: Agent? {
        appState.agents.first { $0.id == session.agentId }
    }

    var body: some View {
        VStack(spacing: 0) {
            sessionHeader
            Divider()
            terminalContent
        }
        .background(palette.background)
    }

    // MARK: - Header

    private var sessionHeader: some View {
        HStack(spacing: 12) {
            // Status indicator
            Circle()
                .fill(.green)
                .frame(width: 8, height: 8)

            // Agent name
            if let agent {
                Text(agent.displayName)
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
            manager: ghosttyManager
        )
        .background(Color.black)
    }

    // MARK: - Actions

    private func endSession() {
        appState.endInteractiveSession()
    }
}

#Preview {
    let state = AppState()
    state.configureMockAgents()

    // Create a mock session
    let agent = state.agents.first!
    let session = InteractiveSession(
        agentId: agent.id,
        step: "design",
        worktreePath: "/tmp/test-worktree"
    )
    state.activeSession = session

    return InteractiveSessionView(session: session, appState: state)
        .frame(width: 600, height: 500)
}
