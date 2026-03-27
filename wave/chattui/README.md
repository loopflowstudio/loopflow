# chattui

Terminal session portability. Launch agents in external terminals, reconnect from anywhere.

## Vision

The default experience of running a flow is: Concerto opens your real terminal (Ghostty, Warp) with the agent running in a tmux session. You watch it work, interact, close the window, walk away. Come back later, click "attach" in Concerto — reopen in an external terminal or pull it into the embedded Ghostty view. The tmux session is the source of truth. Windows are just views into it.

This decouples "where the agent runs" from "where you watch it." The agent doesn't know or care if you're in Concerto, a standalone terminal, or both at once.

### Not here

- Replacing the chat UI — chattui and chatgui serve different users and coexist
- Building a tmux wrapper — we use tmux as infrastructure, not as the product
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
