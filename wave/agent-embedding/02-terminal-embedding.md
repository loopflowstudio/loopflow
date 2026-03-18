---
asana_id: '1213718096106556'
linear_id: ff18178d-77f8-4557-a9bb-b183e78b944d
---
# 02: Terminal Embedding

**Finish line:** Coding sessions run in embedded Ghostty terminals inside Concerto. The agent runs there. Concerto wraps it with wave context — not replacing the terminal, enriching it.

## Context

Stop competing on chat. Claude Code, Cursor, Windsurf, OpenCode — all ahead on polish, all iterating faster. Concerto embeds their terminals and builds the UX around coding sessions.

The agent runs in a real terminal. Concerto provides: which wave this session belongs to, what the wave's current focus is, recent history, related attention items, and queue pressure. The human sees the terminal plus the context that makes it meaningful.

## What to build

1. **Ghostty terminal view.** Embedded terminal component in Concerto. A coding session opens a terminal pane with the agent running inside. Full terminal emulation — not a chat widget.

2. **Wave context sidebar.** Next to the terminal: which wave, current work item, recent PRs, active attention items, and queue state. Updates live as the session progresses.

3. **Session → wave binding.** When a session starts, it's bound to a wave. The terminal opens in the wave's worktree. The agent gets the wave's area, direction, and flow context.

4. **Multi-session.** Multiple terminals, multiple waves, tiled or tabbed. The conductor running three agents at once — coherent because each is bound to a wave with context.

## Done when

- Ghostty terminal renders inside Concerto
- Terminal sessions are bound to waves with visible context
- Multiple concurrent sessions are manageable
- Agent output is real terminal output, not parsed/reformatted
