// Wave workspace — multiplexer view for the selected wave.
// Opens to a tmux-backed shell in the worktree. Supports split layouts
// with shell, markdown, diff, and launchpad panes.

import SwiftUI
import LoopflowCore

struct WaveWorkspaceView: View {
    let wave: WaveViewModel

    @Environment(RepoState.self) private var repoState

    var body: some View {
<<<<<<< HEAD
        MultiplexerView(
            waveId: wave.id,
            worktreePath: wave.worktreePath ?? wave.api.localWorktree
        )
        .id(wave.id)
=======
        VStack(spacing: 0) {
            if hasTerminal {
                tabBar
                Divider()
            }

            if selectedTab == .terminal {
                if repoState.terminalWorkspaceStore.selectedSession != nil {
                    TerminalWorkspaceView()
                } else {
                    ProgressView("Loading terminal session…")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            } else {
                WaveDetailPanel(wave: wave)
                    .id(wave.id)
            }
        }
        .onChange(of: hasTerminal) { _, hasIt in
            if !hasIt && selectedTab == .terminal {
                selectedTab = .work
            }
        }
        .onChange(of: terminalSession?.id) { _, sessionId in
            guard let sessionId,
                  repoState.consumeAutoPresentTerminal(for: wave.id) else { return }
            selectedTab = .terminal
            repoState.selectTerminalSession(sessionId)
        }
        .onChange(of: wave.id) { _, _ in
            selectedTab = .work
        }
    }

    private var tabBar: some View {
        HStack(spacing: Spacing.sm) {
            tabButton("Work", systemImage: "rectangle.stack", tab: .work)
            tabButton("Terminal", systemImage: "terminal", tab: .terminal)
            Spacer()
        }
        .padding(.horizontal, Spacing.lg)
        .padding(.vertical, Spacing.sm)
        .background(palette.surface)
    }

    private func tabButton(_ label: String, systemImage: String, tab: WorkspaceTab) -> some View {
        Button {
            selectedTab = tab
            if tab == .terminal, let session = terminalSession {
                repoState.selectTerminalSession(session.id)
            } else if tab == .work {
                repoState.selectTerminalSession(nil)
            }
        } label: {
            Label(label, systemImage: systemImage)
                .font(Typography.body())
                .padding(.horizontal, Spacing.md)
                .padding(.vertical, Spacing.sm)
                .background(selectedTab == tab ? Color.loopflowBurgundy.opacity(0.12) : Color.clear)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        }
        .buttonStyle(.plain)
>>>>>>> eb790e5f (concerto: stabilize bundled daemon terminal handoff)
    }
}
