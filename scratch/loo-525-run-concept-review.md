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

## Approach

`concept-review.md` gains an "In a Flow without a person" section: settle what
accepted decisions imply as implement direction, record genuine product
choices as unresolved, and report advance / iterate / blocked evidence for the
following `loop-or-next`. `pursue.yaml` takes Jack's sequence;
`.lf/flows/pursue-auto.yaml` inserts the same pair before `gate`. Detail is in
commit 7f77629862.

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
