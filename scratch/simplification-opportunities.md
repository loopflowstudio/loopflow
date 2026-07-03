# Reshape PortfolioWindow → WavesView (reduce, don't add)

Reshape the proven `PortfolioWindow` into the shape sketched by `RepoSidebarWindow`,
then delete the sketch. Reuse proven components; the sketch kept reintroducing
style regressions the old components already solved.

## Component → role mapping

| New role (target shape)        | Proven component reused                          | Keep / adapt / drop |
| ------------------------------ | ------------------------------------------------ | ------------------- |
| Main window                    | `PortfolioWindow` (rename → `WavesView`)          | rename + reshape    |
| Connection / daemon / events   | `PortfolioWindow`'s connection + `EventService`   | keep verbatim       |
| Per-repo waves + create        | `PortfolioRepoState`                              | keep; add attach    |
| Repo sidebar (burgundy)        | `WaveSidebar`'s `Color.loopflowBurgundy` VStack   | adapt: list repos   |
| Wave-list rows                 | `WaveRow` (white-on-burgundy, proven)             | reuse verbatim      |
| New-wave launcher              | `CreateWaveSheet` (from sketch — already styled)  | port, keep          |
| Repo collapse + `~/src` scan   | `RepoScanner.resolveMainWorktree` / scan (sketch) | port logic          |
| Wave → /goal agent terminal    | `WaveService.ensureWaveAgent` + `attachSession`,  | reuse; embed        |
|                                | `TerminalAttachCommand` + `GhosttyTerminalView`   |                     |

## Shape

`HStack`: burgundy **repo rail** (All Repos + collapsed repos) · burgundy **wave
list** (WaveRow, filtered to the selected repo, "+" launcher) · **terminal detail**
(palette.background). Both left columns are burgundy so `WaveRow`'s white text is
correct — no `NavigationSplitView`, no gray sidebar, no black fields.

`ensureWaveAgent(waveId:)` (POST `waves/{id}/run`) starts the /goal agent if idle
and returns a `Session`; `attachSession(id)` returns tmux `SessionConnectionInfo`;
`TerminalAttachCommand(_:)` → `GhosttyTerminalView`. All proven, no `RepoState`
needed, no rust changes.

## Drop
- `RepoSidebarWindow.swift` (the sketch) — its `RepoSidebarWaveRow` superseded by `WaveRow`.
- `PortfolioRepoCard`, `AddRepoCard`, `RepoTypeahead` — the old grid dashboard body (only used by PortfolioWindow).
- Repoint `ConcertoApp` main `WindowGroup` + `"portfolio"` window to `WavesView`.
