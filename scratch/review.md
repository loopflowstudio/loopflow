# Review: LOO-241 PR 2

## Evidence

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Empty population preserves missingness | Zero eligible settled Tasks emits Unavailable at the exact source time with a reason, never observed zero. | `task_loop_trust_observation` returns `kind: unavailable`, the window end as `source_as_of`, and `No eligible settled Tasks in the source window`. | `uv run pytest python/tests/test_lifecycle_scorecard.py -k task_loop_trust -q`; exact empty-population assertion | pass |
| Non-empty population keeps the ratio | An authority-covered non-empty window remains an exact observed ratio. | The focused fixture emits `0.5` for two successful outcomes among four eligible Tasks with the same exact window. A read-only run against the Home database emitted an observed `0.75` for its current seven-day window. | Focused pytest; `uv run scripts/lifecycle_scorecard.py --repo ... --database /Users/jack/.lf/loopflow.db --envelope` filtered to `task-loop-trust` | pass |
| Current producer output is minimal | Current observations contain only durable metric fields; older checkout annotations remain decodable. | Current live output has no `eligible` or `successful` fields. Rust ignores those fields when decoding an older producer envelope. | Read-only Home database run; `cargo test -p loopflow ops::flow::tests::telemetry_envelope_accepts_older_producer_annotations -- --exact` | pass |
| Focused proofs hold | Task-loop trust behavior and the telemetry envelope both pass. | Four Python behavior cases and the exact Rust envelope case pass. | 4 passed; 1 passed | pass |

The Home database demonstration was read-only. It exercised the real scorecard
producer and current checkout without persisting a metric observation or
mutating production state. The empty-population case used the focused local
fixture because manufacturing that state in the Home would be unsafe.

## Source Review

The model stays singular from source to storage: the Python producer emits one
Observed or Unavailable observation, the Rust binder supplies contract-owned
identity, and `metric_observations` remains the sole durable record. The
consumer already maps incomplete Observed evidence to Unknown, so `complete`
is intentional rather than a redundant field. Liberal envelope decoding is
also intentional because the installed binary may execute a different checkout
revision; the removed annotations do not become compatibility fields in the
durable model.

The remaining doctor failures name a direct invariant or a durable obligation.
The authoring guidance now places the denominator rule beside both metric and
doctor authoring boundaries. No new evaluator, adapter, DTO, table, or status
variant was introduced.

## Disposition

Coherent and ready to publish. Review fixed only two trailing-blank-line
`git diff --check` failures in scratch artifacts; no product-code gap remained.
