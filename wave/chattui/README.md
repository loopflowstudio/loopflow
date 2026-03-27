# chattui

Terminal session portability. Launch agents in external terminals, reconnect from anywhere.

## Vision

Agents run in real terminals — Ghostty windows you interact with directly. But when you have multiple agents across multiple worktrees, managing one-off windows breaks down. You close a window and lose track of what was running. You can't tell which agent is waiting for input.

Concerto is the session manager. It launches agents into tmux sessions, opens Ghostty windows for interaction, and shows all active sessions in one place. Close the windows, find your terminals again. Reopen in a new Ghostty window or use it right inside Concerto. The tmux session is the source of truth — windows are just attachment points.

The embedded terminal uses libghostty — fully interactive, same rendering engine. But the full Ghostty app will always have an edge: tabs, splits, its own config system, GPU optimizations, faster updates. For a long interactive session, the real Ghostty window is the better experience. The embedded view is great for quick checks, short interactions, and keeping an eye on things without leaving Concerto. Make it easy to pop out to a real Ghostty window when you want to settle in.

### Not here

- Replacing external terminals — Ghostty windows are the primary interaction surface
- Building a terminal emulator — Ghostty embedding is for convenience, not competition
- Deep integration with agent internals — we launch commands and track sessions, we don't parse agent output

## Goals

1. External terminal as the default launch target for flows/steps
2. One-click reattach from Concerto — embedded or external
3. Session persistence — survive app restarts, terminal closes, disconnects
4. Multi-agent dispatch — choose Claude Code, Codex, or OpenCode per step

## Risks

- Terminal detection and launching is fragile across macOS versions. The existing `TerminalLauncher` uses AppleScript for some terminals — this breaks silently.
- tmux version differences. Session naming, attach semantics, and environment passing vary.
- User's preferred terminal might not support the attach workflow cleanly (Terminal.app is limited).

## Metrics

- Time from "run step" click to visible terminal: < 1s
- Reattach (embedded or external) from cold: < 500ms
- Zero orphaned tmux sessions after Concerto quit
