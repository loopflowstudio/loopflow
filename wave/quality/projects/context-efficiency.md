# Context efficiency

Ambient context earns its tokens. Audited 2026-07-08: the measurement
plumbing exists and is EMPTY — `run_token_usage` has the right shape and 0
rows against 152 completed runs; `lf usage` aggregates nothing. The 20% KR
is unmeasurable until the table is fed.

## KRs

- Provider token counts feed `run_token_usage` at run completion (upsert
  path exists in lfdb/catalog.rs); `lf usage` shows real numbers.
- Cost and per-skill dimensions get schema homes (neither exists today).
- Then: median tokens per comparable run drops 20% without first-pass gate
  regression — re-baselined under the pass model, which re-assembles
  context every pass.
- "See what a flow does — declared" ships (437ca0d).
