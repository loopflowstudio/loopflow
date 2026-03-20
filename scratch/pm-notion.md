# PM priority rename + init rework — validation

## Try it

```bash
cargo test -p loopflow
```

Confirms priority ingest flow (Urgent/High/Medium/Low, `1-` through `4-` prefixes), Asana/Linear priority mapping, prompt parity, and the rest of the crate pass together.

```bash
rg -n "Urgent|High|Medium|Low|PriorityBucket|from_semantic_label" \
  docs/wave-authoring.md \
  rust/loopflow/src/engine/builtins \
  rust/loopflow/src/ops/ingest.rs \
  rust/loopflow/src/lfd/pm \
  rust/loopflow/src/ops/pm.rs
```

Shows the shared priority model end to end: docs, prompts, ingest, shared PM types, and provider adapters.

```bash
lf ops pm init
```

Creates fresh projects for all waves. No per-wave init, no matching against existing remote state.

## Validation checklist

- `cargo fmt --check`
- `cargo clippy -p loopflow -- -D warnings`
- `cargo test -p loopflow`
