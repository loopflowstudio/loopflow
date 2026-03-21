---
asana_id: '1213718096106716'
linear_id: 0f0f48cb-741f-4746-8e75-76113f00b058
---
# 01: Interactive Checkpoints

**Finish line:** Every build and garden checkpoint that needs a human appears in the attention queue through one `interactive` contract, with typed detail for `review-design` and `wave/review` instead of raw JSON or implicit waiting state.

## Context

The queue home screen and interactive session handoff already exist. `lfd` can mark a run waiting, create a tracked terminal session, and Swift can route that waiting wave into the right session or workspace surface. The shared model is intentionally coarse now: Rust and Swift normalize `design_review`, `code_review`, and `calibration` into the single `interactive` path, with step-specific behavior hanging off `context.step`.

That coarse model is the right direction. The remaining gap is end-to-end coverage and detail. `review-design` and the garden-side `wave/review` checkpoint still need explicit creation rules, typed queue detail, and resolution semantics that prove the queue can carry both build and garden human checkpoints without growing a new enum case for every step.

## What to build

1. **Checkpoint creation.** When `review-design` or `wave/review` waits, emit or update an interactive attention item with stable IDs and structured context (`step`, `design_path`, `terminal_session_id`, mutation summary if needed).

2. **Step-scoped detail surfaces.** Teach the queue to render dedicated detail for `review-design` and `wave/review` keyed off `context.step`, while keeping the top-level kind coarse. The queue should explain what decision is needed, not dump raw context.

3. **Resolution and lifecycle.** Opening a session, finishing a design review, or completing a wave review should drive viewed/resolved transitions consistently with run progression. Waiting state, terminal session state, and attention state should agree.

4. **Proof through tests.** Add Rust and Swift coverage showing these checkpoints are created, decoded, rendered, and resolved end to end.

## Done when

- `review-design` and `wave/review` surface interactive queue items with typed detail
- Normal queue usage does not fall back to raw JSON for these checkpoints
- Viewed and resolved transitions follow the underlying run/session lifecycle
- A conductor can handle both build and garden checkpoints from the queue without drilling into logs first
