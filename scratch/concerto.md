# Concerto wave viewer — forward map

Design, decisions, and the `lf`/lfd/pubsub architecture from this branch are
folded into `wave/concerto/MEMORY.md` (Charter model, Wave ontology & viewer,
Swift data path, the spine). This file keeps only the reviewer's forward map.

## Try it (slice 1, shipped this branch)

Launch Concerto against this repo, select the `concerto` wave: the detail pane
renders the Objective from `GOAL.md` and the five projects from `projects/*.md`
beside the live WaveChat surface. (Full commands in `pr-body.md`.)

## Remaining slices

2. **The ledger.** A `lf` runs query over lfdb (daemon-less pull) renders the
   wave's `Run` records as a plain minimal list (NOT the origin-grouped
   chart — see the runs-are-secondary decision below); the bundled lfd's pubsub
   pushes live new-run updates; live rows attach as sessions.
3. **`lf` query surface for the aim + plan** — promote slice 1's direct file
   read to a `lf wave show` query so remote (lfd-as-proxy) works, not just local.
4. **Tasks from Linear** — surface the wave's project issues via `lf op pm`,
   render under the project. Blocked on flowloop R1: a wave has one
   `linear_project`, so tasks are one flat bucket, not yet per-`projects/*.md`.

## Locked decisions carried into slice 2+

- **Runs UI stays simple.** Runs is not the centerpiece — the plan (objective +
  projects) is. A plain runs list is enough; don't grow `lf runs` for
  origin-grouped data now.
- **RunStatus biases to `lf`** — its lowercase tokens, no invented `cancelled`,
  unknown status is loud not `?? .pending`. (In MEMORY.)
