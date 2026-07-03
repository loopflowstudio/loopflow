---
asana_id: '1216257840693375'
---
# Reshape the surface, trim the UI

**Finish line:** The exposed Concerto surface is `WavesView` — a burgundy repo
sidebar filtering a wave list, a new-wave launcher, and clicking a wave opens its
`/goal` agent in an embedded tmux terminal — **reshaped** from the battle-tested
`PortfolioWindow` / `WaveSidebar` / `WaveRow` / `TerminalWorkspaceView`, not rebuilt
fresh. Every UI file not reachable from `WavesView` is deleted.

## Context

The fresh `RepoSidebarWindow` rebuild kept reintroducing regressions the proven
components already solved (gray sidebar, black fields). The move is
reshape-not-rebuild: adapt the proven surface into the target shape, drop the
sketch, trim hard.

- **Reshape** `PortfolioWindow` → `WavesView`: repo sidebar (worktree-collapsed,
  `~/src` scan) reusing `WaveSidebar`'s `Color.loopflowBurgundy` treatment; wave
  list reusing `WaveRow`; the styled `CreateWaveSheet` launcher.
- **Wave screen:** click a wave → its `/goal` agent in an embedded terminal,
  reusing `launch_wave_agent_session` + the `TerminalWorkspaceView` attach path.
- **Drop** `RepoSidebarWindow`; repoint `ConcertoApp`'s main window to `WavesView`.
- **Trim:** delete every `Platform/macOS/Views/*.swift` not reachable from
  `WavesView` — old wave-workspace, `MultiplexerView`, native-chat stack
  (`WaveSessionView`, `ReplyQueue`, `SelectableAssistantMessageTextView`,
  `VoiceInputButton`), `CommandPalette`, the flow/typeahead editors, `CatchWaveView`.
  `xcodebuild` green is the arbiter of dead vs. reachable.

## Done when

- `WavesView` is the exposed surface, styled from proven components — no gray
  sidebar, no black fields.
- Clicking a wave shows its `/goal` agent in an embedded tmux terminal.
- The UI is net-negative: the unreachable view files are gone; build stays green.
