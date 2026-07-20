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
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

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
2. Build each slice as its own branch: independent slices branch from main;
   dependent slices stack on the previous slice's branch and rebase forward
   as predecessors land.
3. Move work with git—`git cherry-pick` for clean commits,
   `git checkout -p <big-branch> -- <paths>` to carve hunks out of mixed
   ones. Don't rewrite code by hand that already exists on the big branch.
4. Give each slice a short `scratch/<branch>.md`: what this slice ships, what
   it deliberately leaves dead, and its done-when.
5. Verify each slice standalone—build and affected tests on that branch, not
   on the union.
6. Land the series with the PR lifecycle: `lf pr publish` for headless
   creation, `lf pr land` to arm auto-merge, next slice rebases and repeats.
   Once the series is fully landed, retire the original big branch
   (`lf pr abandon` if it had a PR, otherwise delete it).
