---
linear_id: 158f1319-795e-4dd0-be62-70e55df76494
---
# Terminal Embedding

**Finish line:** Coding sessions run in embedded Ghostty terminals inside Concerto. The agent runs there. Concerto wraps it with wave context — not replacing the terminal, enriching it.

## Context

Stop competing on chat. Claude Code, Cursor, Windsurf, OpenCode — all ahead on polish, all iterating faster. Concerto embeds their terminals and builds the UX around coding sessions.

The agent runs in a real terminal. Concerto provides: which wave this session belongs to, what the wave's current focus is, recent history, related attention items, and queue pressure. The human sees the terminal plus the context that makes it meaningful.

## Foundation in place

`lfd` now persists `TerminalSession` records with typed lifecycle state (pending/attached/running/completed/cancelled), exposes `/v0/terminal-sessions/*` HTTP routes, and emits session updates through the event hub. LoopflowCore has `TerminalWorkspaceStore` and `LocalWaveService` consuming those routes and events. The daemon-side plumbing for session binding and lifecycle tracking exists — what's missing is the Ghostty view embedding and native UI surfaces.

## What to build

1. **Ghostty terminal view.** Embedded terminal component in Concerto. A coding session opens a terminal pane with the agent running inside. Full terminal emulation — not a chat widget.
2. **Wave context sidebar.** Next to the terminal: which wave, current work item, recent PRs, active attention items, and queue state. Updates live as the session progresses.
3. **Session → wave binding.** Wire through the existing `TerminalSession` model — when a session starts, it's bound to a wave. The terminal opens in the wave's worktree. The agent gets the wave's area, direction, and flow context. `lfd` already tracks this binding; Concerto needs to present it.
4. **Multi-session.** Multiple terminals, multiple waves, tiled or tabbed. The conductor running three agents at once — coherent because each is bound to a wave with context. `lfd` already supports concurrent sessions with independent lifecycle.

## Done when

* Ghostty terminal renders inside Concerto
* Terminal sessions are bound to waves with visible context
* Multiple concurrent sessions are manageable
* Agent output is real terminal output, not parsed/reformatted
