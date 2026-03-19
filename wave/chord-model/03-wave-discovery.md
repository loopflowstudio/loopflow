---
linear_id: e4bced9c-17a1-4cde-bf4a-ea8496e00bc5
---
# 03: Wave Discovery and Root Chord

**Finish line:** lfd discovers waves from `wave/` on disk, reconciles against the store, and runs them. Concerto auto-creates a root chord on launch with membership derived from discovered waves.

## What to build

1. **Disk scanner** — on startup and periodically, scan `wave/` for YAML configs. Reconcile against waves in the store: create new, update changed, mark removed.

2. **Root chord auto-creation** — when Concerto launches (or on first `lfq` command), create the root chord-wave if it doesn't exist. Its `area` includes all discovered `wave/<name>/` entries. Its initial flow is `tend` until the dedicated governance-wave structure exists.

3. **Owner filtering** — eventually, filter discovered waves by `owner` field in the YAML. Initially, run everything.

4. **Reconciliation** — handle the cases: wave YAML added to disk, wave YAML removed from disk, wave YAML changed on disk. Don't destroy runtime state (runs, logs, algedonic history) when reconciling.

## Done when

- lfd discovers wave configs from `wave/` and creates waves in the store
- Root chord exists with correct membership after Concerto launch
- Adding a new `wave/<name>/<name>.yaml` to disk creates the wave on next scan
- Removing a wave YAML marks it inactive without destroying history
