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
