// Wave workspace — the primary selected-wave container.
// Routes to the right surface: native work view by default,
// embedded terminal tab when a terminal session exists for this wave.

import SwiftUI
import LoopflowCore

struct WaveWorkspaceView: View {
    let wave: WaveViewModel

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    private enum WorkspaceTab: Hashable {
        case work
        case terminal
    }

    @State private var selectedTab: WorkspaceTab = .work

    private var terminalSession: TerminalSession? {
        repoState.terminalWorkspaceStore.activeSession(for: wave.id)
    }

    private var hasTerminal: Bool {
        terminalSession != nil
    }

    var body: some View {
        VStack(spacing: 0) {
            if hasTerminal {
                tabBar
                Divider()
            }

            if selectedTab == .terminal,
               repoState.terminalWorkspaceStore.selectedSession != nil {
                TerminalWorkspaceView()
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
    }
}
