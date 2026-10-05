---
requires: none
produces: scratch/<branch>.md
action_style: exploratory
---
Help the User dream big, detail the idea, then shape one exact design artifact.

## Task and design

Treat the Task as the user's problem and desired experience. Explore possible
solutions before choosing one; distinguish observations, proposed mechanisms,
real constraints, and accepted decisions. The design owns architecture,
implementation sequencing, and acceptance checks. A Task need not arrive with an
implementation plan. Preserve its original problem when the solution changes.

Continue an existing design and investigate its material gaps; a newly filed
Task does not require restarting discovery. Keep accepted decisions binding
and linked, preserve draft status and open questions, and honor the selected
Flow. Launching a draft does not approve it.

## Orientation

- Read `scratch/` and the repo agent guide. Continue an existing design instead of re-deriving it.
- Read the active Wave's `GOAL.md` and `MEMORY.md` only when placement is part of the design and the seed names that exact Wave. Use `lf wave status <wave>` only when its chapter state is material; never infer a Wave or repair PM access as a prerequisite.
- Write the design to `scratch/<workspace-slug>.md`. Put unresolved assumptions in `scratch/questions.md`.

## Surface

**Interactive:** work in the current conversation. Discover what they want to build and show the exact final artifact.

**Headless:** infer intent from the Task directive, quoted User language, and parent evidence. Write the best complete draft without claiming User confirmation. Record genuine ambiguity in `scratch/questions.md`.

## Workflow

### 1. Dream

Let the full idea emerge before applying scope pressure. Follow what is surprising. Capture User language that anchors intent, constraints, or priority verbatim in the design.

### 2. Detail

Make the behavior concrete: data structures, public functions, interactions, edge cases, authority boundaries, failure recovery, and the real demo. Write as the design develops so a crashed session loses no decisions.

Classify the shape:

- **Additive series:** identify independently valuable increments. Keep later increments at intent level; each gets its own design when launched.
- **Indivisible change:** detail the full architecture and explicitly state
  that implementation proceeds in internal slices but ships as one PR. Keep
  the target architecture, integration/deletion path, forbidden near-misses,
  and acceptance conditions intact while `This slice` moves.

Sequence the deepest cuts first: remove obsolete concepts, authorities, and
paths before building on what remains. Mark concrete files/symbols and their
exclusive tests/fixtures **Delete — do not maintain** in the plan so later
passes never repair code scheduled for removal. Name required behavior, data,
and tests that must survive. If removal needs a consumer cutover or migration,
include that minimum work in the same cut; do not first modernize the old path.
Additive work needs no invented deletion.
Plan one schema migration per Task, written against the last released schema
and edited in place until landing; a design never schedules intermediate schemas.

### 3. Size-check

A design beyond roughly 1,000 words or an implementation beyond roughly 1,000 lines is a signal, not an automatic split. Split additive work into a keystone plus follow-ups; keep an indivisible architectural change whole. Do not create follow-up Tasks here.

### 4. Place

Choose exactly one Wave by matching its objective and bounds. The current chapter resolves automatically. If no Wave fits, record unresolved placement instead of inventing ownership. If a state lookup fails, report the dated command and failure in evidence notes; keep ambient Home state out of the design.

Tighten the artifact to:

- **What to build** — one sentence describing the new end state.
- **Placement** — Wave, or the recorded unresolved placement.
- **The demo** — the real action and observable result.
- **Data structures** — core domain values.
- **Key functions** — signatures and intent.
- **Constraints** — choices that would force a rewrite if guessed wrong.
- **Done when** — headless gate command and expected outcome.
- **Current system** — concepts, authorities, writers, and paths that the change
  reshapes or deletes.
- **Delete — do not maintain** — concrete removal targets, their exclusive
  tests/fixtures, and any cutover dependencies; keep this list current as slices
  finish or the design changes.
- **Forbidden outcomes** — duplicate representations, compatibility layers, or
  locally passing states that still violate the intended architecture.
- **Internal slices** — deepest deletions first, then build on the surviving
  structure; one coherent cut marked `This slice`. Keep
  remaining work current without a pass ledger.
- **Measure** — only when a meaningful before/after quantity exists.

For an additive series, describe the keystone fully and list the intended follow-ups precisely enough for `launch-plan` to encode. Do not file them yet. Before finishing, reread the artifact and present the consequential scope, keystone boundary, follow-ups, and open assumptions.

When filing or editing a Task, keep its description to the current problem,
desired outcome, observable acceptance, and real constraints. Put dated planning
and execution updates in Task comments, with links to detailed evidence.
Comments may be collapsed: keep current blockers, dependencies, and accepted scope
visible in the description. Reconcile changed scope instead of appending amendments.

## Checks run unattended

Specify headless acceptance commands and expected results for gate to run once.
Desktop uses app builds and view/interaction tests. Never require a person,
display session or permission dialog. Defer unavailable checks to capable CI
without blocking earlier steps. Put judgment in demo/review. Implement/compress
only build and run focused tests; scratch keeps one command/result line.

## Handoff

Keep named, dated decisions, draft/accepted status and remaining work in the
plan. Record one check-result line. Omit session instructions and ambient Home
facts; the plan must not direct its next reader. Keep transcripts separate and
historical skill names unprefixed.

Leave the current design and its evidence ready for kickoff to shape into an
implementation plan. Task allocation and launch belong to launch-plan when
execution is requested. Preserve accepted intent and unresolved choices in the
artifact; finishing design does not itself accountorize launching work.
