# LOO-241 PR 2 — Preserve missingness in health evidence

## Complete Target Architecture

A negative health claim is valid only when durable authority establishes its
owner, expected window, eligible population, and evidence query. Observer
silence without that denominator remains non-failing history, Unknown when
capture is partial, or Unavailable when the current source cannot produce a
value.

`lf doctor` already applies this model through one `CronObligation`: installed
identity, Home, schedule, activation, and exact scheduled receipts. Lifecycle
metric producers use the existing Observed and Unavailable variants. They emit
Observed only for an authority-covered exact window with a non-empty eligible
population; they never manufacture a numeric value for an undefined ratio.

The installed binary executes the scorecard script from the selected checkout,
so producer decoding remains liberal across binary/checkout revision skew.
Checkout-only annotations do not become fields in the durable metric model.

## This Slice

- Audit the remaining `lf doctor` failures and lifecycle scorecard missingness
  paths against the authority-backed denominator rule.
- Change the task-loop trust producer from incomplete observed `0.0` to an
  Unavailable observation when no settled Task is eligible.
- Remove producer-only eligible/successful annotations from current output
  while retaining decoder tolerance for older checkouts that still emit them.
- Put the denominator rule beside metric authoring guidance and the doctor
  implementation boundary.

## Slice Ledger

| Slice | Status |
|---|---|
| Cron continuity restoration and configured-path proof | shipped in PR #1248 |
| Root-cause analysis | complete |
| Missingness-preserving health evidence | current |

## Done When

- Zero eligible settled Tasks emits Unavailable with the exact source time and
  a reason, never observed zero.
- A non-empty task-loop population still emits the same ratio for the same
  exact window.
- Current output contains only durable metric fields, while the decoder still
  accepts the previous checkout's annotations.
- The focused task-loop trust and telemetry-envelope proofs pass.

