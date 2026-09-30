---
requires: intended change and an implementation plan
produces: working implementation, focused proof, and an updated plan
action_style: procedural
---
Turn the design doc into working code.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- Read wave/PM context only when the seed names the exact wave, task, project,
  or a concrete coordination question; never infer it or repair access as a
  prerequisite.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Goal

Working code with rough edges beats perfect code that took too long.

Produce a working change, resolve reversible ambiguity, and verify it. Preserve the intended outcome across internal slices; a first draft does not satisfy unfinished acceptance.

## Workflow

Use the supplied design and repository conventions. A small change may have its
plan in the conversation; do not require a document template or a prior skill.

1. **Understand the design**
   Recover the intended outcome, accepted constraints, approach, and proof.
   Read the current plan wherever it lives. Reconstruct the affected concepts,
   owners, persistence, and call paths before choosing where behavior belongs.
   Read or add the plan's **Delete — do not maintain** list: concrete files/symbols
   and their exclusive tests/fixtures slated for removal, with required behavior,
   data, and proof to preserve. Keep it current across passes.

2. **Implement**
   - Make the deepest planned deletions first: remove obsolete concepts,
     authorities, and paths, then build data structures on what remains.
   - Never repair, refactor, or extend a deletion target or its exclusive
     tests/fixtures. When one fails, carry out the planned removal instead.
     Preserve coverage of required behavior on the surviving path.
   - Move real consumers end to end and include any required data migration in
     the deletion cut. Temporary compile/test breakage within the cut is no
     reason to repair the predecessor; finish the cut before claiming proof.
     New capabilities need no invented predecessor or deletion quota.
   - Functions one at a time, following the signatures
   - Match existing patterns in the codebase
   - Reshape the existing owner instead of adding a parallel representation
   - Follow the design's delivery boundary. An indivisible architectural change
     proceeds in coherent internal slices but ships as one PR. Keep the complete
     target and update the remaining work as implementation teaches us more.
     Do not stage the landing with flags, v2s, or setups nothing uses yet.

3. **Verify**
   - Run the smallest behavioral test that proves the behavior you changed
   - Run the "done when" check from the design doc
   - If a required proof cannot run, stop dependent work and record the exact
     command and blocker. "Authored, not executed" is not a completed pass.
   - Update the working plan in place with remaining work, consequential
     discoveries, and proof results or links. Delete stale instructions; retain
     accepted requirements and unresolved counterexamples. Leave a brief summary
     of progress, learning, and proof when useful for the handoff; avoid
     duplicating the plan in a report for each pass.
   - Do not run an affected-suite or full-repository gate here; gate and CI own
     those broader proofs

## Rules

**Match existing patterns.** Find similar code nearby and match its style. If the codebase uses `@dataclass`, use `@dataclass`. If it uses type hints, use type hints.

**Stay in scope.** Implement exactly what the design describes. Scope creep goes in `scratch/questions.md`, not the code.

**One concept, one authority.** A new type must represent a new real-world
concept. Legacy/New enums, v2 types, adapters, fallbacks, dual writes,
compatibility shims, and parallel stores are blocking by default. Use one only
when the reviewed design explicitly authorizes it and names its deletion point.

**Tests prove it works.** Add tests for user-visible behavior. Don't test implementation details. Assert on results, not mock calls.

## Task context

When a Task is supplied, use its directive, accepted design and included Steers.
Stay in its supplied worktree and preserve the active writer, selected Flow,
and review boundaries. Do not select backlog work, create a second Task or
launch a competing implementation. A failed planning read is a named gap;
continue independent work from the supplied evidence without repairing auth.
Publication, landing and navigation belong to the caller's explicit steps.

While building feature work, notice signals that could help the Wave steer.
Name the outcome, candidate measure, decision value and cheapest credible
producer. Add a useful instrument when it fits the coherent change;
otherwise leave the proposal for Wave sponsorship.
Metric proposals are discoveries, not a completion quota.

## Wave context

If `<lf:wave>` is present, check `wave/<wave>/GOAL.md` and `MEMORY.md` in docs:

- Follow the wave's intent and principles during implementation
- Respect decisions and constraints recorded in `MEMORY.md`
- Note drift from wave constraints in `scratch/questions.md`

## When the design is wrong

If the design doc is unclear, make the simplest reversible choice and record it
in `scratch/questions.md`.

If implementation reveals a counterexample that invalidates the slice,
authority model, deletion path, or full-design trajectory, stop dependent work
and revise the design or return to review. Never note an architectural
contradiction and keep building on it.

## Adaptation

If you had to discover a convention that wasn't documented — error handling pattern, test structure, naming style, import conventions — add it to the repo's style guide (CLAUDE.md, STYLE.md) so the next session doesn't have to rediscover it.
