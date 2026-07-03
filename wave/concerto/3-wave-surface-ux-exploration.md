---
asana_id: '1216257471758208'
---
# wave-surface UX exploration

**Finish line:** The terminal-heavy screens — the wave screen (harness + yazi +
ad-hoc terminals + RepoWork strip) and the waiting-nudge — are designed from
research, not guesses. Ship the simplest UX first (in A+B), then explore 2–3
variants and converge from evidence.

## Context

Agent-supervision-as-window-manager is a young genre; prior art is thin. The A+B
slices ship the simplest possible UX to prove the architecture. This item is the
deliberate UX pass on top.

- **Research first, documented in MEMORY** — how comparable tools (Warp, tmux
  managers, agent dashboards) handle multi-session supervision; the wave screen's
  proportions and pane-switching; how "this wave needs you" should surface.
- **Simplest UX** — the bare three screens from A+B, deliberately unpolished.
- **2–3 variants** — of the wave-screen layout and the waiting-nudge.
- **Converge** — pick from the variants; record why in MEMORY.

## Open

- **Waiting-nudge** is the sharp one: when a terminal-hosted wave needs you, is
  the rollup `waiting` chip enough, or does the director need a native nudge (the
  one piece of the dropped chat stack that might have to survive)? Resist
  rebuilding the reply queue.

## Done when

- UX research for the wave screen + waiting-nudge is recorded in desktop MEMORY.
- 2–3 wave-screen variants were built and evaluated.
- A converged design is chosen and documented, ready to replace the
  simplest-UX placeholder from A+B.
