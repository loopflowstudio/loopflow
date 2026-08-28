---
requires: none
produces: scratch/<branch>.md
action_style: exploratory
---
Help the User dream big, detail the idea, then shape one exact design artifact.

## Orientation

- Read `scratch/` and the repo agent guide. Continue an existing design instead of re-deriving it.
- Read the active Wave's `GOAL.md` and `MEMORY.md` only when placement is part of the design and the seed names that exact Wave. Use `lf pm show --wave <wave>` only when its Project state is material; never infer a Wave or repair PM access as a prerequisite.
- Write the design to `scratch/<workspace-slug>.md`. Put unresolved assumptions in `scratch/questions.md`.

## Surface

**Human present:** work in the current conversation. Discover what they want to build and show the exact final artifact.

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
  and full proof intact while `This slice` moves.

### 3. Size-check

A design beyond roughly 1,000 words or an implementation beyond roughly 1,000 lines is a signal, not an automatic split. Split additive work into a keystone plus follow-ups; keep an indivisible architectural change whole. Do not create follow-up Tasks here.

### 4. Place

Choose exactly one Wave by matching its objective and bounds. Choose one existing Project when the available evidence supports it; if none fits or PM state is unavailable, record that exact ambiguity instead of inventing ownership.

Tighten the artifact to:

- **What to build** — one sentence describing the new end state.
- **Placement** — Wave and Project, or the recorded unresolved placement.
- **The demo** — the real action and observable result.
- **Data structures** — core domain values.
- **Key functions** — signatures and intent.
- **Constraints** — choices that would force a rewrite if guessed wrong.
- **Done when** — focused behavioral proof and expected outcome.
- **Current system** — concepts, authorities, writers, and paths that the change
  reshapes or deletes.
- **Forbidden outcomes** — duplicate representations, compatibility layers, or
  locally passing states that still violate the intended architecture.
- **Internal slices** — ordered coherent cuts, one marked `This slice`, plus a
  durable evidence ledger that never replaces the full design.
- **Measure** — only when a meaningful before/after quantity exists.

For an additive series, describe the keystone fully and list the intended follow-ups precisely enough for `launch-plan` to encode. Do not file them yet. Before finishing, reread the artifact and present the consequential scope, keystone boundary, follow-ups, and open assumptions.
