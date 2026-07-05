# Open questions / blockers

## Roadmap reconciliation deferred — Asana token expired (2026-07-05)

`lf op pm show` fails: "Stored asana token has expired. Run `lf op auth asana`
again." This is a headless run — no one to re-auth — so the wave's Asana roadmap
could not be reconciled against reality this pass. The durable to-do is recorded
in `wave/goals/MEMORY.md` ("Next"). When re-authed, reconcile:

- **Close shipped tasks:** #780 (Asana `--pr` link, merged), #781 (wave
  ancestry, merged), #796 (reactive server, merged), #801 (realignment +
  lfd/lfq/lfdb collapse, merged).
- **File this branch:** #803 — M1/M2 compression (drop postgres, retire `lf q`,
  hoist harness out of lfd, cut docker/ServiceManager, drop the wave `mode`
  knob). Executes a large chunk of the M1/M2 staging debt in
  `wave/goals/architecture-direction.md`.
- **File the forward work** from the two durable design docs beside MEMORY.md:
  the M1 conversion work-list (`architecture-direction.md`) and the remaining
  open decision questions (`wave-agent-follow-ups.md`, #1 now resolved by #803).
- **Track:** #799 (greenfield CLI probe) still open — one of the three
  reference builds; server + mobile remain.

Assumption made: with no live `goals` wave server (`lf memory update` reports
"no live server"), MEMORY.md was refreshed by direct file edit — the sanctioned
fallback when no server holds the pen.
