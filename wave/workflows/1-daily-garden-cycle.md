---
asana_id: '1214270115637604'
---
# Daily garden cycle

**Finish line:** The root wave runs a scheduled garden pass that observes its member waves (`scan` → `assess`) and proposes mutations via `mutate`, producing a reviewable PR each cycle. Runs autonomously on cron; also available on demand via `lf garden`.

## Context

Engine pieces exist: `s5-scan` through `s2-assess` as builtins, four `govern-*` flows, the `garden` flow (`scan` → `assess` → `xor(garden-act, silence)`), and the `garden-act` flow (`mutate` → `review`). Algedonic signals route through the attention queue.

What's missing:

- **Per-wave audit (`vsm-flow`)** — single-pass s5→s2 composed as one flow producing one PR per garden wave
- **Tree traversal (`planning-flow`)** — a garden wave walks its member tree: leaves→root information gathering, root→leaves policy cascade
- **Typed, reversible mutation** — `mutate` grows a structured mutation API: logged before/after, one-command revert, per-lever validation (direction / area / flow / items / agent / triggers / lifecycle)
- **Scheduled operation** — cron entry on the root wave runs garden automatically, not only on manual invocation

## Daily experience

Morning: open Concerto. One new PR from root overnight, proposing 2–3 mutations to member waves (“promote `pm-round-trip` because it is blocking the build loop,” “split `governance-surfaces` if the scope keeps sprawling”). Read, approve or adjust, land. Root observed, proposed, and stayed reviewable.

## Done when

- `lf garden` runs end to end on this repo and produces a reviewable mutation PR against `main`
- The same flow runs on cron without manual trigger
- Each mutation is logged with rationale, structured before/after, and one-command revert
- Nested gardening waves work — a root wave with child waves that themselves garden can traverse children too
