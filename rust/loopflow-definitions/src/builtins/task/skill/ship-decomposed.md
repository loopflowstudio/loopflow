---
requires: a large implemented change on this branch
produces: multiple shippable PRs
action_style: procedural
---
Break a large working change into shippable parts, each an honest, substantial diff that lands on its own.

This is the remedial path. Additive work should be split into Tasks at plan
time; use this when the seams showed up only after the code existed, or when
one branch quietly collected several ideas. Read the design first: it shows
which parts were one idea and which only shared a branch. `git diff main
--stat` shows the real footprint.

## Find the seams

1. **Independent ideas.** Changes that ship together only because they were
   built together. These land in any order.
2. **Divisible impact.** One idea whose parts divide by user-facing behavior,
   files or layer. These land as an ordered series, foundation first.

Each slice is a real diff a reviewer holds in their head as one thing. Three
meaty PRs beat ten confetti ones. If the parts only make sense together, ship
it whole: one large honest PR beats an artificially staged series.

## An honest slice

- **Leaves main shippable.** It builds, its tests pass, and no user-visible
  behavior is half-changed.
- **No scaffolding to make the split possible.** No flag, adapter, shim or
  `v2_` copy whose only job is staging the landing. A seam that needs a shim is
  the wrong seam.
- **Removes the old where it adds the new.** The repository never holds both.
- **May land dead code ahead of its wiring,** marked as intentional, with the
  wiring slice close behind.

## Mechanics

1. Order the slices foundation first and name each.
2. Save the original with `lf commit` before moving anything, and keep the
   source until every change has a home. Carry the existing code into each
   slice; don't rewrite it.
3. Without a Task, stay without one: `lf wt create` per branch. When splitting
   a Task, give each outcome its own, reusing Tasks that match. Independent
   slices start from main; dependent ones stack on their parent with
   `lf checkout <child> --stack-on <parent> --design <path>`.
4. Write each child its own design excerpt: brief, scope, dependencies and
   done-when. Never copy the whole plan and toggle a "current slice" marker.
5. Verify each slice on its own branch, not on the union.
6. Publish each with `lf pr publish`; a child doesn't wait for its parent to
   merge, and `lf sync` retargets it afterwards. Close a superseded original
   PR only once its code and outcome live in other PRs.
