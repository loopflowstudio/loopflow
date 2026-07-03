---
priority: medium
---

# Concerto — per-repo looping sessions

**Finish line:** Concerto shows, per repo, the looping sessions: a launcher +
deep-link for cloud (backend a), a real dashboard for hosted lfd (backend b).

## Context

Concerto is the macOS surface raised a layer (2026-06-19): wave monitoring plus
the frame around vendor sessions, not a chat client. Goals add a per-repo view
of what's looping. The two backends have intentionally different depths.

Concerto's data source is now the unified Swift `Session` model — the
`AgentSession` (transcript/input) and `TerminalSession` (tmux/Ghostty pane)
split was collapsed into one `Session` during the Wave/Run/Session reduction.
The dashboard reads `Session` + `Run` + the `WaveAgentTree`; tmux/Ghostty stay
UI transport details, not product nouns. Build on that model, not the old split.

## What to shape

- **Per-repo looping session list** — one entry per active Wave's Looping Agent.
- **Backend (a) cloud:** launcher + deep-link out to codex/claude. Concerto
  owns launch + the link, not the live state. Be deliberate that this is
  shallow.
- **Backend (b) hosted lfd:** full dashboard — lfd owns the session, so show
  iteration count, current task, blocks, metrics from `goal/` targets.
- **Blocks surface** — the "queue of decisions needed" lands here and/or in
  Asana.
- **Sequence with `3-wave-repo-split`.** Its build order guts `ContentView` to a
  repo-filtered wave list and grows the per-repo streams up from there. This
  session list is the layer that grows on top of that skeleton — don't build the
  dashboard against the old per-repo `Wave` shape if the identity/RepoWork split
  is landing alongside it.

## Done when

- A repo with one cloud Goal and one hosted-lfd Goal shows both: the cloud one
  as launch+link, the hosted one as a live dashboard.
