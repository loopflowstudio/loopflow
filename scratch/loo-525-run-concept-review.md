# LOO-525: concept-review with its own loop in pursue

Status: accepted direction (Jack, 2026-10-19); implemented 2026-10-19.

## Problem

`pursue` publishes as soon as the implement loop advances. Nothing steps back
to ask whether the concepts and interactions the change introduces are the
right ones before a PR exists. Jack decided the builtin pursue Flow runs
`concept-review` after the implement loop, with its own `loop-or-next`, so a
review that changes the design iterates back to implement before publication:

```yaml
- implement
- compress
- flow: refresh
- loop: implement
- concept-review
- loop: implement
- pr-publish
```

Jack asked for this to run as a fully autonomous infra Task.

## Demo

```
lf flow show pursue
```

lists `implement, compress, sync, realign, loop-or-next, concept-review,
loop-or-next, pr-publish`, with both deciders returning to `implement`.
A headless `lf flow pursue` on a change whose concept review rewrites the
design iterates to implement; one with no product choice advances to
`pr-publish`; one exposing a genuine product choice stops Blocked naming it.

## Risk findings

- **Two `loop: implement` deciders compile today.** `.lf/flows/pursue-auto.yaml`
  already declares two (`loop: implement` around `gate`), and
  `flow/graph.rs` `repeated_decisions_keep_independent_counts…` covers
  independent counts. No runner change.
- **`human` is explicit per step**, not derived from skill frontmatter.
  `concept-review`'s `requires: a person present` is prose only (no Rust reads
  `requires`). The step stays non-human, so `pursue_loops_to_a_published_pr`'s
  "no human steps" assertion still holds.
- **A plain skill step cannot return Blocked.** Only deciding occurrences
  (`loop-or-next`) carry the advance/iterate/blocked protocol; a skill step's
  result is its process exit. So "return Blocked" for concept-review in a Flow
  means: record the exact unresolved product choice in the working design and
  exit; the following `loop-or-next` reads it and returns Blocked
  ("exposed an input or judgment the agent cannot supply"). This is the
  existing protocol — the skill needs to say so instead of claiming to block
  itself.
- **The tension named in the Task.** concept-review.md ends with "It is not a
  routine headless loop step. If no person is present, state the exact product
  choice … and return through the caller's existing blocked protocol." Read
  literally, a headless pursue would stop at every concept review. Resolution
  below.

## Chosen approach

1. `rust/loopflow/src/builtins/task/skill/concept-review.md`: replace the
   closing "not a routine headless loop step" paragraph with a headless/Flow
   section:
   - Without a person, run the same review against the accepted design and
     the implemented change. Make implementation-level clarifications that
     follow from accepted product decisions (names, concept consolidation,
     deletable machinery) by updating the working design as the next
     implement direction — do not rewrite code in this step.
   - Record each genuine product choice (one that changes what the product
     should be and is not settled by an accepted decision) as unresolved in
     the working design with the exact question and evidence. Never invent
     approval for it.
   - No changes and no open product choice → say so briefly; the decider
     advances. Design changed → the decider iterates to implement. Open
     product choice → the decider returns Blocked with that question.
   - Keep: do not create a Task, publish, merge, or choose navigation.
   - Update the frontmatter `requires:` so it no longer demands a person
     (e.g. "the intended experience, the current product design, and a person
     when a product choice must be made").
2. `rust/loopflow/src/builtins/task/flow/pursue.yaml`: Jack's sequence above.
3. `.lf/flows/pursue-auto.yaml`: insert `concept-review` + `loop: implement`
   after the first loop, before `gate` (assumption in `scratch/questions.md`).
4. Tests that enumerate pursue:
   - `flow/graph.rs` `pursue_loops_to_a_published_pr`: labels gain
     `concept-review`, `loop-or-next`; returns become two pairs both pointing
     at `implement` (indices 4 and 6); `sources` index for realign unchanged.
   - `flow/mod.rs` ~1298: `edges(&pursue)` becomes
     `[("loop-or-next", 4), ("loop-or-next", 4)]` (verify the actual target
     index the compiler emits).
5. Prose that enumerates pursue's steps:
   - `docs/authoring.md` ~217–221 ("One pass runs implement, compress,
     refresh … and loop-or-next … Advance leaves the loop and publishes") and
     ~45 ("Use `concept-review` … with someone present") — add that pursue
     runs it headless after the implement loop.
   - `builtins/task/skill/launch-plan.md` ~108 ("enters implement → compress →
     refresh → loop-or-next … Advance publishes").
   - Skim `docs/lf.md`, `docs/lf-reference.md`, `task_operate.md`,
     `advance.md` for step lists; they currently only name the Flow.

## Alternatives considered

- **Make concept-review a deciding step itself** (drop the second
  `loop-or-next`). Rejected: Jack's accepted sequence keeps navigation in
  `loop-or-next`, and skills that produce evidence don't navigate.
- **Mark concept-review `human`.** Rejected: that turns every pursue into a
  person-gated Flow, contradicting "Flow invocation without a product
  decision advances".

## Scope

In: the two flow YAMLs, concept-review.md, the two Rust tests, the prose above.
Out: runner/compiler changes; `ship`, workflows (`code`, `feature`) — they
reference `pursue` by name and inherit the change; LOO-521 (PR #1608).

## Done when

- `lf flow show pursue` (or `cargo run -p loopflow --bin lf -- flow show pursue`)
  lists the eight steps above with both deciders returning to `implement`.
- concept-review.md no longer says it is not a headless step; it states
  advance on no product decision and Blocked (via the decider) for a genuine
  product choice.
- `cargo test -p loopflow --lib flow::` passes, including the updated
  `pursue_loops_to_a_published_pr` and the compile-edges test.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean.

## Checks

- implement: `cargo test -p loopflow --lib flow::` and `builtins` pass (LF_* unset);
  fmt/clippy clean; `lf flow show pursue` (LF_HOME isolated, past the installed
  entry gate) lists the eight steps. Also updated
  `builtin_flows_keep_their_delivery_boundaries_without_review_steps` (returns 2).
