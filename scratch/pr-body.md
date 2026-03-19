## Try it!
- `cargo test -p loopflow`
  - Confirms the bucketed ingest flow, Asana/Linear priority mapping, prompt parity, and the rest of the crate still pass together.
- `rg -n "p0-|highest-priority non-empty bucket|PriorityBucket" docs/wave-authoring.md rust/loopflow/src/engine/builtins rust/loopflow/src/ops/ingest.rs rust/loopflow/src/lfd/pm rust/loopflow/src/ops/pm.rs`
  - Shows the new shared planning model end to end: docs, prompts, ingest, shared PM types, and provider adapters.

Validation run on this branch:
- `cargo fmt --check`
- `cargo clippy -p loopflow -- -D warnings`
- `cargo test -p loopflow`

## Intent
This change stops treating roadmap planning as one exact global queue and replaces it with four semantic priority buckets. Loopflow now teaches, stores, ingests, and syncs roadmap work as shared `P0`/`P1`/`P2`/`P3` meaning, while still adapting to Asana and Linear in the language those tools use natively.

## Assumptions
- Legacy numbered roadmap files still need to work during transition, so ingest continues to read them as a fallback behind bucketed files.
- Local items without an explicit bucket prefix default to shared `P1` meaning when syncing to PM.
- Reviewers should validate semantic parity, not exact label parity: Asana may expose custom labels, while Linear keeps native Urgent/High/Medium/Low priorities.

## Key decisions
- Keep `PriorityBucket` as the shared model and translate at the provider edge.
- Make prompts talk about bucket meaning, not exact provider spelling.
- Choose the highest-priority non-empty bucket first in `ingest`, but defer any new within-bucket ordering policy.
- Add prompt/doc regression checks so old numbered-roadmap guidance does not quietly creep back in.

## Not included
- Notion integration itself.
- README sync or supporting-doc import.
- Exact within-bucket ordering.
- Arbitrary configurable priority taxonomies.
