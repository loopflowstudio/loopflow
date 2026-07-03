---
priority: high
---

# Wave ancestry & chord structure

**Finish line:** The durable `Wave` type carries its parent/child relation again,
so `WaveAgentTree.child_waves` is populated, a chord's contents are just its
children, and a two-repo chord runs with one child Looping Agent per repo.

## Context

The wave/chord data model is already self-referential: chords were once
`chords` + `chord_members` tables (migration 011), dropped and folded into
`waves` via `parent_wave_id` / `wave_type` / `position` (migrations 013, 028).
"Member" is purged from the wave-structure code — it survives only in immutable
migration SQL history, which is fine. See DECISIONS.md 2026-06-30.

**What changed:** the Wave/Run/Session runtime reduction (this branch) collapsed
the durable model to three nouns but **dropped the parent-wave field off the
`Wave` type in the process.** The store schema still has `parent_wave_id`
columns, but the domain `Wave` no longer surfaces ancestry, so
`WaveAgentTreeDto.child_waves` is built empty and `WaveAgentTree` returns only
root sessions and parent-session edges. The tree cannot show child waves at all.

This is the leading edge now. The entire goals wave is a **chord — a wave whose
members are waves** — and cross-repo Goals are defined as *a chord whose children
live in different repos*. Neither can be observed until ancestry is back on the
durable type.

## What to shape

- **Reintroduce Wave ancestry on the durable type.** Surface `parent_wave_id`
  (the store already has the column) on `Wave`, and rebuild the parent/child
  query the tree needs. A chord is a wave with `wave_type = chord`; its contents
  are `children where parent_wave_id = id`. No separate chord entity, no
  denormalized ancestry cache — query parents at runtime (the WaveAgentTree
  design already tolerates flexible ancestry).
- **Populate `WaveAgentTree.child_waves`** from that relation so Concerto and
  agents can see the chord structure and child Looping Agents.
- **Keep vocabulary parent / child / sibling** in the wave-structure domain;
  don't let "member" bleed back in from VSM/govern.
- **Cross-repo Goals fall out for free:** a cross-repo Goal is a chord whose
  children live in different repos; the parent spans whatever its children span.
  `wave.repo` stays single on each leaf.
- **Open:** whether a single *leaf* Looping Agent may span repos directly (many
  worktrees, coordinated cross-repo PRs) for tightly-coupled changes. Default:
  no — chord-spanning only — until atomicity proves necessary. Item
  `3-wave-repo-split` is the concrete `repos: [RepoWork]` design for that open
  question; the two items are the two answers to cross-repo Goals. Land ancestry
  first — it unblocks the chord model regardless of how the fork resolves.

## Done when

- `Wave` exposes its parent relation; chord contents query = children where
  `parent_wave_id = id`.
- `WaveAgentTree` returns child waves (not an empty list) for a chord.
- No "member" in the wave-structure code.
- A two-repo chord runs with a child Looping Agent per repo.
- `cargo test` passes.
