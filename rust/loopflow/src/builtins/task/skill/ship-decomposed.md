---
requires: a large implemented change on this branch
produces: multiple shippable PRs
action_style: procedural
---
Break a large working change into shippable parts, each an honest, substantial diff that lands on its own.

Read `scratch/<branch>.md` first: the design shows which parts were one idea and
which only shared a branch. `git diff main --stat` shows the real footprint.

This is the remedial path. Additive work should be split into Tasks at plan
time. Use this when the seams showed up only after the code existed, or when one
branch quietly collected several independent ideas.

## Find the seams

Split along seams already in the work, in this order:

1. **Independent ideas.** Changes that ship together only because they were
   built together. These split cleanly and land in any order.
2. **Divisible impact.** One idea whose parts divide cleanly, by user-facing
   behavior, files or layer. These land as an ordered series, foundation first.

Each slice is a real diff a reviewer holds in their head as one thing. Three
meaty PRs beat ten confetti ones. If the parts only make sense together, ship it
whole. An architectural change built in internal slices still lands as one PR;
one large honest PR beats an artificially staged series.

## Rules of an honest slice

- **Every slice leaves main shippable.** It builds, affected tests pass, and no
  user-visible behavior is half-changed.
- **Dead code may land ahead of its wiring.** A foundation slice can add code the
  next slice connects. Mark it as intentional (in Rust, `#[allow(dead_code)]`
  with a one-line reason), and land the wiring slice promptly.
- **No scaffolding to make the split possible.** No feature flag, adapter,
  compat shim or `v2_` copy whose only job is staging the landing. If a seam
  needs a shim, it's the wrong seam.
- **Remove the old in the slice that adds the new.** The repo never holds both.

## Mechanics

1. Order the slices foundation first and name each one; the branch name becomes
   the PR title prefix.
2. Without a Task, stay without one: `lf wt create` for each branch, and publish
   each PR with no Tasks. Use Tasks only when splitting an existing Task or when
   managed work is asked for. Then give each independently deliverable outcome
   its own Task, reuse existing Tasks for matching scope, and keep the original
   Task's intended outcome with its slice. Independent Tasks start from main;
   dependent Tasks stack on their parent. The parent's branch is enough to
   start and publish a child; only landing waits for the parent.
3. Save the original change with `lf commit` before moving anything. Use
   Loopflow for checkouts and Git changes; carry the existing code into each
   slice without rewriting it or losing mixed hunks. Keep the source until every
   change has a home.
4. Write each child its own design excerpt: brief, accepted scope, dependencies
   and done-when. Without Tasks, copy it into the child's worktree. With Tasks,
   pass it before launch with `lf checkout <child> --stack-on <parent> --design
   <path>`, which records where it came from and keeps newer child work. Retry
   with identical input; a different design at an occupied destination is a
   conflict to reconcile, never permission to overwrite. Don't copy a whole plan
   and toggle a "current slice" marker. Later parent scratch doesn't replace the
   child's design.
5. Verify each slice on its own branch: build and affected tests there, not on
   the union.
6. Publish each child with `lf pr publish` without waiting for its parent to
   merge. `lf sync` picks up parent changes and, after the parent's squash
   merge, retargets main; the child keeps its Task, branch, PR, Session and
   design. Land approved work through `ship`: gate, then a waited landing.
   Reaching the end of the Task's graph requests completion. Put accepted but
   unverified outcomes in the PR description before scratch is cleared. Later
   work is an ordinary new Task. If finishing stops after merge, check live work
   and retry completion alone. Close a superseded original PR only once its code
   and outcome live in other PRs and closing it is within what you were asked to
   do; keep unrelated work.
