---
asana_id: '1214270115637604'
---
# Daily garden cycle

**Finish line:** The root chord runs a scheduled garden pass that observes its member waves (`garden/scan` → `garden/assess`) and proposes mutations via `wave/mutate`, producing a reviewable PR each cycle. Runs autonomously on cron; also available on demand via `lf garden`.

## Context

Engine pieces exist: `vsm/s5-scan` through `vsm/s2-assess` as builtins, four `govern-*` flows, `garden/scan` → `garden/assess` → `wave/mutate` → `wave/review`, and `wave/mutate` as a shipped step that edits wave YAML and items. Algedonic signals route through the attention queue.

What's missing:

- **Per-chord audit (`vsm-flow`)** — single-pass s5→s2 composed as one flow producing one PR per chord
- **Tree traversal (`planning-flow`)** — chord walks its member tree: leaves→root info gathering, root→leaves policy cascade. Composes `vsm-flow` at each level.
- **Typed, reversible wave mutation** — `wave/mutate` grows a structured mutation API: logged before/after, one-command revert, per-lever validation (direction / area / flow / items / agent / triggers / lifecycle)
- **Scheduled operation** — cron entry on the chord-wave runs garden automatically, not only on manual invocation

## Daily experience

Morning: open Concerto. One new PR from the chord-wave overnight, proposing 2–3 mutations to member waves ("promote `pm-round-trip` because it is blocking the build loop," "split `governance-surfaces` if the scope keeps sprawling"). Read, approve or adjust, land. The chord observed, proposed, and stayed reviewable.

## Done when

- `lf garden` runs end-to-end on this repo and produces a reviewable mutation PR against `main`
- The same flow runs on cron without manual trigger
- Each mutation is logged with rationale, structured before/after, and one-command revert
- Nested chords work — a chord of chords traverses children too
