# Gate Review: Preserve missingness in health evidence

## What was implemented

The lifecycle scorecard now emits an Unavailable task-loop-trust observation
when the exact seven-day source window contains no eligible settled Tasks. It
no longer manufactures an incomplete observed `0.0` for the undefined `0 / 0`
ratio. Non-empty populations retain the same ratio calculation.

Current producer output also drops the checkout-only `eligible` and
`successful` annotations. The installed binary remains tolerant of those keys
from an older checkout, while the durable metric model stays unchanged.

## Key choices

- Reuse the existing Observed and Unavailable variants instead of adding a
  third missingness state or a generic health evaluator.
- Timestamp Unavailable at the exact source-window end so freshness remains
  computable without pretending a value exists.
- Keep the producer envelope liberal across installed-binary/checkout version
  skew. Unknown producer annotations are ignored, not persisted.
- Keep `complete` on Observed evidence because partial capture must continue to
  derive Unknown rather than Met or Missed.

## How it fits together

The checkout-owned Python producer names the metric and emits Observed or
Unavailable evidence. The installed Rust binary binds contract revision,
instrument authority, and the content-addressed observation id before writing
the single `metric_observations` store. Portfolio readers derive the public
Met, Missed, Unknown, or Unavailable verdict from that durable fact.

`lf doctor` remains obligation-based through `CronObligation`; this slice puts
the same authority-backed denominator rule beside metric authoring without
creating a shared framework prematurely.

## Validation

- `uv run pytest python/tests/test_lifecycle_scorecard.py -k task_loop_trust -q`
  — 4 passed.
- `cargo test -p loopflow ops::flow::tests::telemetry_envelope_accepts_older_producer_annotations -- --exact`
  — 1 passed.
- A read-only run of the current producer against `/Users/jack/.lf/loopflow.db`
  emitted observed `0.75` for the exact current window with no removed
  annotations.
- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.

The changed-aware gate selected Python, Rust, and website. It stopped before
product tests because the host resource envelope found three active build roots
over the 12 GiB per-worktree budget: `install-unpublished-local-builds-through`
at 16.2 GiB, `make-pr-landing-a-watched` at 14.2 GiB, and `main` at 17.7 GiB.
Allowlisted recovery removed nothing because all three roots were active. The
affected suites therefore remain unproven locally and are left to CI; this is
host-state pressure, not a product assertion failure.

## Risks and bottlenecks

- The installed binary intentionally accepts unknown producer annotations.
  Adding `deny_unknown_fields` would break supported checkout skew.
- The scorecard's source-window eligibility query remains the metric's
  authority boundary. Future changes to what counts as a settled Task must
  update that query and its behavior tests together.
- Linear writeback is currently degraded by expired OAuth. GitHub publication
  succeeds; a later authenticated operation retries the link.

## What's not included

- LOO-219 still owns why the August 4–11 historical ledger events were absent.
- This slice does not change the durable metric schema, public metric DTOs,
  historical observations, or cron continuity already shipped in PR #1248.
- It does not add a generic health evaluator before a second concrete check
  proves that abstraction.
- Unassociated PR #1254 remains for its owning lifecycle to reconcile.

## Wave alignment

The change keeps nightly telemetry progressing after terminal historical gaps
and prevents an empty evidence population from becoming a false numeric health
claim. That directly supports the Infrastructure release-cadence and
actionable-red KRs without weakening historical evidence.
