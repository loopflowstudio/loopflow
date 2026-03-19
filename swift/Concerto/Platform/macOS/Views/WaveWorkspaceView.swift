// Wave workspace — multiplexer view for the selected wave.
// Opens to a tmux-backed shell in the worktree. Supports split layouts
// with shell, markdown, diff, and launchpad panes.

import SwiftUI
import LoopflowCore

struct WaveWorkspaceView: View {
    let wave: WaveViewModel

    @Environment(RepoState.self) private var repoState

    var body: some View {
        MultiplexerView(
            waveId: wave.id,
            worktreePath: wave.worktreePath ?? wave.api.localWorktree
        )
        .id(wave.id)
    }
}
