---
requires: a large implemented change on this branch
produces: multiple shippable PRs
action_style: procedural
---
Break a large working change into discrete shippable parts—each an honest, substantial diff that lands on its own.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — `scratch/<branch>.md` is the design behind the big
  change; it tells you which parts were one idea and which merely shared a
  branch.
- Run `git diff main --stat` to see the change's real footprint.
- Read the repo's agent doc (`AGENTS.md`) for conventions.

## When this skill applies

This is the remedial path. Additive work should be sliced into tasks at plan
time (`lf launch-plan` or the design session), each increment shipping on its
own from the start. Reach for this skill when divisibility showed up only
after the code existed, or when one branch quietly accumulated several
independent ideas.

## Find the seams

Decompose along boundaries that already exist in the work, in order of
preference:

1. **Independent ideas.** Changes that shipped together only because they
   were built together. These split cleanly and can land in any order.
2. **Divisible impact.** One idea whose parts divide cleanly—by user-behavior
   impact, by files touched, by layer. These land as an ordered series,
   foundation first.

Each slice should be substantial: a real diff a reviewer holds in their head
as one thing. Prefer three meaty PRs over ten confetti ones. If no clean
seams exist—the parts only make sense together—ship it whole. An
architectural change built in internal slices against its design still lands
as one PR; one large honest PR beats an artificially staged series.

## Rules of an honest slice

- **Every slice leaves main shippable.** It builds, affected tests pass, and
  no user-visible behavior is half-changed.
- **Dead code may land ahead of its wiring.** A foundation slice can check in
  code the next slice connects. Mark it so it reads as intentional (in Rust,
  `#[allow(dead_code)]` with a one-line reason), and make sure the wiring
  slice follows promptly.
- **No scaffolding to make the split possible.** Never introduce a feature
  flag, adapter, compat shim, or `v2_`/parallel implementation whose only
  purpose is staging the landing. If a boundary needs a shim, it's the wrong
  boundary—move it.
- **Remove the old in the slice that creates the new.** A slice that adds a
  replacement deletes what it replaces; the repo never holds both. Git keeps
  the history.

## Mechanics

1. Order the slices foundation-first and name each one—the branch name
   becomes the PR title prefix.
2. Give every independently deliverable PR its own Task. A Task has zero or
   one PR, never a serial chain. Reuse existing Tasks for matching scope;
   preserve the original Task's intended outcome when assigning its slice.
   Independent Tasks start from main; dependent Tasks stack on their parent.
   A published parent PR, including a draft, is enough to start and publish a
   child before the parent merges; only landing waits for that dependency.
3. Preserve the original change with `lf commit` before moving work. Use
   Loopflow for checkout and Git mutations; transfer the existing code into
   each slice without rewriting it or losing mixed hunks. Keep the source
   available until every intended change is accounted for.
4. Prepare each child's self-contained design excerpt: its own brief, accepted
   scope, dependencies and done-when. Transfer it before launching with
   `lf checkout <child> --stack-on <parent> --design <path>`. The command records
   source identity and content and preserves newer child work. Retry identical
   input; a changed handoff at an occupied destination is a conflict to reconcile,
   never permission to overwrite. Do not copy a whole plan and toggle a current
   slice marker. Later parent scratch does not replace the child's working design.
5. Verify each slice standalone—build and affected tests on that branch, not
   on the union.
6. Publish each child with `lf pr publish` without waiting for its parent to
   merge. Use `lf sync` for parent code updates and after its squash merge;
   the child retains its Task, branch, PR, Session and design while targeting main.
   Land authorized work through `ship`: gate, waited landing, then follow-through.
   Preserve accepted unverified outcomes in PR copy before scratch cleanup.
   After authoritative merge, file linked follow-ups or record none needed, then
   confirm completion. If finishing stopped, inspect live work and receipts before
   running `finish-delivery`; never re-land merely to finish filing.
   Retire a superseded original PR only after its code and intended outcome have
   owners and the caller's authority covers abandonment; preserve unrelated work.
