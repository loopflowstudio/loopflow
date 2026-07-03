---
priority: low
asana_id: '1216257604423395'
---
# Looping Agent — hosted lfd + Ghostty backend (b)

**Finish line:** Hosted lfd owns a long-lived per-wave looping session, run in an embedded Ghostty terminal, with full state visible in Concerto.

## Context

Backend (b) is **second** — ours to own and support well, after we've adapted to codex/claude (backend a). This is where the real persistent-agent engineering lives, built on the systems wave's hosted lfd.

## What to shape

- **Persistence model (the core fork):**
  - *Long-lived agent* — one process per wave, the loop lives inside the session. Matches "24/7 master loop" exactly; cost: unbounded context growth, crash drops the session, commit/PR boundaries from inside a never-ending session.
  - *Threaded ticker* — keep cold-spawned runs underneath but thread memory across iterations. Safer (recoverable, bounded), less "one master loop."
- **Embedded Ghostty** session hosting (the lfd CLI session surface).
- **Full state to Concerto** — lfd owns it, so expose everything.

## Done when

- A hosted-lfd Goal loops unattended for ≥20 iterations, survives a restart, and streams full state to Concerto.
