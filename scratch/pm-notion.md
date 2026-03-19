# PM priority-bucket redo — validation

## Try it

```bash
cargo test -p loopflow
```

Confirms bucketed ingest flow, Asana/Linear priority mapping, prompt parity, and the rest of the crate pass together.

```bash
rg -n "p0-|highest-priority non-empty bucket|PriorityBucket" \
  docs/wave-authoring.md \
  rust/loopflow/src/engine/builtins \
  rust/loopflow/src/ops/ingest.rs \
  rust/loopflow/src/lfd/pm \
  rust/loopflow/src/ops/pm.rs
```

Shows the new shared planning model end to end: docs, prompts, ingest, shared PM types, and provider adapters.

## Validation checklist

- `cargo fmt --check`
- `cargo clippy -p loopflow -- -D warnings`
- `cargo test -p loopflow`
