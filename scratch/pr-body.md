## Evaluate

```bash
uv run pytest python/tests/test_lifecycle_scorecard.py -k task_loop_trust -q
cargo test -p loopflow ops::flow::tests::telemetry_envelope_accepts_older_producer_annotations -- --exact
```

The task-loop cases pass 4/4 and the older-envelope decoder passes 1/1. A
read-only run of the current scorecard against the Home database emitted an
observed `0.75` for its exact seven-day window with no `eligible` or
`successful` annotations. The empty-population fixture emits:

```json
{
  "kind": "unavailable",
  "source_as_of": "2026-07-22T00:00:00Z",
  "reason": "No eligible settled Tasks in the source window"
}
```

The changed-aware local gate selected Python, Rust, and website but stopped
before product tests because three active build roots exceeded the repository's
per-worktree resource budget. Allowlisted recovery could not remove active
roots. Focused behavior, Rust formatting, and diff hygiene pass; CI owns the
remaining affected-suite proof.

## Why it matters

An empty source population is not a measured zero. Preserving that missingness
prevents telemetry from making a false health judgment and keeps terminal
historical gaps from permanently blocking the current scorecard.

## What changed

- Emit Unavailable with the exact source time and an actionable reason when no
  settled Task is eligible.
- Keep the existing ratio for non-empty exact windows.
- Remove producer-only count annotations while accepting them from older
  checkout revisions.
- Document the authority-backed denominator rule beside metric and doctor
  authoring boundaries.

## Risks / Not included

The producer decoder remains intentionally liberal because an installed binary
can execute a different checkout revision. This does not change durable metric
storage or public DTOs. LOO-219 owns the cause of the historical August 4–11
capture gaps; cron continuity itself shipped separately in PR #1248.
