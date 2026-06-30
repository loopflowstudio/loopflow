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

## What to shape

- **Per-repo looping session list** — one entry per active Wave's Looping Agent.
- **Backend (a) cloud:** launcher + deep-link out to codex/claude. Concerto
  owns launch + the link, not the live state. Be deliberate that this is
  shallow.
- **Backend (b) hosted lfd:** full dashboard — lfd owns the session, so show
  iteration count, current task, blocks, metrics from `goal/` targets.
- **Blocks surface** — the "queue of decisions needed" lands here and/or in
  Asana.

## Done when

- A repo with one cloud Goal and one hosted-lfd Goal shows both: the cloud one
  as launch+link, the hosted one as a live dashboard.
