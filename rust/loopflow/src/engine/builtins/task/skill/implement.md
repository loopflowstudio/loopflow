---
requires: scratch/<branch>.md
produces: code, tests
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

Produce a first draft quickly. Polish cleans it up. You can be re-invoked if needed. Don't block on ambiguity—make the simplest choice and keep moving.

## Workflow

The design doc and style guides are in your context.

1. **Understand the design**
   The design doc has data structures, function signatures, constraints, a
   "done when" check, the complete target architecture, and one marked current
   slice. Reconstruct the current concepts, authorities, writers, persistence,
   and call paths before choosing where the behavior belongs.

2. **Implement**
   - Data structures first—get the core types right
   - Functions one at a time, following the signatures
   - Match existing patterns in the codebase
   - Reshape the existing owner instead of adding a parallel representation
   - Delete the authority or path the design makes obsolete
   - A large design proceeds in slices—one coherent piece at a time, each
     checked against both its focused proof and the full-design trajectory—but
     the branch ships as one PR. Update only `This slice` and the slice ledger;
     never replace the complete design with a local implementation plan. Don't
     stage the landing with flags, v2s, or setups nothing uses yet.

3. **Verify**
   - Run the smallest behavioral test that proves the behavior you changed
   - Run the "done when" check from the design doc
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
and revise the design or return to human review. Never note an architectural
contradiction and keep building on it.

## Adaptation

If you had to discover a convention that wasn't documented — error handling pattern, test structure, naming style, import conventions — add it to the repo's style guide (CLAUDE.md, STYLE.md) so the next session doesn't have to rediscover it.
