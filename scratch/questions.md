# LOO-1125 questions

- Q2 b ("Allow realign to edit docs inline (only code/tests force iterate)")
  is read as `docs/` and `README.md` only; skills, goldens, `AGENTS.md`,
  `.lf/qa/` and comments in source go through `implement`. Concept-review
  recommended the stricter form instead: `realign` writes only `scratch/`
  (and Wave memory), docs fixes iterate too, and `concept-review.html` never
  keeps a stale "No" row. Keep the narrowed Q2 b, or go scratch-only?
- `DECISION_CRITERIA` in `output.rs` is being changed in this Task so a
  skill's own loop rule wins over "substantial next pass". Fine here, or a
  separate Task?
- `optimize` ends `compress → graph: refresh → measure`, with no `implement`
  after `refresh`. A code mismatch `realign` finds there now stays in
  `scratch/plan.md`. Worth an `implement` step there, or fine as is?
- Loops have no pass cap. If `realign` iterating on every in-scope code finding
  makes `build` spin in practice, the fix is a cap or explore's Vision C guard;
  both are outside this Task.
- `compress` and `qa` can edit code after `concept-review`. Read the freshness
  requirement as covering the fixes queued by `realign`, matching the choice
  to leave the Graph order and review-page format alone.
- QA (2026-11-27): in a `- loop:` of `cmd: sync` and `realign` with no
  `implement`, haiku `realign` returned `iterate` three times while refining the
  plan, never `blocked`. Its third turn first returned invalid output; after the
  correction steer lf failed with `read the final answer of … it has no turn
  f9fd08fb-…` (exit 1). That transcript read is outside this Task; worth its own
  Task if it recurs.
