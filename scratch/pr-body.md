## Try it!
- `cargo test -p loopflow`
  - Confirms the bucketed ingest flow, Asana/Linear priority mapping, prompt parity, and the rest of the crate still pass together.
- `rg -n "Urgent|High|Medium|Low|PriorityBucket|from_semantic_label" docs/wave-authoring.md rust/loopflow/src/engine/builtins rust/loopflow/src/ops/ingest.rs rust/loopflow/src/lfd/pm rust/loopflow/src/ops/pm.rs`
  - Shows the shared priority model end to end across docs, prompts, ingest, shared PM types, and provider adapters.
- `lf ops pm init`
  - With PM auth configured, creates a fresh provider team/projects for all waves, clears stale local provider IDs, and writes back the new project linkage. Run this against a test workspace/team if you want to inspect the bootstrap behavior.

Validation run on this branch:
- `cargo fmt --check`
- `cargo clippy -p loopflow -- -D warnings`
- `cargo test -p loopflow`

## Intent
This change stops treating roadmap planning as one exact global queue and replaces it with four shared semantic priority buckets. Loopflow now teaches, stores, ingests, and syncs roadmap work as `Urgent` / `High` / `Medium` / `Low` meaning, while adapting that model to Asana and Linear in the native language each provider expects. It also makes `lf ops pm init` deterministic by always creating fresh PM bootstrap state instead of trying to match existing remote projects.

## Assumptions
- Existing waves may still contain legacy numbered roadmap files, so ingest continues to read them as a fallback behind `1-` through `4-` bucketed files.
- Local items without an explicit bucket prefix default to shared `High` meaning when syncing to PM.
- All waves participating in `lf ops pm init` use the same configured PM provider for bootstrap.
- Reviewers should validate semantic parity, not exact label parity: Asana may expose custom-field labels, while Linear keeps native Urgent/High/Medium/Low priorities.

## Key decisions
- Keep `PriorityBucket` as the shared model and translate at the provider edge.
- Factor provider-ID clearing and Linear priority conversion into shared helpers instead of open-coding them in each caller.
- Make `lf ops pm init` create fresh team/project state and clear stale local provider IDs before bootstrap rather than trying to reconcile against existing remote state.
- Choose the highest-priority non-empty bucket first in `ingest`, but defer any new within-bucket ordering policy.
- Add prompt/doc regression checks so old numbered-roadmap guidance does not quietly creep back in.

## Not included
- Notion integration itself.
- README sync or supporting-doc import.
- Exact within-bucket ordering.
- Arbitrary configurable priority taxonomies.
- Automatic renaming of every existing legacy roadmap file.
